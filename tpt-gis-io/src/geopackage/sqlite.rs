//! A minimal, pure-Rust SQLite 3 file-format reader and writer tailored to the
//! needs of GeoPackage.
//!
//! This is **not** a database engine: it does not run SQL, manage transactions, or
//! build indexes. It knows just enough of the on-disk b-tree format to read the
//! records of table leaf (and simple interior) b-trees and to write small tables
//! that consist of a single leaf page each. That is exactly what GeoPackage
//! feature tables and metadata tables look like at the sizes this crate targets.
//!
//! # Read support
//! - 100-byte database header (magic, page size, reserved bytes, text encoding).
//! - Varints and record serial types (integers of 1/2/3/4/6/8 bytes, IEEE-754
//!   floats, `NULL`, `TEXT`, `BLOB`).
//! - Leaf and interior table b-trees with overflow-page following for large payloads.
//! - `INTEGER PRIMARY KEY` is a rowid alias and is sourced from the cell's rowid
//!   rather than from the record body.
//!
//! # Write support (intentionally limited)
//! - One leaf page per table, no interior splitting, no freelist, no overflow pages.
//!   Each record (including its geometry BLOB) must therefore fit inline in a single
//!   page. This is fine for the modest feature sets GeoPackage round-trips here, but
//!   it is **not** a general-purpose SQLite writer.
//!
//! Only UTF-8 text encoding is supported (which is what the GeoPackage spec
//! mandates), so reading a UTF-16 database returns [`SqliteError::UnsupportedTextEncoding`].

use core::fmt;

/// The 16-byte magic string at the start of every SQLite database file.
pub const MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// A value stored in a single table column, as decoded from (or to be encoded into)
/// a SQLite record.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    /// A `NULL` value (serial type 0).
    Null,
    /// A signed integer (serial types 1–6, 8, 9).
    Integer(i64),
    /// An IEEE-754 double (serial type 7).
    Real(f64),
    /// A UTF-8 `TEXT` value (odd serial types ≥ 13).
    Text(String),
    /// A `BLOB` value (even serial types ≥ 12).
    Blob(Vec<u8>),
}

/// One column of a table's logical schema, as the reader/writer needs to know it.
#[derive(Debug, Clone)]
pub struct Column {
    /// The column's name (as declared in the `CREATE TABLE` statement).
    pub name: String,
    /// Whether this column is an `INTEGER PRIMARY KEY` rowid alias. Such a column is
    /// *not* stored in the record body; its value is the cell's rowid.
    pub is_rowid_alias: bool,
}

/// An error encountered while reading or writing the SQLite file format.
#[derive(Debug)]
pub struct SqliteError {
    kind: SqliteErrorKind,
}

#[derive(Debug)]
enum SqliteErrorKind {
    /// The file did not begin with the SQLite magic string.
    InvalidMagic,
    /// The database header declared an unsupported page size.
    UnsupportedPageSize(u16),
    /// A read ran past the end of the available bytes.
    UnexpectedEndOfInput,
    /// A payload claimed a length that overflowed an offset computation.
    OffsetOverflow,
    /// An overflow-page chain referenced page 0 or ran off the end of the file before
    /// the declared payload was fully assembled.
    TruncatedOverflow,
    /// A record used serial type 10 or 11, which the format reserves and never uses.
    ReservedSerialType(u64),
    /// A text value was not valid UTF-8 (only UTF-8 text encoding is supported).
    InvalidUtf8,
    /// The database text encoding was not UTF-8.
    UnsupportedTextEncoding(u32),
    /// A b-tree page was neither a leaf nor interior table page (e.g. an index page),
    /// which this reader does not walk.
    UnsupportedPageType(u8),
}

impl SqliteError {
    fn new(kind: SqliteErrorKind) -> Self {
        SqliteError { kind }
    }
}

