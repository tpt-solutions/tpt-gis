//! Pure-Rust GeoPackage (`.gpkg`) reader and writer.
//!
//! GeoPackage is, per the OGC spec, a SQLite database file. This module reads and
//! writes it without any C FFI (no `rusqlite`/`libsqlite3-sys`) and with no external
//! engine — the underlying SQLite file-format logic lives in [`sqlite`], and the
//! geometry BLOB codec (a GeoPackageBinary header wrapping standard WKB) lives in
//! [`geometry`]. Both are zero-dependency and reuse the crate's existing WKB reader
//! and the shared [`geometry::Geometry`] type, so output is identical in shape to the
//! GeoJSON / WKT / Shapefile readers.
//!
//! The public entry points are [`read_gpkg`] (a `.gpkg` byte slice →
//! `Vec<geojson::Feature>`) and [`write_gpkg`] (features + [`WriteOptions`] → `.gpkg`
//! bytes). Reading handles leaf and simple interior table b-trees, inline and
//! overflow payloads, and `INTEGER PRIMARY KEY` rowid aliasing. Writing is
//! intentionally limited to **single-leaf-page tables** (no interior splitting, no
//! R-tree spatial index, no tile/attributes extension, no overflow pages) — see the
//! [`sqlite`] module docs for the exact scope.
//!
//! # Attribute mapping
//! `INTEGER`/`REAL` → JSON number, `TEXT` → JSON string, `BOOLEAN` → JSON bool (stored
//! as an integer `0`/`1`), and attribute `BLOB` → base64-encoded JSON string (JSON has
//! no byte type). A geometry column is decoded from its BLOB; on write it is encoded
//! via [`geometry::encode_geometry_blob`].

mod geometry;
mod sqlite;

pub use geometry::{decode_geometry_blob, encode_geometry_blob, GeoPackageGeometryError};
pub use sqlite::{CellValue, Column, SqliteError};

use core::fmt;
use std::collections::{BTreeSet, HashMap};

use serde_json::{Map, Value};

use crate::geojson::Feature;
use crate::geometry::Geometry;

/// An error encountered while reading or writing a GeoPackage file.
#[derive(Debug)]
pub enum GeoPackageError {
    /// The underlying SQLite file format failed to parse.
    Sqlite(SqliteError),
    /// A geometry BLOB failed to decode.
    Geometry(GeoPackageGeometryError),
    /// No feature tables (no `gpkg_geometry_columns` entries) were found.
    NoFeatureTables,
    /// A required GeoPackage metadata table was missing.
    MissingMetadataTable(String),
    /// A write target was too large for the single-leaf-page writer (a record would
    /// not fit inline, or the table would overflow a page).
    WriteTooLarge,
}

impl fmt::Display for GeoPackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoPackageError::Sqlite(e) => write!(f, "SQLite error: {e}"),
            GeoPackageError::Geometry(e) => write!(f, "geometry BLOB error: {e}"),
            GeoPackageError::NoFeatureTables => write!(f, "GeoPackage contains no feature tables"),
            GeoPackageError::MissingMetadataTable(name) => {
                write!(f, "GeoPackage is missing required table {name}")
            }
            GeoPackageError::WriteTooLarge => {
                write!(f, "GeoPackage write target exceeds the single-leaf-page limit")
            }
        }
    }
}

impl std::error::Error for GeoPackageError {}

impl From<SqliteError> for GeoPackageError {
    fn from(e: SqliteError) -> Self {
        GeoPackageError::Sqlite(e)
    }
}

impl From<GeoPackageGeometryError> for GeoPackageError {
    fn from(e: GeoPackageGeometryError) -> Self {
        GeoPackageError::Geometry(e)
    }
}

/// Options controlling [`write_gpkg`].
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// The feature table name (also recorded in `gpkg_contents` / `gpkg_geometry_columns`).
    pub table_name: String,
    /// The name of the geometry column.
    pub geometry_column: String,
    /// The SRS id (EPSG code) for the geometry and contents entries.
    pub srs_id: i32,
    /// An optional `INTEGER PRIMARY KEY` column. If set, its values are taken from each
    /// feature's properties (falling back to a sequential id); otherwise an implicit
    /// rowid is used.
    pub id_column: Option<String>,
    /// An optional human-readable description for `gpkg_contents`.
    pub description: Option<String>,
}

