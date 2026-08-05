//! ESRI Shapefile (`.shp` + `.dbf`) reader.
//!
//! This module reads geometry from the `.shp` *main file* and attributes from the
//! `.dbf` *attribute file*. It is deliberately **read-only** (see `todo.md`: the
//! scoped feature is "Shapefile (.shp/.shx/.dbf) reader") and has two further scope
//! limitations worth calling out prominently:
//!
//! - **`.shx` (the index file) is not read at all.** The `.shx` file is a random-access
//!   optimization: for each record it stores an (offset, length) pair into the `.shp`
//!   file so a reader can jump directly to record *n* without scanning everything
//!   before it. A sequential reader doesn't need that: each `.shp` record already
//!   carries its own content-length header, so [`read_shp`] just walks the file
//!   front-to-back, and the result is byte-for-byte identical to what `.shx`-assisted
//!   random access would produce. If random access is ever needed, `.shx` support can
//!   be added without changing anything here.
//! - **Multi-outer-ring polygons are not detected.** The Shapefile spec allows a single
//!   `Polygon` record to encode more than one *disjoint* exterior ring (e.g. a country
//!   with islands), distinguishing exterior rings from holes by winding order (clockwise
//!   = exterior, counterclockwise = hole, by the spec's convention) and point-in-polygon
//!   containment tests to know which exterior a given hole belongs to. Implementing that
//!   grouping correctly is a non-trivial algorithm in its own right and easy to get
//!   subtly wrong under time pressure, so it has been left out of this first pass.
//!   Instead, [`parse_polygon_record`] treats **ring 0 (file order) as the exterior and
//!   every remaining ring as an interior (hole) of a single [`Geometry::Polygon`]**,
//!   regardless of winding order. This is correct for the overwhelmingly common case of
//!   one exterior ring plus holes, but will misrepresent a genuinely multi-part polygon
//!   (it will be read as one polygon with extra "holes" rather than a `MultiPolygon`).
//!   A future pass adding winding-order-aware grouping should replace this function.
//!
//! Z/M-flavored shape types (`PointZ`, `PolyLineZ`, `PolygonZ`, `MultiPointZ`, and the
//! `*M` variants) are not supported; [`read_shp`] returns
//! [`ShapefileError::UnsupportedShapeType`] rather than misinterpret their trailing
//! Z/M arrays as more X/Y points.

use core::fmt;

use crate::geometry::{Geometry, Point, Polygon};

/// An error encountered while reading a `.shp` or `.dbf` file.
#[derive(Debug)]
pub enum ShapefileError {
    /// The `.shp` file header's File Code (bytes 0-3) was not `9994`.
    InvalidFileCode(i32),
    /// A shape type code was encountered that this reader does not support (either a
    /// Z/M variant, or a value outside the spec entirely).
    UnsupportedShapeType(i32),
    /// A field type byte in a `.dbf` field descriptor was not one of the supported
    /// codes (`C`, `N`, `F`, `L`, `D`).
    InvalidFieldType(u8),
    /// A numeric (`N`/`F`) `.dbf` field's text could not be parsed as an `f64`.
    InvalidNumericField(String),
    /// A date (`D`) `.dbf` field's text was not 8 ASCII digits.
    InvalidDateField(String),
    /// The input ended before a length-prefixed or fixed-size structure it was in the
    /// middle of reading was complete.
    UnexpectedEndOfInput,
}

impl fmt::Display for ShapefileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShapefileError::InvalidFileCode(code) => {
                write!(f, "invalid shapefile file code: expected 9994, got {code}")
            }
            ShapefileError::UnsupportedShapeType(code) => {
                write!(f, "unsupported shape type: {code}")
            }
            ShapefileError::InvalidFieldType(byte) => {
                write!(f, "invalid dbf field type byte: {byte:#04x}")
            }
            ShapefileError::InvalidNumericField(text) => {
                write!(f, "invalid dbf numeric field text: {text:?}")
            }
            ShapefileError::InvalidDateField(text) => {
                write!(f, "invalid dbf date field text: {text:?}")
            }
            ShapefileError::UnexpectedEndOfInput => {
                write!(f, "unexpected end of input")
            }
        }
    }
}

impl std::error::Error for ShapefileError {}

// ---------------------------------------------------------------------------------
// .shp
// ---------------------------------------------------------------------------------