impl fmt::Display for SqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            SqliteErrorKind::InvalidMagic => write!(f, "not a SQLite database (bad magic)"),
            SqliteErrorKind::UnsupportedPageSize(ps) => write!(f, "unsupported page size {ps}"),
            SqliteErrorKind::UnexpectedEndOfInput => write!(f, "unexpected end of SQLite input"),
            SqliteErrorKind::OffsetOverflow => write!(f, "SQLite offset computation overflowed"),
            SqliteErrorKind::TruncatedOverflow => {
                write!(f, "overflow-page payload was truncated")
            }
            SqliteErrorKind::ReservedSerialType(st) => {
                write!(f, "reserved record serial type {st}")
            }
            SqliteErrorKind::InvalidUtf8 => write!(f, "text value was not valid UTF-8"),
            SqliteErrorKind::UnsupportedTextEncoding(enc) => {
                write!(f, "unsupported text encoding {enc} (only UTF-8)")
            }
            SqliteErrorKind::UnsupportedPageType(t) => {
                write!(f, "unsupported b-tree page type {t:#04x}")
            }
        }
    }
}

impl std::error::Error for SqliteError {}

impl From<SqliteErrorKind> for SqliteError {
    fn from(kind: SqliteErrorKind) -> Self {
        SqliteError { kind }
    }
}

/// Information extracted from the 100-byte database header.
#[derive(Debug, Clone, Copy)]
pub struct DbInfo {
    /// The page size in bytes (the header value `1` means 65536).
    pub page_size: usize,
    /// Usable page size = `page_size - reserved`.
    pub usable: usize,
}

fn be_u16(bytes: &[u8], pos: usize) -> Result<u16, SqliteError> {
    let slice =
        bytes.get(pos..pos + 2).ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?;
    Ok(u16::from_be_bytes([slice[0], slice[1]]))
}

fn be_u32(bytes: &[u8], pos: usize) -> Result<u32, SqliteError> {
    let slice =
        bytes.get(pos..pos + 4).ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Parses the 100-byte database header.
///
/// # Errors
/// Returns [`SqliteError`] if the magic is wrong, the page size is unsupported, or
/// the text encoding is not UTF-8.
pub fn parse_db_header(db: &[u8]) -> Result<DbInfo, SqliteError> {
    if db.len() < 100 || db[0..16] != *MAGIC {
        return Err(SqliteError::new(SqliteErrorKind::InvalidMagic));
    }
    let ps_raw = be_u16(db, 16)?;
    let page_size = if ps_raw == 1 { 65536 } else { usize::from(ps_raw) };
    if !(512..=65536).contains(&page_size) || (page_size != 65536 && !page_size.is_power_of_two()) {
        return Err(SqliteError::new(SqliteErrorKind::UnsupportedPageSize(ps_raw)));
    }
    let reserved = db[20];
    let usable = page_size - usize::from(reserved);
    let text_encoding = be_u32(db, 56)?;
    if text_encoding != 1 {
        return Err(SqliteError::new(SqliteErrorKind::UnsupportedTextEncoding(text_encoding)));
    }
    Ok(DbInfo { page_size, usable })
}

/// Reads a big-endian SQLite varint (1–9 bytes) starting at `pos`.
///
/// # Errors
/// Returns [`SqliteError::UnexpectedEndOfInput`] if the varint runs past the buffer.
pub fn read_varint(bytes: &[u8], mut pos: usize) -> Result<(u64, usize), SqliteError> {
    let mut result: u64 = 0;
    for i in 0..9 {
        let byte =
            *bytes.get(pos).ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?;
        pos = pos.checked_add(1).ok_or(SqliteError::new(SqliteErrorKind::OffsetOverflow))?;
        if i == 8 {
            // The 9th byte contributes all 8 of its bits (no continuation bit).
            result = (result << 8) | u64::from(byte);
            break;
        }
        result = (result << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            break;
        }
    }
    Ok((result, pos))
}

/// Appends a big-endian SQLite varint encoding of `value` to `out`.
pub fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    if value < 0x80 {
        out.push(value as u8);
        return;
    }
    if value < 1 << 56 {
        // 1–8 bytes: each carries 7 bits with a continuation bit, most-significant
        // group first; the least-significant group carries no continuation bit.
        let mut stack = [0u8; 8];
        let mut n = 0usize;
        while value > 0 {
            stack[n] = (value & 0x7f) as u8;
            value >>= 7;
            n += 1;
        }
        for k in (1..n).rev() {
            out.push(stack[k] | 0x80);
        }
        out.push(stack[0]);
    } else {
        // 9-byte form: low 8 bits last, next 56 bits as 8 seven-bit groups.
        let low = (value & 0xff) as u8;
        let high = value >> 8;
        for k in (0..8).rev() {
            out.push(((high >> (7 * k)) & 0x7f) as u8 | 0x80);
        }
        out.push(low);
    }
}