/// Reads a GeoPackage file into a flat list of [`Feature`]s, one per feature-table row
/// across every feature table declared in `gpkg_geometry_columns`.
///
/// Geometry BLOBs are decoded via [`decode_geometry_blob`]; other columns become
/// `Feature.properties` (`BLOB` attributes become base64 strings).
///
/// # Errors
/// Returns [`GeoPackageError`] if the file is not a valid SQLite database, a required
/// metadata table is missing, or a geometry BLOB fails to decode.
pub fn read_gpkg(bytes: &[u8]) -> Result<Vec<Feature>, GeoPackageError> {
    let master_columns = sqlite_master_columns();
    let master = sqlite::read_table(bytes, 1, &master_columns)?;

    let tables: HashMap<String, (u32, String)> = master
        .into_iter()
        .filter_map(|(_, vals)| {
            let name = text_of(&vals[1])?;
            let rootpage = match &vals[3] {
                CellValue::Integer(n) => u32::try_from(*n).ok()?,
                _ => return None,
            };
            let sql = match &vals[4] {
                CellValue::Text(s) => s.clone(),
                _ => String::new(),
            };
            Some((name, (rootpage, sql)))
        })
        .collect();

    let geom_columns = read_geometry_columns(bytes, &tables)?;
    if geom_columns.is_empty() {
        return Err(GeoPackageError::NoFeatureTables);
    }

    let mut features = Vec::new();
    for (table_name, geom_col) in geom_columns {
        let Some((rootpage, sql)) = tables.get(&table_name) else {
            continue;
        };
        let columns = parse_create_table(sql);
        let rows = sqlite::read_table(bytes, *rootpage, &columns)?;
        for (_rid, vals) in rows {
            let mut properties = Map::new();
            let mut geometry: Option<Geometry> = None;
            for (col, val) in columns.iter().zip(vals) {
                if col.name == geom_col {
                    if let CellValue::Blob(blob) = &val {
                        if let Ok(Some((g, _))) = decode_geometry_blob(blob) {
                            geometry = Some(g);
                        }
                    }
                    continue;
                }
                properties.insert(col.name.clone(), cell_to_json(val));
            }
            features.push(Feature { geometry, properties });
        }
    }
    Ok(features)
}