/// Shape type codes from the Shapefile spec that this reader understands.
mod shape_type {
    pub(super) const NULL: i32 = 0;
    pub(super) const POINT: i32 = 1;
    pub(super) const POLYLINE: i32 = 3;
    pub(super) const POLYGON: i32 = 5;
    pub(super) const MULTIPOINT: i32 = 8;
}

/// A small cursor over a byte slice, tracking a read position.
///
/// Kept private to this module: it exists purely to make the field-by-field parsing
/// below read linearly instead of threading an index through every call.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ShapefileError> {
        let end = self.pos.checked_add(len).ok_or(ShapefileError::UnexpectedEndOfInput)?;
        let slice = self.bytes.get(self.pos..end).ok_or(ShapefileError::UnexpectedEndOfInput)?;
        self.pos = end;
        Ok(slice)
    }

    fn i32_be(&mut self) -> Result<i32, ShapefileError> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i32_le(&mut self) -> Result<i32, ShapefileError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f64_le(&mut self) -> Result<f64, ShapefileError> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn skip(&mut self, len: usize) -> Result<(), ShapefileError> {
        self.take(len).map(|_| ())
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    /// The number of bytes left between the current position and the end of the
    /// buffer. Used to bound count-derived `Vec` allocations against the actual
    /// available input.
    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
}

/// Reads an (x, y) pair, little-endian, in the order they appear in every shape
/// record's points array.
fn read_xy(cursor: &mut Cursor<'_>) -> Result<Point, ShapefileError> {
    let x = cursor.f64_le()?;
    let y = cursor.f64_le()?;
    Ok(Point::new(x, y))
}

/// Reads the `NumParts`/`NumPoints`/`Parts[]`/`Points[]` block shared by the
/// `PolyLine` and `Polygon` shape types, returning each part as its own `Vec<Point>`.
fn read_parts_and_points(cursor: &mut Cursor<'_>) -> Result<Vec<Vec<Point>>, ShapefileError> {
    // Box (Xmin, Ymin, Xmax, Ymax), unused by this reader.
    cursor.skip(32)?;

    let num_parts = usize_from_i32(cursor.i32_le()?)?;
    let num_points = usize_from_i32(cursor.i32_le()?)?;

    // Bound the count-derived allocations to what can plausibly fit in the remaining
    // input. A hostile `NumParts`/`NumPoints` would otherwise make `Vec::with_capacity`
    // allocate an unbounded amount of memory before any further validation runs.
    let rest = cursor.remaining();
    let needed = num_parts
        .checked_mul(4)
        .and_then(|parts_bytes| {
            num_points.checked_mul(16).and_then(|pts_bytes| parts_bytes.checked_add(pts_bytes))
        })
        .ok_or(ShapefileError::UnexpectedEndOfInput)?;
    if needed > rest {
        return Err(ShapefileError::UnexpectedEndOfInput);
    }

    let mut part_starts = Vec::with_capacity(num_parts);
    for _ in 0..num_parts {
        part_starts.push(usize_from_i32(cursor.i32_le()?)?);
    }

    let mut points = Vec::with_capacity(num_points);
    for _ in 0..num_points {
        points.push(read_xy(cursor)?);
    }

    let mut parts = Vec::with_capacity(num_parts);
    for (i, &start) in part_starts.iter().enumerate() {
        let end = part_starts.get(i + 1).copied().unwrap_or(num_points);
        let part = points.get(start..end).ok_or(ShapefileError::UnexpectedEndOfInput)?;
        parts.push(part.to_vec());
    }
    Ok(parts)
}

fn usize_from_i32(value: i32) -> Result<usize, ShapefileError> {
    usize::try_from(value).map_err(|_| ShapefileError::UnexpectedEndOfInput)
}

/// Parses a `PolyLine` (shape type `3`) record's content (after its own 4-byte shape
/// type field has already been consumed) into a `LineString` or `MultiLineString`.
fn parse_polyline_record(cursor: &mut Cursor<'_>) -> Result<Geometry, ShapefileError> {
    let mut parts = read_parts_and_points(cursor)?;
    if parts.len() == 1 {
        Ok(Geometry::LineString(parts.pop().unwrap_or_default()))
    } else {
        Ok(Geometry::MultiLineString(parts))
    }
}