/// The byte length a value of the given serial type occupies in the record body.
fn serial_type_len(serial: u64) -> Result<usize, SqliteError> {
    match serial {
        0 => Ok(0),
        1 => Ok(1),
        2 => Ok(2),
        3 => Ok(3),
        4 => Ok(4),
        5 => Ok(6),
        6 => Ok(8),
        7 => Ok(8),
        8 | 9 => Ok(0),
        10 | 11 => Err(SqliteError::new(SqliteErrorKind::ReservedSerialType(serial))),
        n if n >= 12 => Ok(((n - 12) / 2) as usize),
        _ => Err(SqliteError::new(SqliteErrorKind::ReservedSerialType(serial))),
    }
}

/// Decodes one column value of the given serial type from `payload` at `pos`.
fn decode_value(
    payload: &[u8],
    mut pos: usize,
    serial: u64,
) -> Result<(CellValue, usize), SqliteError> {
    match serial {
        0 => Ok((CellValue::Null, pos)),
        1..=6 => {
            let len = serial_type_len(serial)?;
            let bytes = payload
                .get(pos..pos + len)
                .ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?;
            let mut val: i64 = 0;
            for &b in bytes {
                val = (val << 8) | i64::from(b);
            }
            // Sign-extend from the value's width.
            let bits = len * 8;
            if bits < 64 {
                let shift = 64 - bits;
                val = (val << shift) >> shift;
            }
            pos = pos.checked_add(len).ok_or(SqliteError::new(SqliteErrorKind::OffsetOverflow))?;
            Ok((CellValue::Integer(val), pos))
        }
        7 => {
            let bytes = payload
                .get(pos..pos + 8)
                .ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?;
            let val = f64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            Ok((CellValue::Real(val), pos + 8))
        }
        8 => Ok((CellValue::Integer(0), pos)),
        9 => Ok((CellValue::Integer(1), pos)),
        10 | 11 => Err(SqliteError::new(SqliteErrorKind::ReservedSerialType(serial))),
        n if n >= 12 => {
            let len = ((n - 12) / 2) as usize;
            let data = payload
                .get(pos..pos + len)
                .ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?
                .to_vec();
            pos = pos.checked_add(len).ok_or(SqliteError::new(SqliteErrorKind::OffsetOverflow))?;
            if n % 2 == 0 {
                Ok((CellValue::Blob(data), pos))
            } else {
                let text = String::from_utf8(data)
                    .map_err(|_| SqliteError::new(SqliteErrorKind::InvalidUtf8))?;
                Ok((CellValue::Text(text), pos))
            }
        }
        _ => Err(SqliteError::new(SqliteErrorKind::ReservedSerialType(serial))),
    }
}

/// Parses a record body into values aligned to `columns`, filling any
/// `INTEGER PRIMARY KEY` alias column with the cell's `rowid`.
fn read_record(
    payload: &[u8],
    columns: &[Column],
    rowid: i64,
) -> Result<Vec<CellValue>, SqliteError> {
    let (header_len, mut pos) = read_varint(payload, 0)?;
    let header_end = usize::try_from(header_len)
        .map_err(|_| SqliteError::new(SqliteErrorKind::OffsetOverflow))?;

    let mut serial_types = Vec::with_capacity(columns.len());
    while pos < header_end {
        let (st, p) = read_varint(payload, pos)?;
        serial_types.push(st);
        pos = p;
    }
    if pos != header_end {
        return Err(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput));
    }

    let mut values = Vec::with_capacity(columns.len());
    let mut serial_iter = serial_types.into_iter();
    for column in columns {
        if column.is_rowid_alias {
            values.push(CellValue::Integer(rowid));
            continue;
        }
        let st =
            serial_iter.next().ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?;
        let (value, p) = decode_value(payload, pos, st)?;
        pos = p;
        values.push(value);
    }
    Ok(values)
}

