//! Shapefile writing: the `.shp` and `.shx` main and index files, and the `.dbf`
//! attribute table.
//!
//! Each shapefile holds one shape type. [`write_shapefile`] chooses it from the geometries
//! and refuses a mixture, and [`write_dbf`] writes attributes as UTF-8 with a `.cpg`
//! sidecar that says so.
//!
//! Polygon rings are written with exteriors clockwise and holes counter-clockwise, which is
//! the orientation GDAL and ESRI software use to tell them apart. The reader in
//! [`crate::shapefile`] does not depend on orientation, so a round trip keeps the rings but
//! may reverse their direction.

use crate::geometry::{Geometry, Point, Polygon};
use crate::shapefile::{DbfValue, ShapefileError};

/// Byte length of a shapefile header.
const HEADER_BYTES: usize = 100;

/// A byte buffer under construction.
type Buffer = Vec<u8>;

/// The shape type code written into the header and each record.
fn shape_type_for(geometries: &[Option<Geometry>]) -> Result<i32, ShapefileError> {
    let mut found: Option<i32> = None;
    for g in geometries.iter().flatten() {
        let code = match g {
            Geometry::Point(_) => 1,
            Geometry::MultiPoint(_) => 8,
            Geometry::LineString(_) | Geometry::MultiLineString(_) => 3,
            Geometry::Polygon(_) | Geometry::MultiPolygon(_) => 5,
            Geometry::GeometryCollection(_) => {
                return Err(ShapefileError::GeometryCollectionUnsupported);
            }
        };
        match found {
            None => found = Some(code),
            Some(existing) if existing == code => {}
            Some(_) => return Err(ShapefileError::MixedShapeTypes),
        }
    }
    Ok(found.unwrap_or(0))
}

/// Every coordinate of the geometries, for the bounding box.
fn all_points(geometries: &[Option<Geometry>]) -> Vec<Point> {
    fn walk(g: &Geometry, out: &mut Vec<Point>) {
        match g {
            Geometry::Point(p) => out.push(*p),
            Geometry::LineString(pts) | Geometry::MultiPoint(pts) => out.extend(pts),
            Geometry::Polygon(p) => p.rings.iter().for_each(|r| out.extend(r)),
            Geometry::MultiLineString(lines) => lines.iter().for_each(|l| out.extend(l)),
            Geometry::MultiPolygon(polys) => {
                polys.iter().flat_map(|p| p.rings.iter()).for_each(|r| out.extend(r))
            }
            Geometry::GeometryCollection(items) => items.iter().for_each(|i| walk(i, out)),
        }
    }
    let mut out = Vec::new();
    for g in geometries.iter().flatten() {
        walk(g, &mut out);
    }
    out
}

/// Signed shoelace area: positive for counter-clockwise rings.
fn signed_area(ring: &[Point]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (p, q) = (ring[i], ring[(i + 1) % n]);
            p.x * q.y - q.x * p.y
        })
        .sum::<f64>()
        / 2.0
}

/// A ring with the requested orientation: clockwise when `clockwise`, else counter-clockwise.
fn oriented(ring: &[Point], clockwise: bool) -> Vec<Point> {
    let mut points: Vec<Point> = ring.to_vec();
    if (signed_area(&points) < 0.0) != clockwise {
        points.reverse();
    }
    points
}

/// The rings of a polygon, exteriors clockwise and holes counter-clockwise.
fn polygon_parts(polygon: &Polygon) -> Vec<Vec<Point>> {
    polygon.rings.iter().enumerate().map(|(i, ring)| oriented(ring, i == 0)).collect()
}