/// Parses a `Polygon` (shape type `5`) record's content (after its own 4-byte shape
/// type field has already been consumed) into a [`Geometry::Polygon`].
///
/// See the module-level doc comment for the scope limitation this function
/// implements: ring 0 is always treated as the exterior, and every other ring as a
/// hole, with no winding-order-based detection of multiple disjoint exteriors.
fn parse_polygon_record(cursor: &mut Cursor<'_>) -> Result<Geometry, ShapefileError> {
    let rings = read_parts_and_points(cursor)?;
    Ok(Geometry::Polygon(Polygon { rings }))
}

/// Parses a `MultiPoint` (shape type `8`) record's content (after its own 4-byte shape
/// type field has already been consumed) into a [`Geometry::MultiPoint`].
fn parse_multipoint_record(cursor: &mut Cursor<'_>) -> Result<Geometry, ShapefileError> {
    // Box, unused.
    cursor.skip(32)?;
    let num_points = usize_from_i32(cursor.i32_le()?)?;
    // Bound the point allocation to the remaining input (16 bytes per point).
    if num_points > cursor.remaining() / 16 {
        return Err(ShapefileError::UnexpectedEndOfInput);
    }
    let mut points = Vec::with_capacity(num_points);
    for _ in 0..num_points {
        points.push(read_xy(cursor)?);
    }
    Ok(Geometry::MultiPoint(points))
}

/// Reads every geometry record from a `.shp` file's raw bytes.
///
/// `Null Shape` records (shape type `0`) contribute nothing to the returned `Vec`
/// (they carry no geometry) — the returned list may therefore be shorter than the
/// record count in the file. This is a straightforward, sequential reader: see the
/// module-level doc comment for why `.shx` is not needed and for the polygon
/// multi-exterior-ring scope limitation.
///
/// # Errors
/// Returns [`ShapefileError::InvalidFileCode`] if the header's file code isn't 9994,
/// [`ShapefileError::UnsupportedShapeType`] for Z/M or unrecognized shape types, and
/// [`ShapefileError::UnexpectedEndOfInput`] if the file is truncated.
pub fn read_shp(bytes: &[u8]) -> Result<Vec<Geometry>, ShapefileError> {
    let mut header = Cursor::new(bytes);

    let file_code = header.i32_be()?;
    if file_code != 9994 {
        return Err(ShapefileError::InvalidFileCode(file_code));
    }
    // Bytes 4-23: five unused int32s.
    header.skip(20)?;
    // Bytes 24-27: file length (in 16-bit words); not needed for sequential reading.
    header.skip(4)?;
    // Bytes 28-31: version.
    header.skip(4)?;
    // Bytes 32-35: the header's own shape type field is descriptive only (it names
    // the single shape type used throughout the file) and is not needed to parse
    // records, since each record repeats its own shape type.
    header.skip(4)?;
    // Bytes 36-67: Xmin/Ymin/Xmax/Ymax bounding box.
    // Bytes 68-99: Zmin/Zmax/Mmin/Mmax, unused for 2D.
    header.skip(64)?;
    debug_assert_eq!(header.pos, 100, "shp header is exactly 100 bytes");

    let mut cursor = header;
    let mut geometries = Vec::new();

    while !cursor.is_at_end() {
        // Record header: big-endian record number (unused) then big-endian content
        // length in 16-bit words.
        cursor.skip(4)?;
        let content_words = cursor.i32_be()?;
        let content_bytes = usize_from_i32(content_words)?
            .checked_mul(2)
            .ok_or(ShapefileError::UnexpectedEndOfInput)?;

        let record_bytes = cursor.take(content_bytes)?;
        let mut record = Cursor::new(record_bytes);
        let shape_type = record.i32_le()?;

        let geometry = match shape_type {
            shape_type::NULL => None,
            shape_type::POINT => Some(Geometry::Point(read_xy(&mut record)?)),
            shape_type::POLYLINE => Some(parse_polyline_record(&mut record)?),
            shape_type::POLYGON => Some(parse_polygon_record(&mut record)?),
            shape_type::MULTIPOINT => Some(parse_multipoint_record(&mut record)?),
            other => return Err(ShapefileError::UnsupportedShapeType(other)),
        };
        if let Some(geometry) = geometry {
            geometries.push(geometry);
        }
    }

    Ok(geometries)
}

// ---------------------------------------------------------------------------------
// .dbf
// ---------------------------------------------------------------------------------