/// Writes a set of [`Feature`]s to a minimal valid GeoPackage byte buffer.
///
/// See the module docs for the (intentional) writer limitations: single-leaf-page
/// tables, no spatial index, no overflow pages. Each record must therefore fit inline
/// in a 4096-byte page.
///
/// # Errors
/// Returns [`GeoPackageError::WriteTooLarge`] if any record (or the assembled table)
/// would exceed the single-leaf-page limit, or [`GeoPackageError::MissingMetadataTable`]
/// if the inputs are inconsistent.
pub fn write_gpkg(features: &[Feature], opts: &WriteOptions) -> Result<Vec<u8>, GeoPackageError> {
    const PAGE_SIZE: usize = 4096;
    const RESERVED: u8 = 0;
    const PAGE_COUNT: u32 = 5;

    // Collect attribute columns (sorted, excluding the optional id column).
    let mut attr_names: BTreeSet<String> = BTreeSet::new();
    for feature in features {
        for key in feature.properties.keys() {
            if Some(key) != opts.id_column.as_ref() {
                attr_names.insert(key.clone());
            }
        }
    }
    let attr_specs: Vec<(String, String)> =
        attr_names.iter().map(|name| (name.clone(), infer_sql_type(features, name))).collect();

    // Full column list (in CREATE TABLE order) and the stored subset (alias excluded).
    let mut all_cols: Vec<(String, String, bool)> = Vec::new();
    if let Some(id) = &opts.id_column {
        all_cols.push((id.clone(), "INTEGER".into(), true));
    }
    all_cols.push((opts.geometry_column.clone(), "GEOMETRY".into(), false));
    for (name, ty) in &attr_specs {
        all_cols.push((name.clone(), ty.clone(), false));
    }

    let stored_columns: Vec<Column> = all_cols
        .iter()
        .filter(|(_, _, alias)| !*alias)
        .map(|(name, _, _)| Column { name: name.clone(), is_rowid_alias: false })
        .collect();

    // Feature rows.
    let mut feature_cells: Vec<(i64, Vec<u8>)> = Vec::new();
    for (idx, feature) in features.iter().enumerate() {
        let rowid = if let Some(id) = &opts.id_column {
            match feature.properties.get(id) {
                Some(Value::Number(n)) => n.as_i64().unwrap_or(idx as i64 + 1),
                _ => idx as i64 + 1,
            }
        } else {
            idx as i64 + 1
        };

        let mut vals: Vec<CellValue> = Vec::with_capacity(stored_columns.len());
        match &feature.geometry {
            Some(g) => vals.push(CellValue::Blob(encode_geometry_blob(g, opts.srs_id))),
            None => vals.push(CellValue::Null),
        }
        for (name, _) in &attr_specs {
            let value = feature.properties.get(name).cloned().unwrap_or(Value::Null);
            vals.push(json_to_cell(&value));
        }
        let payload = sqlite::encode_record(&stored_columns, &vals);
        check_fits(PAGE_SIZE, RESERVED, 0, &feature_cells, &payload)?;
        feature_cells.push((rowid, payload));
    }

    // Metadata tables.
    let srs_cells = build_meta_cells(&srs_columns(), &srs_row(opts));
    let contents_cells = build_meta_cells(&contents_columns(), &contents_row(opts));
    let geomcols_cells = build_meta_cells(&geom_columns_cols(), &geomcols_row(opts));

    // Assemble pages: 1 = master, 2 = srs, 3 = contents, 4 = geomcols, 5 = features.
    let mut buf = vec![0u8; PAGE_SIZE * usize::try_from(PAGE_COUNT).unwrap()];

    let master_cells = build_master_cells(opts, &attr_specs);
    let master_page = sqlite::build_leaf_page(PAGE_SIZE, 100, &master_cells);
    buf[0..PAGE_SIZE].copy_from_slice(&master_page);

    let srs_page = sqlite::build_leaf_page(PAGE_SIZE, 0, &srs_cells);
    buf[PAGE_SIZE..2 * PAGE_SIZE].copy_from_slice(&srs_page);

    let contents_page = sqlite::build_leaf_page(PAGE_SIZE, 0, &contents_cells);
    buf[2 * PAGE_SIZE..3 * PAGE_SIZE].copy_from_slice(&contents_page);

    let geomcols_page = sqlite::build_leaf_page(PAGE_SIZE, 0, &geomcols_cells);
    buf[3 * PAGE_SIZE..4 * PAGE_SIZE].copy_from_slice(&geomcols_page);

    let feature_page = sqlite::build_leaf_page(PAGE_SIZE, 0, &feature_cells);
    buf[4 * PAGE_SIZE..5 * PAGE_SIZE].copy_from_slice(&feature_page);

    // The DB header overwrites the first 100 bytes of page 1 (the b-tree header of the
    // master page begins at offset 100).
    write_db_header(&mut buf, PAGE_SIZE, PAGE_COUNT);

    Ok(buf)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn sqlite_master_columns() -> Vec<Column> {
    ["type", "name", "tbl_name", "rootpage", "sql"]
        .iter()
        .map(|n| Column { name: (*n).to_string(), is_rowid_alias: false })
        .collect()
}

fn text_of(value: &CellValue) -> Option<String> {
    match value {
        CellValue::Text(s) => Some(s.clone()),
        _ => None,
    }
}

fn read_geometry_columns(
    bytes: &[u8],
    tables: &HashMap<String, (u32, String)>,
) -> Result<Vec<(String, String)>, GeoPackageError> {
    let Some((rootpage, sql)) = tables.get("gpkg_geometry_columns") else {
        return Ok(Vec::new());
    };
    let columns = parse_create_table(sql);
    let table_name_idx = columns.iter().position(|c| c.name.eq_ignore_ascii_case("table_name"));
    let column_name_idx = columns.iter().position(|c| c.name.eq_ignore_ascii_case("column_name"));
    let (tn, cn) = match (table_name_idx, column_name_idx) {
        (Some(a), Some(b)) => (a, b),
        _ => return Ok(Vec::new()),
    };
    let rows = sqlite::read_table(bytes, *rootpage, &columns)?;
    let mut out = Vec::new();
    for (_rid, vals) in rows {
        if let (CellValue::Text(t), CellValue::Text(c)) = (&vals[tn], &vals[cn]) {
            out.push((t.clone(), c.clone()));
        }
    }
    Ok(out)
}

/// Parses a `CREATE TABLE` statement into its column list, skipping table-level
/// constraints (`PRIMARY KEY (...)`, `FOREIGN KEY`, `UNIQUE`, `CHECK`, `CONSTRAINT`).
fn parse_create_table(sql: &str) -> Vec<Column> {
    let open = match sql.find('(') {
        Some(i) => i,
        None => return Vec::new(),
    };
    let body = &sql[open + 1..];
    let mut depth = 0;
    let mut end = body.len();
    for (i, c) in body.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &body[..end];

    let mut columns = Vec::new();
    for def in split_top_level(body) {
        let def = def.trim();
        if def.is_empty() {
            continue;
        }
        let first = match def.split_whitespace().next() {
            Some(f) => f,
            None => continue,
        };
        match first.to_ascii_uppercase().as_str() {
            "PRIMARY" | "UNIQUE" | "FOREIGN" | "CHECK" | "CONSTRAINT" | "KEY" => continue,
            _ => {}
        }
        let name = strip_ident(first);
        let mut parts = def.split_whitespace();
        parts.next();
        let type_tok = parts.next().unwrap_or("").to_ascii_uppercase();
        let rest = def[name.len()..].to_ascii_uppercase();
        let is_alias = type_tok == "INTEGER" && rest.contains("PRIMARY KEY");
        columns.push(Column { name, is_rowid_alias: is_alias });
    }
    columns
}

/// Splits a `CREATE TABLE` body on top-level commas (ignoring commas inside `()`).
fn split_top_level(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur);
    }
    parts
}