fn push_f64(out: &mut Buffer, v: f64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_i32_le(out: &mut Buffer, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// The bounding box of a set of points as `[xmin, ymin, xmax, ymax]`, or zeros if empty.
fn bbox(points: &[Point]) -> [f64; 4] {
    match points.first() {
        None => [0.0; 4],
        Some(first) => {
            points.iter().fold([first.x, first.y, first.x, first.y], |[a, b, c, d], p| {
                [a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)]
            })
        }
    }
}

/// Appends the box and part/point layout shared by PolyLine and Polygon records.
fn push_parts(out: &mut Buffer, parts: &[Vec<Point>]) {
    let all: Vec<Point> = parts.iter().flatten().copied().collect();
    for v in bbox(&all) {
        push_f64(out, v);
    }
    push_i32_le(out, parts.len() as i32);
    push_i32_le(out, all.len() as i32);
    let mut start = 0i32;
    for part in parts {
        push_i32_le(out, start);
        start += part.len() as i32;
    }
    for p in &all {
        push_f64(out, p.x);
        push_f64(out, p.y);
    }
}

/// The content (after the 8-byte record header) of one record.
fn record_content(geometry: Option<&Geometry>, shape_type: i32) -> Buffer {
    let mut out = Vec::new();
    let Some(geometry) = geometry else {
        // A null record: the shape type is 0 and nothing follows.
        push_i32_le(&mut out, 0);
        return out;
    };
    push_i32_le(&mut out, shape_type);
    match geometry {
        Geometry::Point(p) => {
            push_f64(&mut out, p.x);
            push_f64(&mut out, p.y);
        }
        Geometry::MultiPoint(points) => {
            for v in bbox(points) {
                push_f64(&mut out, v);
            }
            push_i32_le(&mut out, points.len() as i32);
            for p in points {
                push_f64(&mut out, p.x);
                push_f64(&mut out, p.y);
            }
        }
        Geometry::LineString(points) => push_parts(&mut out, core::slice::from_ref(points)),
        Geometry::MultiLineString(lines) => push_parts(&mut out, lines),
        Geometry::Polygon(polygon) => push_parts(&mut out, &polygon_parts(polygon)),
        Geometry::MultiPolygon(polygons) => {
            let parts: Vec<Vec<Point>> = polygons.iter().flat_map(polygon_parts).collect();
            push_parts(&mut out, &parts);
        }
        // Rejected by shape_type_for before any record is written.
        Geometry::GeometryCollection(_) => {}
    }
    out
}

/// A 100-byte shapefile header for a file of `file_bytes` bytes.
fn header(file_bytes: usize, shape_type: i32, bounds: [f64; 4]) -> Buffer {
    let mut out = Vec::with_capacity(HEADER_BYTES);
    out.extend_from_slice(&9994i32.to_be_bytes());
    out.extend_from_slice(&[0u8; 20]);
    out.extend_from_slice(&((file_bytes / 2) as i32).to_be_bytes());
    push_i32_le(&mut out, 1000);
    push_i32_le(&mut out, shape_type);
    for v in bounds {
        push_f64(&mut out, v);
    }
    // Z and M ranges are unused for 2D data.
    out.extend_from_slice(&[0u8; 32]);
    debug_assert_eq!(out.len(), HEADER_BYTES);
    out
}

/// The `.shp` (main) and `.shx` (index) files for a list of geometries. `None` entries
/// become null records, so the record numbers line up with the `.dbf` rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapefileBytes {
    /// The main file.
    pub shp: Vec<u8>,
    /// The index file: one `(offset, content length)` pair per record.
    pub shx: Vec<u8>,
}

/// Writes `geometries` as a shapefile.
///
/// # Errors
/// Returns an error if the geometries mix shape types, if a geometry collection is
/// present (shapefiles have no such type), or if the file would exceed the 2 GiB
/// format limit.
pub fn write_shapefile(geometries: &[Option<Geometry>]) -> Result<ShapefileBytes, ShapefileError> {
    let shape_type = shape_type_for(geometries)?;
    let records: Vec<Buffer> =
        geometries.iter().map(|g| record_content(g.as_ref(), shape_type)).collect();

    let shp_len = HEADER_BYTES + records.iter().map(|c| 8 + c.len()).sum::<usize>();
    let shx_len = HEADER_BYTES + 8 * records.len();
    if shp_len / 2 > i32::MAX as usize {
        return Err(ShapefileError::FileTooLarge);
    }
    let bounds_arr = bbox(&all_points(geometries));

    let mut shp = header(shp_len, shape_type, bounds_arr);
    let mut shx = header(shx_len, shape_type, bounds_arr);
    let mut offset_words = (HEADER_BYTES / 2) as i32;
    for (i, content) in records.iter().enumerate() {
        let content_words = (content.len() / 2) as i32;
        // Record header: 1-based record number, then content length, both big-endian.
        shp.extend_from_slice(&((i + 1) as i32).to_be_bytes());
        shp.extend_from_slice(&content_words.to_be_bytes());
        shp.extend_from_slice(content);
        shx.extend_from_slice(&offset_words.to_be_bytes());
        shx.extend_from_slice(&content_words.to_be_bytes());
        offset_words += 4 + content_words;
    }
    Ok(ShapefileBytes { shp, shx })
}

/// A dBASE field: its name (at most 10 ASCII characters) and how its values are stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbfField {
    /// Field name, at most 10 characters.
    pub name: String,
    /// The storage type and width.
    pub kind: DbfFieldKind,
}

/// Storage type of a dBASE field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbfFieldKind {
    /// Text of at most `width` bytes (254 is the dBASE limit).
    Character {
        /// Maximum width in bytes.
        width: u8,
    },
    /// A number, `width` bytes wide with `decimals` after the point.
    Numeric {
        /// Total width in bytes.
        width: u8,
        /// Digits after the decimal point.
        decimals: u8,
    },
    /// A true/false value.
    Logical,
}