/// A single `.dbf` attribute value, tagged by its field type.
#[derive(Debug, Clone, PartialEq)]
pub enum DbfValue {
    /// A `'C'` (Character) field: text with trailing padding spaces trimmed.
    Character(String),
    /// An `'N'` (Numeric) or `'F'` (Float) field, parsed from its ASCII text.
    Numeric(f64),
    /// An `'L'` (Logical) field: `Some(true)`/`Some(false)` for `T`/`F`-family
    /// characters, `None` for the dBase "unknown" placeholder (`?`, or unrecognized).
    Logical(Option<bool>),
    /// A `'D'` (Date) field, parsed from its 8-digit `YYYYMMDD` text.
    Date {
        /// Four-digit year.
        year: u16,
        /// Month, `1..=12`.
        month: u8,
        /// Day of month, `1..=31`.
        day: u8,
    },
}

/// One `.dbf` record: its deletion flag plus field values in field-descriptor order.
#[derive(Debug, Clone, PartialEq)]
pub struct DbfRecord {
    /// `true` if the record's deletion flag byte was `0x2A` ("deleted" in dBase's
    /// soft-delete convention). Deleted records are still returned (not filtered out)
    /// so callers can decide whether to honor the flag.
    pub is_deleted: bool,
    /// Field values, `(field name, value)`, in the same order as the `.dbf`'s field
    /// descriptors.
    pub values: Vec<(String, DbfValue)>,
}

struct FieldDescriptor {
    name: String,
    field_type: u8,
    length: usize,
}

fn trimmed_ascii(bytes: &[u8]) -> String {
    // Lossy rather than strict: real-world `.dbf` Character fields are frequently not
    // valid UTF-8 (legacy code pages, etc.). `from_utf8_lossy` replaces bad bytes with
    // U+FFFD instead of silently producing an empty string.
    String::from_utf8_lossy(bytes).trim().to_string()
}

/// Parses one field's raw fixed-width text into a [`DbfValue`], per its descriptor's
/// type byte.
///
/// # Errors
/// Returns [`ShapefileError::InvalidNumericField`] if an `N`/`F` field's text doesn't
/// parse as an `f64`, [`ShapefileError::InvalidDateField`] if a `D` field's text isn't
/// 8 ASCII digits, and [`ShapefileError::InvalidFieldType`] for an unsupported type byte.
fn parse_field_value(field_type: u8, raw: &[u8]) -> Result<DbfValue, ShapefileError> {
    match field_type {
        b'C' => Ok(DbfValue::Character(trimmed_ascii(raw))),
        b'N' | b'F' => {
            let text = trimmed_ascii(raw);
            if text.is_empty() {
                // A blank numeric field (common for null/unset values in real-world
                // data) parses to 0.0 rather than erroring.
                return Ok(DbfValue::Numeric(0.0));
            }
            text.parse::<f64>()
                .map(DbfValue::Numeric)
                .map_err(|_| ShapefileError::InvalidNumericField(text.to_string()))
        }
        b'L' => {
            let ch = raw.first().copied().unwrap_or(b'?');
            Ok(DbfValue::Logical(match ch {
                b'T' | b't' | b'Y' | b'y' => Some(true),
                b'F' | b'f' | b'N' | b'n' => Some(false),
                _ => None,
            }))
        }
        b'D' => {
            let text = trimmed_ascii(raw);
            if text.len() != 8 || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ShapefileError::InvalidDateField(text.to_string()));
            }
            let year = text[0..4]
                .parse()
                .map_err(|_| ShapefileError::InvalidDateField(text.to_string()))?;
            let month = text[4..6]
                .parse()
                .map_err(|_| ShapefileError::InvalidDateField(text.to_string()))?;
            let day = text[6..8]
                .parse()
                .map_err(|_| ShapefileError::InvalidDateField(text.to_string()))?;
            Ok(DbfValue::Date { year, month, day })
        }
        other => Err(ShapefileError::InvalidFieldType(other)),
    }
}