/// Assembles a record payload (header + body) for the given stored column values.
///
/// `columns` and `values` are aligned and must already exclude any `INTEGER PRIMARY
/// KEY` alias column (whose value lives in the cell's rowid, not the record).
#[must_use]
pub fn encode_record(columns: &[Column], values: &[CellValue]) -> Vec<u8> {
    debug_assert_eq!(columns.len(), values.len());
    let mut header = Vec::new();
    let mut body = Vec::new();
    for (column, value) in columns.iter().zip(values) {
        debug_assert!(!column.is_rowid_alias, "alias column must not be stored");
        let (serial, bytes) = serial_for_value(value);
        write_varint(&mut header, serial);
        body.extend_from_slice(&bytes);
    }
    let header_len = header.len() + varint_len(header.len() as u64);
    let mut record = Vec::with_capacity(header_len + body.len());
    write_varint(&mut record, header_len as u64);
    record.extend_from_slice(&header);
    record.extend_from_slice(&body);
    record
}

/// The number of bytes `write_varint` will emit for `value`.
fn varint_len(mut value: u64) -> usize {
    if value < 0x80 {
        return 1;
    }
    if value < 1 << 56 {
        let mut n = 0;
        while value > 0 {
            value >>= 7;
            n += 1;
        }
        n
    } else {
        9
    }
}

/// Returns the (serial type, big-endian encoding) for a cell value.
fn serial_for_value(value: &CellValue) -> (u64, Vec<u8>) {
    match value {
        CellValue::Null => (0, Vec::new()),
        CellValue::Integer(n) => {
            let bytes8 = n.to_be_bytes();
            let (serial, len) = if *n >= i8::MIN as i64 && *n <= i8::MAX as i64 {
                (1u64, 1usize)
            } else if *n >= i16::MIN as i64 && *n <= i16::MAX as i64 {
                (2, 2)
            } else if *n >= -(1 << 23) && *n < 1 << 23 {
                (3, 3)
            } else if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 {
                (4, 4)
            } else if *n >= -(1i64 << 47) && *n < 1i64 << 47 {
                (5, 6)
            } else {
                (6, 8)
            };
            (serial, bytes8[8 - len..].to_vec())
        }
        CellValue::Real(f) => (7, f.to_be_bytes().to_vec()),
        CellValue::Text(s) => {
            let b = s.as_bytes();
            ((2 * b.len() + 13) as u64, b.to_vec())
        }
        CellValue::Blob(b) => ((2 * b.len() + 12) as u64, b.clone()),
    }
}

/// Reads the full payload of a leaf-table cell, following overflow pages if the
/// payload does not fit inline.
fn read_cell_payload(
    db: &[u8],
    info: &DbInfo,
    _cell_page_start: usize,
    payload_start: usize,
    payload_len: usize,
) -> Result<Vec<u8>, SqliteError> {
    let max_local = info.usable - 35; // leaf-table max inline payload
    if payload_len <= max_local {
        return db
            .get(payload_start..payload_start + payload_len)
            .ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))
            .map(|s| s.to_vec());
    }

    let min_local = ((info.usable - 12) * 32 / 255) - 23;
    let k = min_local + ((payload_len - min_local) % (info.usable - 4));
    let local = if k <= max_local { k } else { min_local };

    let mut out = Vec::with_capacity(payload_len);
    out.extend_from_slice(
        db.get(payload_start..payload_start + local)
            .ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?,
    );

    let mut next = be_u32(db, payload_start + local)?;
    let mut remaining = payload_len - local;
    while remaining > 0 {
        if next == 0 {
            return Err(SqliteError::new(SqliteErrorKind::TruncatedOverflow));
        }
        let ov_start = (usize::try_from(next)
            .map_err(|_| SqliteError::new(SqliteErrorKind::OffsetOverflow))?
            - 1)
            * info.page_size;
        let chunk = (info.usable - 4).min(remaining);
        let data = db
            .get(ov_start + 4..ov_start + 4 + chunk)
            .ok_or(SqliteError::new(SqliteErrorKind::TruncatedOverflow))?;
        out.extend_from_slice(data);
        next = be_u32(db, ov_start)?;
        remaining -= chunk;
    }
    Ok(out)
}