/// Removes surrounding double quotes / backticks / square brackets from an identifier.
fn strip_ident(ident: &str) -> String {
    let bytes = ident.as_bytes();
    if !bytes.is_empty() {
        match bytes[0] {
            b'"' | b'`' if bytes.last() == Some(&bytes[0]) => {
                return ident[1..ident.len() - 1].to_string();
            }
            b'[' if bytes.last() == Some(&b']') => {
                return ident[1..ident.len() - 1].to_string();
            }
            _ => {}
        }
    }
    ident.to_string()
}

fn cell_to_json(value: CellValue) -> Value {
    match value {
        CellValue::Null => Value::Null,
        CellValue::Integer(i) => Value::Number(i.into()),
        CellValue::Real(f) => {
            Value::Number(serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)))
        }
        CellValue::Text(s) => Value::String(s),
        CellValue::Blob(b) => Value::String(base64_encode(&b)),
    }
}

fn json_to_cell(value: &Value) -> CellValue {
    match value {
        Value::Null => CellValue::Null,
        Value::Bool(b) => CellValue::Integer(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CellValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                CellValue::Real(f)
            } else {
                CellValue::Null
            }
        }
        Value::String(s) => CellValue::Text(s.clone()),
        Value::Array(_) | Value::Object(_) => {
            CellValue::Text(serde_json::to_string(value).unwrap_or_default())
        }
    }
}

/// Infers a SQL column type for `key` from the first non-null value across `features`.
fn infer_sql_type(features: &[Feature], key: &str) -> String {
    for feature in features {
        if let Some(value) = feature.properties.get(key) {
            match value {
                Value::Bool(_) => return "INTEGER".into(),
                Value::Number(n) => {
                    if n.is_i64() {
                        return "INTEGER".into();
                    }
                    if n.is_f64() {
                        return "REAL".into();
                    }
                }
                Value::String(_) => return "TEXT".into(),
                _ => {}
            }
        }
    }
    "TEXT".into()
}

fn meta_columns(defs: &[(&str, &str)]) -> Vec<Column> {
    defs.iter()
        .map(|(name, _)| Column { name: (*name).to_string(), is_rowid_alias: false })
        .collect()
}

fn build_meta_cells(columns: &[Column], row: &[CellValue]) -> Vec<(i64, Vec<u8>)> {
    vec![(1, sqlite::encode_record(columns, row))]
}

fn check_fits(
    page_size: usize,
    reserved: u8,
    header_offset: usize,
    existing: &[(i64, Vec<u8>)],
    payload: &[u8],
) -> Result<(), GeoPackageError> {
    let max_local = page_size - usize::from(reserved) - 35;
    if payload.len() > max_local {
        return Err(GeoPackageError::WriteTooLarge);
    }
    let used: usize = existing.iter().map(|(_, p)| p.len()).sum();
    let need = header_offset + 8 + 2 * (existing.len() + 1) + used + payload.len();
    if need > page_size {
        return Err(GeoPackageError::WriteTooLarge);
    }
    Ok(())
}