/// Reads every attribute record from a `.dbf` file's raw bytes.
///
/// Only field types `C`, `N`, `F`, `L`, and `D` are supported; any other type byte in
/// a field descriptor causes this to return [`ShapefileError::InvalidFieldType`]
/// rather than silently dropping or misreading the field.
///
/// Records are returned regardless of their deletion flag (see
/// [`DbfRecord::is_deleted`]) — callers who want the common "skip deleted records"
/// behavior can `.filter(|r| !r.is_deleted)` themselves.
///
/// # Errors
/// Returns [`ShapefileError::UnexpectedEndOfInput`] if the file is truncated, plus the
/// field-parsing errors described above.
pub fn read_dbf(bytes: &[u8]) -> Result<Vec<DbfRecord>, ShapefileError> {
    let mut cursor = Cursor::new(bytes);

    // Byte 0: version (ignored). Bytes 1-3: date of last update (ignored).
    cursor.skip(4)?;
    let num_records = usize_from_i32(cursor.i32_le()?)?;
    let header_bytes = usize::from(u16_le(cursor.take(2)?));
    let record_bytes = usize::from(u16_le(cursor.take(2)?));
    // Bytes 12-31: reserved.
    cursor.skip(20)?;
    debug_assert_eq!(cursor.pos, 32, "dbf header is exactly 32 bytes");

    let mut fields = Vec::new();
    loop {
        // A single terminator byte (0x0D) ends the field descriptor array; peek at it
        // without consuming the rest of a descriptor.
        let marker = cursor.take(1)?[0];
        if marker == 0x0D {
            break;
        }
        // The name's first byte was already consumed as `marker`; read the remaining
        // 10 name bytes plus the rest of the 32-byte descriptor.
        let mut name_bytes = vec![marker];
        name_bytes.extend_from_slice(cursor.take(10)?);
        let field_type = cursor.take(1)?[0];
        // Bytes 12-15: field data address (ignored).
        cursor.skip(4)?;
        let length = usize::from(cursor.take(1)?[0]);
        // Byte 17: decimal count (informational only — `str::parse::<f64>` handles
        // the decimal point in the field text without needing this).
        // Bytes 18-31: reserved.
        cursor.skip(1 + 14)?;

        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).trim().to_string();

        // Fail fast on an unsupported field type here (rather than only when a
        // record's value is parsed) so a malformed schema is reported immediately.
        if !matches!(field_type, b'C' | b'N' | b'F' | b'L' | b'D') {
            return Err(ShapefileError::InvalidFieldType(field_type));
        }

        fields.push(FieldDescriptor { name, field_type, length });
    }

    // The header declares its own total byte count; if the field descriptors (plus
    // the 32-byte header and 1-byte terminator already consumed) don't add up to it,
    // trust the file's stated header length and jump to it, so any vendor padding
    // between the terminator and the first record is skipped correctly.
    if header_bytes > cursor.pos {
        cursor.skip(header_bytes - cursor.pos)?;
    }

    // Bound the `records` allocation to what the remaining file bytes can actually
    // hold. A hostile `num_records` (a full `i32`, e.g. 2^31) would otherwise make
    // `Vec::with_capacity` attempt an unbounded allocation.
    let records_capacity =
        bytes.len().saturating_sub(cursor.pos).checked_div(record_bytes.max(1)).unwrap_or(0);
    let mut records = Vec::with_capacity(num_records.min(records_capacity));
    for _ in 0..num_records {
        let record_start = cursor.pos;
        let deletion_flag = cursor.take(1)?[0];
        let is_deleted = deletion_flag == 0x2A;

        let mut values = Vec::with_capacity(fields.len());
        for field in &fields {
            let raw = cursor.take(field.length)?;
            let value = parse_field_value(field.field_type, raw)?;
            values.push((field.name.clone(), value));
        }

        // Advance past any trailing bytes in the record that the field descriptors
        // didn't account for, so the next record starts at the right offset even if
        // `record_bytes` is padded beyond the sum of field lengths.
        let consumed = cursor.pos - record_start;
        if record_bytes > consumed {
            cursor.skip(record_bytes - consumed)?;
        }

        records.push(DbfRecord { is_deleted, values });
    }

    Ok(records)
}

/// An iterator over the geometry records in a `.shp` file, yielding one
/// [`Geometry`] at a time without collecting the whole file into memory.
///
/// This is the streaming counterpart to [`read_shp`]. `Null Shape` records are
/// skipped (they carry no geometry), so the iterator may yield fewer items than
/// the record count in the file.
///
/// # Errors
/// Iterator items are `Result<Geometry, ShapefileError>`; parsing stops on the
/// first error.
pub struct ShpGeometryIter<'a> {
    cursor: Cursor<'a>,
    finished: bool,
}

impl<'a> ShpGeometryIter<'a> {
    /// Creates a new iterator from raw `.shp` bytes.
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        let mut cursor = Cursor::new(bytes);
        let finished = false;
        let _ = skip_shp_header(&mut cursor);
        Self { cursor, finished }
    }
}