/// Recursively walks a table b-tree, appending `(rowid, values)` rows.
fn read_btree(
    db: &[u8],
    info: &DbInfo,
    page_number: u32,
    columns: &[Column],
    rows: &mut Vec<(i64, Vec<CellValue>)>,
) -> Result<(), SqliteError> {
    let page_start = (usize::try_from(page_number)
        .map_err(|_| SqliteError::new(SqliteErrorKind::OffsetOverflow))?
        - 1)
        * info.page_size;
    let header_offset = page_start + if page_number == 1 { 100 } else { 0 };
    let page_type =
        *db.get(header_offset).ok_or(SqliteError::new(SqliteErrorKind::UnexpectedEndOfInput))?;
    let is_interior = page_type == 0x05;
    let is_leaf = page_type == 0x0d;
    if !is_interior && !is_leaf {
        return Err(SqliteError::new(SqliteErrorKind::UnsupportedPageType(page_type)));
    }

    let num_cells = usize::from(be_u16(db, header_offset + 3)?);
    let ptr_array_start = header_offset + if is_interior { 12 } else { 8 };

    for i in 0..num_cells {
        let ptr = usize::from(be_u16(db, ptr_array_start + i * 2)?);
        let cell_offset = page_start + ptr;
        if is_interior {
            let left_child = be_u32(db, cell_offset)?;
            read_btree(db, info, left_child, columns, rows)?;
        } else {
            let (payload_len, p1) = read_varint(db, cell_offset)?;
            let (rowid, p2) = read_varint(db, p1)?;
            let payload = read_cell_payload(db, info, page_start, p2, payload_len as usize)?;
            let values = read_record(&payload, columns, rowid as i64)?;
            rows.push((rowid as i64, values));
        }
    }

    if is_interior {
        let right_child = be_u32(db, header_offset + 8)?;
        read_btree(db, info, right_child, columns, rows)?;
    }
    Ok(())
}

/// Reads every row of the table rooted at `rootpage` into `(rowid, values)` pairs
/// aligned to `columns`.
///
/// # Errors
/// Returns [`SqliteError`] on a malformed header, truncated record, or unsupported
/// page/text encoding.
pub fn read_table(
    db: &[u8],
    rootpage: u32,
    columns: &[Column],
) -> Result<Vec<(i64, Vec<CellValue>)>, SqliteError> {
    let info = parse_db_header(db)?;
    let mut rows = Vec::new();
    read_btree(db, &info, rootpage, columns, &mut rows)?;
    Ok(rows)
}