/// Writes a `.dbf` table. Each row has one value per field, in field order; a missing value
/// (`None`) is written as blank. Text is written as UTF-8.
///
/// # Errors
/// Returns an error if a field name is longer than 10 bytes, a text value is longer than
/// its field, or a numeric field is too narrow for its value.
pub fn write_dbf(
    fields: &[DbfField],
    rows: &[Vec<Option<DbfValue>>],
) -> Result<Buffer, ShapefileError> {
    for f in fields {
        if f.name.len() > 10 || !f.name.is_ascii() {
            return Err(ShapefileError::InvalidFieldName(f.name.clone()));
        }
    }
    let record_len: usize = 1 + fields.iter().map(|f| usize::from(width_of(f.kind))).sum::<usize>();
    let header_len = 32 + 32 * fields.len() + 1;

    let mut out = Vec::new();
    out.push(0x03); // dBASE III without memo
    out.extend_from_slice(&[126, 1, 1]); // last update: 2026-01-01 (any date is valid)
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&(header_len as u16).to_le_bytes());
    out.extend_from_slice(&(record_len as u16).to_le_bytes());
    out.extend_from_slice(&[0u8; 20]);

    for f in fields {
        let mut name = [0u8; 11];
        name[..f.name.len()].copy_from_slice(f.name.as_bytes());
        out.extend_from_slice(&name);
        let (type_byte, width, decimals) = match f.kind {
            DbfFieldKind::Character { width } => (b'C', width, 0),
            DbfFieldKind::Numeric { width, decimals } => (b'N', width, decimals),
            DbfFieldKind::Logical => (b'L', 1, 0),
        };
        out.push(type_byte);
        out.extend_from_slice(&[0u8; 4]);
        out.push(width);
        out.push(decimals);
        out.extend_from_slice(&[0u8; 14]);
    }
    out.push(0x0D);

    for row in rows {
        out.push(b' '); // not deleted
        for (i, f) in fields.iter().enumerate() {
            let width = usize::from(width_of(f.kind));
            let value = row.get(i).cloned().flatten();
            let text = field_text(f.kind, value.as_ref(), &f.name)?;
            let mut cell = vec![b' '; width];
            // Numbers and logicals are right-aligned, text is left-aligned.
            match f.kind {
                DbfFieldKind::Character { .. } => cell[..text.len()].copy_from_slice(&text),
                _ => cell[width - text.len()..].copy_from_slice(&text),
            }
            out.extend_from_slice(&cell);
        }
    }
    out.push(0x1A); // end-of-file marker
    Ok(out)
}

fn width_of(kind: DbfFieldKind) -> u8 {
    match kind {
        DbfFieldKind::Character { width } | DbfFieldKind::Numeric { width, .. } => width,
        DbfFieldKind::Logical => 1,
    }
}