impl<'a> Iterator for ShpGeometryIter<'a> {
    type Item = Result<Geometry, ShapefileError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.finished || self.cursor.is_at_end() {
                return None;
            }
            match read_next_shp_record(&mut self.cursor) {
                // A real geometry: yield it and remember whether we're now at EOF.
                Ok(Some(geometry)) => {
                    self.finished = self.cursor.is_at_end();
                    return Some(Ok(geometry));
                }
                // A `Null Shape` record carries no geometry; skip it and keep going.
                Ok(None) => continue,
                // Any parse error stops iteration and is surfaced to the caller.
                Err(e) => {
                    self.finished = true;
                    return Some(Err(e));
                }
            }
        }
    }
}

fn skip_shp_header(cursor: &mut Cursor<'_>) -> Result<(), ShapefileError> {
    let file_code = cursor.i32_be()?;
    if file_code != 9994 {
        return Err(ShapefileError::InvalidFileCode(file_code));
    }
    cursor.skip(20)?;
    cursor.skip(4)?;
    cursor.skip(4)?;
    cursor.skip(4)?;
    cursor.skip(64)?;
    Ok(())
}

fn read_next_shp_record(cursor: &mut Cursor<'_>) -> Result<Option<Geometry>, ShapefileError> {
    if cursor.is_at_end() {
        return Ok(None);
    }
    cursor.skip(4)?;
    let content_words = cursor.i32_be()?;
    let content_bytes = usize_from_i32(content_words)?
        .checked_mul(2)
        .ok_or(ShapefileError::UnexpectedEndOfInput)?;
    let record_bytes = cursor.take(content_bytes)?;
    let mut record = Cursor::new(record_bytes);
    let shape_type = record.i32_le()?;
    let geometry = match shape_type {
        shape_type::NULL => None,
        shape_type::POINT => Some(Geometry::Point(read_xy(&mut record)?)),
        shape_type::POLYLINE => Some(parse_polyline_record(&mut record)?),
        shape_type::POLYGON => Some(parse_polygon_record(&mut record)?),
        shape_type::MULTIPOINT => Some(parse_multipoint_record(&mut record)?),
        other => return Err(ShapefileError::UnsupportedShapeType(other)),
    };
    Ok(geometry)
}
fn u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes.try_into().unwrap_or([0, 0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shp_header(shape_type: i32) -> Vec<u8> {
        let mut header = Vec::with_capacity(100);
        header.extend_from_slice(&9994i32.to_be_bytes());
        header.extend_from_slice(&[0u8; 20]);
        header.extend_from_slice(&0i32.to_be_bytes());
        header.extend_from_slice(&1000i32.to_le_bytes());
        header.extend_from_slice(&shape_type.to_le_bytes());
        header.extend_from_slice(&0.0f64.to_le_bytes());
        header.extend_from_slice(&0.0f64.to_le_bytes());
        header.extend_from_slice(&0.0f64.to_le_bytes());
        header.extend_from_slice(&0.0f64.to_le_bytes());
        header.extend_from_slice(&[0u8; 32]);
        assert_eq!(header.len(), 100);
        header
    }

    fn push_record(buf: &mut Vec<u8>, record_number: i32, content: &[u8]) {
        assert_eq!(content.len() % 2, 0, "record content must be a whole number of 16-bit words");
        buf.extend_from_slice(&record_number.to_be_bytes());
        buf.extend_from_slice(&(i32::try_from(content.len() / 2).unwrap()).to_be_bytes());
        buf.extend_from_slice(content);
    }

    #[test]
    fn reads_single_point_record() {
        let mut bytes = shp_header(1);
        let mut content = Vec::new();
        content.extend_from_slice(&1i32.to_le_bytes());
        content.extend_from_slice(&12.5f64.to_le_bytes());
        content.extend_from_slice(&(-34.25f64).to_le_bytes());
        push_record(&mut bytes, 1, &content);

        let geometries = read_shp(&bytes).unwrap();
        assert_eq!(geometries, vec![Geometry::Point(Point::new(12.5, -34.25))]);
    }

    #[test]
    fn reads_multi_part_polyline_as_multi_line_string() {
        let mut bytes = shp_header(3);

        let part_a = [Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
        let part_b = [Point::new(5.0, 5.0), Point::new(6.0, 5.0), Point::new(6.0, 6.0)];
        let num_points = part_a.len() + part_b.len();

        let mut content = Vec::new();
        content.extend_from_slice(&3i32.to_le_bytes());
        content.extend_from_slice(&[0u8; 32]);
        content.extend_from_slice(&2i32.to_le_bytes());
        content.extend_from_slice(&(i32::try_from(num_points).unwrap()).to_le_bytes());
        content.extend_from_slice(&0i32.to_le_bytes());
        content.extend_from_slice(&(i32::try_from(part_a.len()).unwrap()).to_le_bytes());
        for p in part_a.iter().chain(part_b.iter()) {
            content.extend_from_slice(&p.x.to_le_bytes());
            content.extend_from_slice(&p.y.to_le_bytes());
        }
        push_record(&mut bytes, 1, &content);

        let geometries = read_shp(&bytes).unwrap();
        assert_eq!(geometries.len(), 1);
        match &geometries[0] {
            Geometry::MultiLineString(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0], part_a);
                assert_eq!(parts[1], part_b);
            }
            other => panic!("expected MultiLineString, got {other:?}"),
        }
    }

    #[test]
    fn reads_polygon_with_hole() {
        let mut bytes = shp_header(5);

        let exterior = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let hole = [
            Point::new(2.0, 2.0),
            Point::new(4.0, 2.0),
            Point::new(2.0, 4.0),
            Point::new(2.0, 2.0),
        ];
        let num_points = exterior.len() + hole.len();

        let mut content = Vec::new();
        content.extend_from_slice(&5i32.to_le_bytes());
        content.extend_from_slice(&[0u8; 32]);
        content.extend_from_slice(&2i32.to_le_bytes());
        content.extend_from_slice(&(i32::try_from(num_points).unwrap()).to_le_bytes());
        content.extend_from_slice(&0i32.to_le_bytes());
        content.extend_from_slice(&(i32::try_from(exterior.len()).unwrap()).to_le_bytes());
        for p in exterior.iter().chain(hole.iter()) {
            content.extend_from_slice(&p.x.to_le_bytes());
            content.extend_from_slice(&p.y.to_le_bytes());
        }
        push_record(&mut bytes, 1, &content);

        let geometries = read_shp(&bytes).unwrap();
        assert_eq!(geometries.len(), 1);
        match &geometries[0] {
            Geometry::Polygon(polygon) => {
                assert_eq!(polygon.rings.len(), 2);
                assert_eq!(polygon.rings[0].len(), exterior.len());
                assert_eq!(polygon.rings[1].len(), hole.len());
                assert_eq!(polygon.rings[0], exterior);
                assert_eq!(polygon.rings[1], hole);
            }
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    #[test]
    fn reads_multipoint_record() {
        let mut bytes = shp_header(8);
        let points = [Point::new(1.0, 2.0), Point::new(3.0, 4.0), Point::new(5.0, 6.0)];

        let mut content = Vec::new();
        content.extend_from_slice(&8i32.to_le_bytes());
        content.extend_from_slice(&[0u8; 32]);
        content.extend_from_slice(&(i32::try_from(points.len()).unwrap()).to_le_bytes());
        for p in &points {
            content.extend_from_slice(&p.x.to_le_bytes());
            content.extend_from_slice(&p.y.to_le_bytes());
        }
        push_record(&mut bytes, 1, &content);

        let geometries = read_shp(&bytes).unwrap();
        assert_eq!(geometries, vec![Geometry::MultiPoint(points.to_vec())]);
    }

    #[test]
    fn null_shape_record_produces_no_geometry() {
        let mut bytes = shp_header(1);
        let content = 0i32.to_le_bytes();
        push_record(&mut bytes, 1, &content);

        let geometries = read_shp(&bytes).unwrap();
        assert!(geometries.is_empty());
    }

    #[test]
    fn rejects_unsupported_shape_type() {
        let mut bytes = shp_header(11);
        let content = 11i32.to_le_bytes();
        push_record(&mut bytes, 1, &content);

        let result = read_shp(&bytes);
        assert!(matches!(result, Err(ShapefileError::UnsupportedShapeType(11))));
    }

    #[test]
    fn rejects_wrong_file_code() {
        let mut bytes = shp_header(1);
        bytes[0..4].copy_from_slice(&1234i32.to_be_bytes());

        let result = read_shp(&bytes);
        assert!(matches!(result, Err(ShapefileError::InvalidFileCode(1234))));
    }

    #[test]
    fn rejects_truncated_input() {
        let mut bytes = shp_header(1);
        let mut content = Vec::new();
        content.extend_from_slice(&1i32.to_le_bytes());
        content.extend_from_slice(&1.0f64.to_le_bytes());
        content.extend_from_slice(&2.0f64.to_le_bytes());
        push_record(&mut bytes, 1, &content);
        bytes.truncate(bytes.len() - 4);

        let result = read_shp(&bytes);
        assert!(matches!(result, Err(ShapefileError::UnexpectedEndOfInput)));
    }

    #[test]
    fn shp_geometry_iter_yields_each_record() {
        let mut bytes = shp_header(1);
        let mut content = Vec::new();
        content.extend_from_slice(&1i32.to_le_bytes());
        content.extend_from_slice(&12.5f64.to_le_bytes());
        content.extend_from_slice(&(-34.25f64).to_le_bytes());
        push_record(&mut bytes, 1, &content);

        content.clear();
        content.extend_from_slice(&1i32.to_le_bytes());
        content.extend_from_slice(&0.0f64.to_le_bytes());
        content.extend_from_slice(&0.0f64.to_le_bytes());
        push_record(&mut bytes, 2, &content);

        let geometries: Vec<_> = ShpGeometryIter::new(&bytes).map(|g| g.unwrap()).collect();
        assert_eq!(
            geometries,
            vec![Geometry::Point(Point::new(12.5, -34.25)), Geometry::Point(Point::new(0.0, 0.0)),]
        );
    }

    fn build_dbf() -> Vec<u8> {
        let field_descriptors: &[(&str, u8, u8, u8)] =
            &[("NAME", b'C', 10, 0), ("AGE", b'N', 4, 0)];
        let record_len: usize =
            1 + field_descriptors.iter().map(|f| usize::from(f.2)).sum::<usize>();
        let header_len: usize = 32 + field_descriptors.len() * 32 + 1;

        let mut bytes = Vec::new();
        bytes.push(0x03);
        bytes.extend_from_slice(&[0, 0, 0]);
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&(u16::try_from(header_len).unwrap()).to_le_bytes());
        bytes.extend_from_slice(&(u16::try_from(record_len).unwrap()).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 20]);
        assert_eq!(bytes.len(), 32);

        for &(name, field_type, length, decimals) in field_descriptors {
            let mut name_bytes = [0u8; 11];
            name_bytes[..name.len()].copy_from_slice(name.as_bytes());
            bytes.extend_from_slice(&name_bytes);
            bytes.push(field_type);
            bytes.extend_from_slice(&[0u8; 4]);
            bytes.push(length);
            bytes.push(decimals);
            bytes.extend_from_slice(&[0u8; 14]);
        }
        bytes.push(0x0D);
        assert_eq!(bytes.len(), header_len);

        bytes.push(0x20);
        bytes.extend_from_slice(b"Alice     ");
        bytes.extend_from_slice(b"  30");

        bytes.push(0x2A);
        bytes.extend_from_slice(b"Bob       ");
        bytes.extend_from_slice(b"   7");

        bytes
    }

    #[test]
    fn reads_dbf_records_with_character_and_numeric_fields() {
        let bytes = build_dbf();
        let records = read_dbf(&bytes).unwrap();

        assert_eq!(records.len(), 2);

        assert!(!records[0].is_deleted);
        assert_eq!(
            records[0].values,
            vec![
                ("NAME".to_string(), DbfValue::Character("Alice".to_string())),
                ("AGE".to_string(), DbfValue::Numeric(30.0)),
            ]
        );

        assert!(records[1].is_deleted);
        assert_eq!(
            records[1].values,
            vec![
                ("NAME".to_string(), DbfValue::Character("Bob".to_string())),
                ("AGE".to_string(), DbfValue::Numeric(7.0)),
            ]
        );
    }

    #[test]
    fn rejects_unsupported_dbf_field_type() {
        let mut bytes = Vec::new();
        bytes.push(0x03);
        bytes.extend_from_slice(&[0, 0, 0]);
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&(32u16 + 32 + 1).to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 20]);

        let mut name_bytes = [0u8; 11];
        name_bytes[..4].copy_from_slice(b"BAD ");
        bytes.extend_from_slice(&name_bytes);
        bytes.push(b'X');
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.push(1);
        bytes.push(0);
        bytes.extend_from_slice(&[0u8; 14]);
        bytes.push(0x0D);

        let result = read_dbf(&bytes);
        assert!(matches!(result, Err(ShapefileError::InvalidFieldType(b'X'))));
    }
}