/// Builds a single leaf-table b-tree page containing `cells`.
///
/// `header_offset` is `100` for page 1 (the database header precedes the b-tree
/// header) and `0` for every other page. Each cell is `(rowid, record)`, and is written
/// on disk as `[payload-length varint][rowid varint][record]` — the format a SQLite
/// leaf table b-tree cell uses. The records must collectively fit in the page
/// alongside the header and cell-pointer array — callers (the writer) are responsible
/// for keeping tables small enough.
#[must_use]
pub fn build_leaf_page(
    page_size: usize,
    header_offset: usize,
    cells: &[(i64, Vec<u8>)],
) -> Vec<u8> {
    let mut page = vec![0u8; page_size];
    page[header_offset] = 0x0d; // leaf table
                                // first freeblock offset = 0
    page[header_offset + 1..header_offset + 3].copy_from_slice(&0u16.to_be_bytes());
    // number of cells
    page[header_offset + 3..header_offset + 5].copy_from_slice(&(cells.len() as u16).to_be_bytes());

    let mut cell_content_start = page_size;
    let mut ptr_off = header_offset + 8;
    for (rowid, record) in cells {
        // Build the on-disk cell: payload length, rowid, then the record body.
        let mut cell = Vec::with_capacity(2 * 9 + record.len());
        write_varint(&mut cell, record.len() as u64);
        write_varint(&mut cell, *rowid as u64);
        cell.extend_from_slice(record);
        cell_content_start -= cell.len();
        page[cell_content_start..cell_content_start + cell.len()].copy_from_slice(&cell);
        page[ptr_off..ptr_off + 2].copy_from_slice(&(cell_content_start as u16).to_be_bytes());
        ptr_off += 2;
    }
    // cell content area start
    page[header_offset + 5..header_offset + 7]
        .copy_from_slice(&(cell_content_start as u16).to_be_bytes());
    // fragmented free bytes = 0
    page[header_offset + 7] = 0;
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trips_small() {
        for v in [0u64, 1, 127, 128, 200, 16383, 16384, 1 << 56, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let (decoded, pos) = read_varint(&buf, 0).unwrap();
            assert_eq!(decoded, v, "varint mismatch for {v}");
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn nine_byte_varint_length_is_nine() {
        let mut buf = Vec::new();
        write_varint(&mut buf, u64::MAX);
        assert_eq!(buf.len(), 9);
    }

    #[test]
    fn serial_type_len_known_values() {
        assert_eq!(serial_type_len(0).unwrap(), 0);
        assert_eq!(serial_type_len(1).unwrap(), 1);
        assert_eq!(serial_type_len(5).unwrap(), 6);
        assert_eq!(serial_type_len(6).unwrap(), 8);
        assert_eq!(serial_type_len(7).unwrap(), 8);
        assert_eq!(serial_type_len(8).unwrap(), 0);
        assert_eq!(serial_type_len(13).unwrap(), 0); // (13-13)/2 = 0-char text
        assert_eq!(serial_type_len(14).unwrap(), 1); // 1-char text
        assert_eq!(serial_type_len(12).unwrap(), 0); // 0-byte blob
        assert_eq!(serial_type_len(100).unwrap(), 44); // (100-12)/2
        assert!(serial_type_len(10).is_err());
    }

    #[test]
    fn record_round_trips_values() {
        let columns = [
            Column { name: "id".into(), is_rowid_alias: true },
            Column { name: "name".into(), is_rowid_alias: false },
            Column { name: "count".into(), is_rowid_alias: false },
            Column { name: "ratio".into(), is_rowid_alias: false },
            Column { name: "blob".into(), is_rowid_alias: false },
        ];
        let stored = &columns[1..];
        let values = vec![
            CellValue::Text("hello".into()),
            CellValue::Integer(42),
            CellValue::Real(3.5),
            CellValue::Blob(vec![1, 2, 3]),
        ];
        let payload = encode_record(stored, &values);
        let decoded = read_record(&payload, &columns, 7).unwrap();
        assert_eq!(decoded[0], CellValue::Integer(7)); // alias filled from rowid
        assert_eq!(decoded[1], CellValue::Text("hello".into()));
        assert_eq!(decoded[2], CellValue::Integer(42));
        assert_eq!(decoded[3], CellValue::Real(3.5));
        assert_eq!(decoded[4], CellValue::Blob(vec![1, 2, 3]));
    }

    #[test]
    fn leaf_page_parses_back() {
        let page_size = 512;
        let columns = [Column { name: "v".into(), is_rowid_alias: false }];
        let mut cells = Vec::new();
        for i in 1..=5u64 {
            let payload = encode_record(&columns, &[CellValue::Integer(i64::try_from(i).unwrap())]);
            cells.push((i64::try_from(i).unwrap(), payload));
        }
        // Build a page-1 leaf (header at offset 100, preceded by a 100-byte DB header).
        // `build_leaf_page` already places the b-tree header at index `header_offset`
        // within a full page-sized buffer, so the buffer is the whole page-1 file.
        let mut page = build_leaf_page(page_size, 100, &cells);
        assert_eq!(page[100], 0x0d); // leaf table page type lives at offset 100
                                     // Overwrite the first 100 bytes with a minimal valid DB header.
        page[0..16].copy_from_slice(MAGIC);
        page[16..18].copy_from_slice(&(page_size as u16).to_be_bytes());
        page[56..60].copy_from_slice(&1u32.to_be_bytes()); // UTF-8
        let rows = read_table(&page, 1, &columns).unwrap();
        let vals: Vec<i64> = rows
            .iter()
            .map(|(_, v)| match &v[0] {
                CellValue::Integer(n) => *n,
                _ => 0,
            })
            .collect();
        assert_eq!(vals, vec![1, 2, 3, 4, 5]);
    }
}