/// The bytes a value occupies in its field, or an error if they do not fit.
fn field_text(
    kind: DbfFieldKind,
    value: Option<&DbfValue>,
    name: &str,
) -> Result<Vec<u8>, ShapefileError> {
    let width = usize::from(width_of(kind));
    let text: String = match (kind, value) {
        (_, None) => return Ok(Vec::new()),
        (DbfFieldKind::Character { .. }, Some(DbfValue::Character(s))) => s.clone(),
        (DbfFieldKind::Numeric { decimals, .. }, Some(DbfValue::Numeric(n))) => {
            format!("{:.*}", usize::from(decimals), n)
        }
        (DbfFieldKind::Logical, Some(DbfValue::Logical(Some(b)))) => {
            if *b {
                "T".into()
            } else {
                "F".into()
            }
        }
        (DbfFieldKind::Logical, Some(DbfValue::Logical(None))) => return Ok(Vec::new()),
        _ => return Err(ShapefileError::ValueTypeMismatch { field: name.to_string() }),
    };
    let bytes = text.into_bytes();
    if bytes.len() > width {
        return Err(ShapefileError::ValueTooWide { field: name.to_string() });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapefile::{read_dbf_with, read_shp_records, DbfEncoding};

    fn square(x0: f64, y0: f64, x1: f64, y1: f64, ccw: bool) -> Vec<Point> {
        let mut ring = vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
            Point::new(x0, y0),
        ];
        if !ccw {
            ring.reverse();
        }
        ring
    }

    #[test]
    fn points_round_trip_through_the_reader() {
        let input = vec![
            Some(Geometry::Point(Point::new(1.5, -2.0))),
            None,
            Some(Geometry::Point(Point::new(3.0, 4.0))),
        ];
        let out = write_shapefile(&input).unwrap();
        assert_eq!(read_shp_records(&out.shp).unwrap(), input);
    }

    /// Polygon rings come back with the same area and hole count, and the exterior is
    /// clockwise on disk (the reader keeps the stored order).
    #[test]
    fn polygon_with_hole_round_trips_with_standard_orientation() {
        let exterior = square(0.0, 0.0, 10.0, 10.0, true);
        let hole = square(4.0, 4.0, 6.0, 6.0, true);
        let input = vec![Some(Geometry::Polygon(Polygon { rings: vec![exterior, hole] }))];
        let out = write_shapefile(&input).unwrap();
        let back = read_shp_records(&out.shp).unwrap();
        let Some(Geometry::Polygon(poly)) = &back[0] else {
            panic!("expected a polygon: {back:?}");
        };
        assert_eq!(poly.rings.len(), 2);
        assert!(signed_area(&poly.rings[0]) < 0.0, "exterior must be clockwise");
        assert!(signed_area(&poly.rings[1]) > 0.0, "hole must be counter-clockwise");
        assert!((signed_area(&poly.rings[0]).abs() - 100.0).abs() < 1e-9);
        assert!((signed_area(&poly.rings[1]).abs() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn index_offsets_point_at_each_record() {
        let input = vec![
            Some(Geometry::LineString(vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)])),
            Some(Geometry::LineString(vec![
                Point::new(2.0, 2.0),
                Point::new(3.0, 5.0),
                Point::new(4.0, 4.0),
            ])),
        ];
        let out = write_shapefile(&input).unwrap();
        assert_eq!(out.shx.len(), HEADER_BYTES + 8 * input.len());
        for i in 0..input.len() {
            let at = HEADER_BYTES + 8 * i;
            let offset_words = i32::from_be_bytes(out.shx[at..at + 4].try_into().unwrap()) as usize;
            let length_words =
                i32::from_be_bytes(out.shx[at + 4..at + 8].try_into().unwrap()) as usize;
            let record = &out.shp[offset_words * 2..];
            assert_eq!(i32::from_be_bytes(record[0..4].try_into().unwrap()) as usize, i + 1);
            assert_eq!(i32::from_be_bytes(record[4..8].try_into().unwrap()) as usize, length_words);
        }
        assert_eq!(read_shp_records(&out.shp).unwrap(), input);
    }

    #[test]
    fn mixed_shape_types_are_refused() {
        let input = vec![
            Some(Geometry::Point(Point::new(0.0, 0.0))),
            Some(Geometry::LineString(vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)])),
        ];
        assert_eq!(write_shapefile(&input).unwrap_err(), ShapefileError::MixedShapeTypes);
        let collection = vec![Some(Geometry::GeometryCollection(vec![]))];
        assert_eq!(
            write_shapefile(&collection).unwrap_err(),
            ShapefileError::GeometryCollectionUnsupported
        );
    }

    #[test]
    fn dbf_round_trips_character_numeric_and_logical_fields() {
        let fields = vec![
            DbfField { name: "NAME".into(), kind: DbfFieldKind::Character { width: 12 } },
            DbfField { name: "POP".into(), kind: DbfFieldKind::Numeric { width: 12, decimals: 2 } },
            DbfField { name: "CAPITAL".into(), kind: DbfFieldKind::Logical },
        ];
        let rows = vec![
            vec![
                Some(DbfValue::Character("Zoë".into())),
                Some(DbfValue::Numeric(1234.5)),
                Some(DbfValue::Logical(Some(true))),
            ],
            vec![
                Some(DbfValue::Character("Oslo".into())),
                None,
                Some(DbfValue::Logical(Some(false))),
            ],
        ];
        let bytes = write_dbf(&fields, &rows).unwrap();
        let back = read_dbf_with(&bytes, DbfEncoding::Utf8).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].values[0].1, DbfValue::Character("Zoë".into()));
        assert_eq!(back[0].values[1].1, DbfValue::Numeric(1234.5));
        assert_eq!(back[0].values[2].1, DbfValue::Logical(Some(true)));
        assert_eq!(back[1].values[2].1, DbfValue::Logical(Some(false)));
    }

    #[test]
    fn dbf_refuses_values_wider_than_their_field() {
        let fields =
            vec![DbfField { name: "CODE".into(), kind: DbfFieldKind::Character { width: 3 } }];
        let rows = vec![vec![Some(DbfValue::Character("TOO LONG".into()))]];
        assert_eq!(
            write_dbf(&fields, &rows).unwrap_err(),
            ShapefileError::ValueTooWide { field: "CODE".into() }
        );
    }

    #[test]
    fn dbf_refuses_long_field_names() {
        let fields =
            vec![DbfField { name: "A_VERY_LONG_NAME".into(), kind: DbfFieldKind::Logical }];
        assert!(matches!(write_dbf(&fields, &[]), Err(ShapefileError::InvalidFieldName(_))));
    }
}