fn srs_columns() -> Vec<Column> {
    meta_columns(&[
        ("srs_name", "TEXT"),
        ("srs_id", "INTEGER"),
        ("organization", "TEXT"),
        ("organization_coordsys_id", "INTEGER"),
        ("definition", "TEXT"),
        ("description", "TEXT"),
    ])
}

fn srs_row(opts: &WriteOptions) -> Vec<CellValue> {
    let (name, defn) = srs_definition(opts.srs_id);
    vec![
        CellValue::Text(name),
        CellValue::Integer(i64::from(opts.srs_id)),
        CellValue::Text("EPSG".into()),
        CellValue::Integer(i64::from(opts.srs_id)),
        CellValue::Text(defn),
        CellValue::Text(String::new()),
    ]
}

fn srs_definition(srs_id: i32) -> (String, String) {
    if srs_id == 4326 {
        (
            "WGS 84".into(),
            "GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],\
PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]]"
                .into(),
        )
    } else {
        ("Undefined Cartesian".into(), "undefined".into())
    }
}

fn contents_columns() -> Vec<Column> {
    meta_columns(&[
        ("table_name", "TEXT"),
        ("data_type", "TEXT"),
        ("identifier", "TEXT"),
        ("description", "TEXT"),
        ("last_change", "DATETIME"),
        ("min_x", "DOUBLE"),
        ("min_y", "DOUBLE"),
        ("max_x", "DOUBLE"),
        ("max_y", "DOUBLE"),
        ("srs_id", "INTEGER"),
    ])
}

fn contents_row(opts: &WriteOptions) -> Vec<CellValue> {
    vec![
        CellValue::Text(opts.table_name.clone()),
        CellValue::Text("features".into()),
        CellValue::Text(opts.table_name.clone()),
        CellValue::Text(opts.description.clone().unwrap_or_default()),
        CellValue::Text("2024-01-01T00:00:00Z".into()),
        CellValue::Null,
        CellValue::Null,
        CellValue::Null,
        CellValue::Null,
        CellValue::Integer(i64::from(opts.srs_id)),
    ]
}

fn geom_columns_cols() -> Vec<Column> {
    meta_columns(&[
        ("table_name", "TEXT"),
        ("column_name", "TEXT"),
        ("geometry_type_name", "TEXT"),
        ("srs_id", "INTEGER"),
        ("z", "TINYINT"),
        ("m", "TINYINT"),
    ])
}

fn geomcols_row(opts: &WriteOptions) -> Vec<CellValue> {
    vec![
        CellValue::Text(opts.table_name.clone()),
        CellValue::Text(opts.geometry_column.clone()),
        CellValue::Text("GEOMETRY".into()),
        CellValue::Integer(i64::from(opts.srs_id)),
        CellValue::Integer(0),
        CellValue::Integer(0),
    ]
}

/// Builds the `sqlite_master` rows for the four tables this writer emits.
fn build_master_cells(opts: &WriteOptions, attr_specs: &[(String, String)]) -> Vec<(i64, Vec<u8>)> {
    let master_columns = sqlite_master_columns();
    let defs = [
        ("gpkg_spatial_ref_sys", srs_sql(), 2u32),
        ("gpkg_contents", contents_sql(), 3),
        ("gpkg_geometry_columns", geomcols_sql(), 4),
        (&opts.table_name, feature_sql(opts, attr_specs), 5),
    ];
    defs.iter()
        .enumerate()
        .map(|(i, (name, sql, rootpage))| {
            let row = vec![
                CellValue::Text("table".into()),
                CellValue::Text((*name).to_string()),
                CellValue::Text((*name).to_string()),
                CellValue::Integer(i64::from(*rootpage)),
                CellValue::Text((*sql).to_string()),
            ];
            (i as i64 + 1, sqlite::encode_record(&master_columns, &row))
        })
        .collect()
}

fn srs_sql() -> String {
    "CREATE TABLE gpkg_spatial_ref_sys (srs_name TEXT, srs_id INTEGER, organization TEXT, \
organization_coordsys_id INTEGER, definition TEXT, description TEXT)"
        .to_string()
}

fn contents_sql() -> String {
    "CREATE TABLE gpkg_contents (table_name TEXT, data_type TEXT, identifier TEXT, \
description TEXT, last_change DATETIME, min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, \
max_y DOUBLE, srs_id INTEGER)"
        .to_string()
}

fn geomcols_sql() -> String {
    "CREATE TABLE gpkg_geometry_columns (table_name TEXT, column_name TEXT, \
geometry_type_name TEXT, srs_id INTEGER, z TINYINT, m TINYINT)"
        .to_string()
}

fn feature_sql(opts: &WriteOptions, attr_specs: &[(String, String)]) -> String {
    let mut defs = Vec::new();
    if let Some(id) = &opts.id_column {
        defs.push(format!("\"{id}\" INTEGER PRIMARY KEY"));
    }
    defs.push(format!("\"{}\" GEOMETRY", opts.geometry_column));
    for (name, ty) in attr_specs {
        defs.push(format!("\"{name}\" {ty}"));
    }
    format!("CREATE TABLE \"{}\" ({})", opts.table_name, defs.join(", "))
}

fn write_db_header(buf: &mut [u8], page_size: usize, page_count: u32) {
    buf[0..16].copy_from_slice(sqlite::MAGIC);
    let ps = if page_size == 65536 { 1u16 } else { page_size as u16 };
    buf[16..18].copy_from_slice(&ps.to_be_bytes());
    buf[18] = 1; // file format write version
    buf[19] = 1; // file format read version
    buf[20] = 0; // reserved bytes at page end
    buf[21] = 64; // max embedded payload fraction
    buf[22] = 32; // min embedded payload fraction
    buf[23] = 32; // leaf payload fraction
    buf[28..32].copy_from_slice(&page_count.to_be_bytes());
    buf[40..44].copy_from_slice(&1u32.to_be_bytes()); // schema cookie
    buf[44..48].copy_from_slice(&4u32.to_be_bytes()); // schema format number
    buf[56..60].copy_from_slice(&1u32.to_be_bytes()); // text encoding = UTF-8
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 encoder (no external dependency needed).
fn base64_encode(input: &[u8]) -> String {
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(B64[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn sample_feature() -> Feature {
        let mut props = Map::new();
        props.insert("name".into(), Value::String("alpha".into()));
        props.insert("count".into(), Value::Number(serde_json::Number::from(3)));
        props.insert("ratio".into(), Value::Number(serde_json::Number::from_f64(1.5).unwrap()));
        props.insert("flag".into(), Value::Bool(true));
        Feature { geometry: Some(Geometry::Point(Point::new(1.0, 2.0))), properties: props }
    }

    fn opts() -> WriteOptions {
        WriteOptions {
            table_name: "features".into(),
            geometry_column: "geom".into(),
            srs_id: 4326,
            id_column: Some("fid".into()),
            description: Some("test layer".into()),
        }
    }

    #[test]
    fn write_then_read_round_trips() {
        let features = vec![sample_feature(), sample_feature()];
        let bytes = write_gpkg(&features, &opts()).unwrap();
        let read = read_gpkg(&bytes).unwrap();
        assert_eq!(read.len(), 2);
        for f in &read {
            assert_eq!(f.geometry, Some(Geometry::Point(Point::new(1.0, 2.0))));
            assert_eq!(f.properties.get("name"), Some(&Value::String("alpha".into())));
            assert_eq!(f.properties.get("count"), Some(&Value::Number(3.into())));
            assert_eq!(
                f.properties.get("ratio"),
                Some(&Value::Number(serde_json::Number::from_f64(1.5).unwrap()))
            );
            assert_eq!(f.properties.get("flag"), Some(&Value::Number(1.into())));
        }
    }

    #[test]
    fn round_trip_without_id_column() {
        let mut f = sample_feature();
        f.properties.remove("fid");
        let bytes = write_gpkg(&[f], &WriteOptions { id_column: None, ..opts() }).unwrap();
        let read = read_gpkg(&bytes).unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].geometry, Some(Geometry::Point(Point::new(1.0, 2.0))));
    }

    #[test]
    fn produced_file_is_recognized_as_sqlite() {
        let bytes = write_gpkg(&[sample_feature()], &opts()).unwrap();
        let info = sqlite::parse_db_header(&bytes).unwrap();
        assert_eq!(info.page_size, 4096);
        assert_eq!(&bytes[0..16], sqlite::MAGIC);
    }

    #[test]
    fn parse_create_table_finds_rowid_alias() {
        let sql = "CREATE TABLE \"t\" (\"fid\" INTEGER PRIMARY KEY, \"geom\" GEOMETRY, \"a\" TEXT)";
        let cols = parse_create_table(sql);
        assert_eq!(cols.len(), 3);
        assert!(cols[0].is_rowid_alias);
        assert!(!cols[1].is_rowid_alias);
    }
}
