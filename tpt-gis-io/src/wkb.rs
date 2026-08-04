//! Well-Known Binary (WKB) reader/writer.
//!
//! This implements the standard 2D ISO/OGC Simple Features binary encoding — no
//! SRID prefix and no Z/M coordinates (that's the PostGIS "EWKB" extension, a
//! different, non-standardized wire format, and out of scope here). Every geometry
//! — and, recursively, every element nested inside a `Multi*` or
//! `GeometryCollection` — is a self-contained unit: a 1-byte byte-order marker (`0`
//! = big-endian/XDR, `1` = little-endian/NDR), a 4-byte geometry type code (encoded
//! in that byte order), and then type-specific data.
//!
//! `LineString` and `Polygon` store their points as flat `count`-then-coordinates
//! blocks with no per-point header. `MultiPoint`/`MultiLineString`/`MultiPolygon`/
//! `GeometryCollection` are different: their elements are stored as *complete*
//! nested WKB geometries, each with its own byte-order marker and type code. This
//! is easy to get wrong, since a flat `MultiPoint` body looks superficially similar
//! to a `LineString` body but isn't.

use core::fmt;

use crate::geometry::{Geometry, Point, Polygon};

/// The 4-byte geometry type codes used in a WKB header.
mod geom_type {
    pub(super) const POINT: u32 = 1;
    pub(super) const LINE_STRING: u32 = 2;
    pub(super) const POLYGON: u32 = 3;
    pub(super) const MULTI_POINT: u32 = 4;
    pub(super) const MULTI_LINE_STRING: u32 = 5;
    pub(super) const MULTI_POLYGON: u32 = 6;
    pub(super) const GEOMETRY_COLLECTION: u32 = 7;
}

/// A human-readable name for a WKB geometry type code, for error messages.
/// Returns `"unknown"` for a code outside the standard 2D set.
fn type_name(type_code: u32) -> &'static str {
    match type_code {
        geom_type::POINT => "Point",
        geom_type::LINE_STRING => "LineString",
        geom_type::POLYGON => "Polygon",
        geom_type::MULTI_POINT => "MultiPoint",
        geom_type::MULTI_LINE_STRING => "MultiLineString",
        geom_type::MULTI_POLYGON => "MultiPolygon",
        geom_type::GEOMETRY_COLLECTION => "GeometryCollection",
        _ => "unknown",
    }
}

/// The WKB type code that a parsed [`Geometry`] value would have been read from.
fn type_code_of(geometry: &Geometry) -> u32 {
    match geometry {
        Geometry::Point(_) => geom_type::POINT,
        Geometry::LineString(_) => geom_type::LINE_STRING,
        Geometry::Polygon(_) => geom_type::POLYGON,
        Geometry::MultiPoint(_) => geom_type::MULTI_POINT,
        Geometry::MultiLineString(_) => geom_type::MULTI_LINE_STRING,
        Geometry::MultiPolygon(_) => geom_type::MULTI_POLYGON,
        Geometry::GeometryCollection(_) => geom_type::GEOMETRY_COLLECTION,
    }
}

/// The byte order a WKB geometry (or one of its nested sub-geometries) is encoded
/// in, per its own 1-byte marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteOrder {
    /// Marker `0`: big-endian, a.k.a. XDR.
    Big,
    /// Marker `1`: little-endian, a.k.a. NDR. Used by essentially every
    /// real-world WKB producer (PostGIS, GDAL, ...).
    Little,
}

impl ByteOrder {
    fn from_marker(marker: u8) -> Result<Self, WkbError> {
        match marker {
            0 => Ok(ByteOrder::Big),
            1 => Ok(ByteOrder::Little),
            other => Err(WkbError::InvalidByteOrder(other)),
        }
    }

    fn marker(self) -> u8 {
        match self {
            ByteOrder::Big => 0,
            ByteOrder::Little => 1,
        }
    }
}

/// An error encountered while parsing WKB.
#[derive(Debug)]
pub enum WkbError {
    /// The input ended before a byte-order marker, type code, coordinate count, or
    /// coordinate value that the format declared was present could be fully read.
    UnexpectedEndOfInput,
    /// A byte-order marker was neither `0` (big-endian) nor `1` (little-endian).
    InvalidByteOrder(u8),
    /// A 4-byte geometry type code was not one of the 7 standard 2D types.
    UnknownGeometryType(u32),
    /// A `Polygon` declared zero rings. A valid polygon always has an exterior
    /// ring — see [`Polygon::exterior`], which panics on an empty ring list.
    EmptyPolygon,
    /// An element nested inside a `MultiPoint`/`MultiLineString`/`MultiPolygon`
    /// was not the geometry type that collection requires (e.g. a `LineString`
    /// found inside a `MultiPoint`).
    UnexpectedElementType {
        /// The geometry type code required by the enclosing collection.
        expected: u32,
        /// The geometry type code actually found.
        found: u32,
    },
    /// The input had unconsumed bytes remaining after one complete geometry was
    /// read.
    TrailingBytes(usize),
}

impl fmt::Display for WkbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WkbError::UnexpectedEndOfInput => write!(f, "unexpected end of WKB input"),
            WkbError::InvalidByteOrder(marker) => {
                write!(f, "invalid WKB byte-order marker {marker} (expected 0 or 1)")
            }
            WkbError::UnknownGeometryType(code) => {
                write!(f, "unknown WKB geometry type code {code}")
            }
            WkbError::EmptyPolygon => write!(f, "WKB polygon has no rings"),
            WkbError::UnexpectedElementType { expected, found } => write!(
                f,
                "expected a {} element, found a {} (type code {found})",
                type_name(*expected),
                type_name(*found),
            ),
            WkbError::TrailingBytes(count) => {
                write!(f, "{count} unconsumed byte(s) after the WKB geometry")
            }
        }
    }
}

impl std::error::Error for WkbError {}

/// Converts a length to the `u32` WKB uses for every count (points, rings,
/// elements).
///
/// # Panics
/// Panics if `len` exceeds `u32::MAX`. No real-world geometry has anywhere near
/// four billion points, rings, or elements, so this is not a practical concern.
fn u32_len(len: usize) -> u32 {
    u32::try_from(len).expect("geometry has more than u32::MAX points/rings/elements")
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// A cursor over a WKB byte slice.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn remaining_len(&self) -> usize {
        self.bytes.len() - self.pos
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], WkbError> {
        let end = self.pos.checked_add(len).ok_or(WkbError::UnexpectedEndOfInput)?;
        let slice = self.bytes.get(self.pos..end).ok_or(WkbError::UnexpectedEndOfInput)?;
        self.pos = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, WkbError> {
        Ok(self.take(1)?[0])
    }

    fn read_u32(&mut self, order: ByteOrder) -> Result<u32, WkbError> {
        let bytes: [u8; 4] = self.take(4)?.try_into().expect("take(4) yields exactly 4 bytes");
        Ok(match order {
            ByteOrder::Little => u32::from_le_bytes(bytes),
            ByteOrder::Big => u32::from_be_bytes(bytes),
        })
    }

    fn read_f64(&mut self, order: ByteOrder) -> Result<f64, WkbError> {
        let bytes: [u8; 8] = self.take(8)?.try_into().expect("take(8) yields exactly 8 bytes");
        Ok(match order {
            ByteOrder::Little => f64::from_le_bytes(bytes),
            ByteOrder::Big => f64::from_be_bytes(bytes),
        })
    }

    fn read_point(&mut self, order: ByteOrder) -> Result<Point, WkbError> {
        let x = self.read_f64(order)?;
        let y = self.read_f64(order)?;
        Ok(Point::new(x, y))
    }

    /// Reads a 4-byte count followed by that many flat (x, y) coordinate pairs —
    /// the shape shared by a `LineString` body and each ring of a `Polygon` body.
    fn read_points(&mut self, order: ByteOrder) -> Result<Vec<Point>, WkbError> {
        let count = self.read_u32(order)?;
        let mut points = Vec::new();
        for _ in 0..count {
            points.push(self.read_point(order)?);
        }
        Ok(points)
    }

    fn read_polygon(&mut self, order: ByteOrder) -> Result<Polygon, WkbError> {
        let ring_count = self.read_u32(order)?;
        let mut rings = Vec::new();
        for _ in 0..ring_count {
            rings.push(self.read_points(order)?);
        }
        if rings.is_empty() {
            return Err(WkbError::EmptyPolygon);
        }
        Ok(Polygon { rings })
    }

    /// Reads a 4-byte count followed by that many complete, independent WKB
    /// geometries (each with its own byte-order marker and type code) — the shape
    /// used by `Multi*` and `GeometryCollection` bodies.
    fn read_elements(&mut self, order: ByteOrder) -> Result<Vec<Geometry>, WkbError> {
        let count = self.read_u32(order)?;
        let mut geometries = Vec::new();
        for _ in 0..count {
            geometries.push(self.read_geometry()?);
        }
        Ok(geometries)
    }

    /// Reads a `Multi*` element list, checking that every nested geometry has the
    /// expected type and unwrapping it via `extract`.
    fn read_typed_elements<T>(
        &mut self,
        order: ByteOrder,
        expected: u32,
        extract: impl Fn(Geometry) -> Result<T, Geometry>,
    ) -> Result<Vec<T>, WkbError> {
        self.read_elements(order)?
            .into_iter()
            .map(|geometry| {
                extract(geometry).map_err(|other| WkbError::UnexpectedElementType {
                    expected,
                    found: type_code_of(&other),
                })
            })
            .collect()
    }

    /// Reads one full geometry: byte-order marker, type code, and type-specific
    /// body.
    fn read_geometry(&mut self) -> Result<Geometry, WkbError> {
        let order = ByteOrder::from_marker(self.read_u8()?)?;
        let type_code = self.read_u32(order)?;
        match type_code {
            geom_type::POINT => Ok(Geometry::Point(self.read_point(order)?)),
            geom_type::LINE_STRING => Ok(Geometry::LineString(self.read_points(order)?)),
            geom_type::POLYGON => Ok(Geometry::Polygon(self.read_polygon(order)?)),
            geom_type::MULTI_POINT => {
                let points = self.read_typed_elements(order, geom_type::POINT, |g| match g {
                    Geometry::Point(p) => Ok(p),
                    other => Err(other),
                })?;
                Ok(Geometry::MultiPoint(points))
            }
            geom_type::MULTI_LINE_STRING => {
                let lines =
                    self.read_typed_elements(order, geom_type::LINE_STRING, |g| match g {
                        Geometry::LineString(points) => Ok(points),
                        other => Err(other),
                    })?;
                Ok(Geometry::MultiLineString(lines))
            }
            geom_type::MULTI_POLYGON => {
                let polygons =
                    self.read_typed_elements(order, geom_type::POLYGON, |g| match g {
                        Geometry::Polygon(polygon) => Ok(polygon),
                        other => Err(other),
                    })?;
                Ok(Geometry::MultiPolygon(polygons))
            }
            geom_type::GEOMETRY_COLLECTION => {
                Ok(Geometry::GeometryCollection(self.read_elements(order)?))
            }
            other => Err(WkbError::UnknownGeometryType(other)),
        }
    }
}

/// Parses a single WKB geometry from its binary encoding.
///
/// Both big-endian (XDR) and little-endian (NDR) input are accepted: the 1-byte
/// order marker at the start of the geometry (and of each sub-geometry nested
/// inside a `Multi*`/`GeometryCollection`) selects the encoding used for the bytes
/// that immediately follow it. The input must contain exactly one geometry and
/// nothing else — trailing bytes are an error, not silently ignored.
pub fn parse_geometry(bytes: &[u8]) -> Result<Geometry, WkbError> {
    let mut reader = Reader::new(bytes);
    let geometry = reader.read_geometry()?;
    if reader.remaining_len() > 0 {
        return Err(WkbError::TrailingBytes(reader.remaining_len()));
    }
    Ok(geometry)
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

fn push_u32(out: &mut Vec<u8>, order: ByteOrder, value: u32) {
    match order {
        ByteOrder::Little => out.extend_from_slice(&value.to_le_bytes()),
        ByteOrder::Big => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_f64(out: &mut Vec<u8>, order: ByteOrder, value: f64) {
    match order {
        ByteOrder::Little => out.extend_from_slice(&value.to_le_bytes()),
        ByteOrder::Big => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_point(out: &mut Vec<u8>, order: ByteOrder, point: Point) {
    push_f64(out, order, point.x);
    push_f64(out, order, point.y);
}

/// Writes a 4-byte count followed by that many flat (x, y) coordinate pairs.
fn push_points(out: &mut Vec<u8>, order: ByteOrder, points: &[Point]) {
    push_u32(out, order, u32_len(points.len()));
    for &point in points {
        push_point(out, order, point);
    }
}

fn push_header(out: &mut Vec<u8>, order: ByteOrder, type_code: u32) {
    out.push(order.marker());
    push_u32(out, order, type_code);
}

fn write_point(out: &mut Vec<u8>, order: ByteOrder, point: Point) {
    push_header(out, order, geom_type::POINT);
    push_point(out, order, point);
}

fn write_line_string(out: &mut Vec<u8>, order: ByteOrder, points: &[Point]) {
    push_header(out, order, geom_type::LINE_STRING);
    push_points(out, order, points);
}

fn write_polygon(out: &mut Vec<u8>, order: ByteOrder, polygon: &Polygon) {
    push_header(out, order, geom_type::POLYGON);
    push_u32(out, order, u32_len(polygon.rings.len()));
    for ring in &polygon.rings {
        push_points(out, order, ring);
    }
}

fn write_geometry_ordered(out: &mut Vec<u8>, order: ByteOrder, geometry: &Geometry) {
    match geometry {
        Geometry::Point(point) => write_point(out, order, *point),
        Geometry::LineString(points) => write_line_string(out, order, points),
        Geometry::Polygon(polygon) => write_polygon(out, order, polygon),
        Geometry::MultiPoint(points) => {
            push_header(out, order, geom_type::MULTI_POINT);
            push_u32(out, order, u32_len(points.len()));
            for &point in points {
                write_point(out, order, point);
            }
        }
        Geometry::MultiLineString(lines) => {
            push_header(out, order, geom_type::MULTI_LINE_STRING);
            push_u32(out, order, u32_len(lines.len()));
            for line in lines {
                write_line_string(out, order, line);
            }
        }
        Geometry::MultiPolygon(polygons) => {
            push_header(out, order, geom_type::MULTI_POLYGON);
            push_u32(out, order, u32_len(polygons.len()));
            for polygon in polygons {
                write_polygon(out, order, polygon);
            }
        }
        Geometry::GeometryCollection(geometries) => {
            push_header(out, order, geom_type::GEOMETRY_COLLECTION);
            push_u32(out, order, u32_len(geometries.len()));
            for geometry in geometries {
                write_geometry_ordered(out, order, geometry);
            }
        }
    }
}

/// Serializes a [`Geometry`] to little-endian (NDR) WKB.
///
/// Little-endian is the near-universal convention for WKB produced by real-world
/// tools (PostGIS, GDAL, ...). [`parse_geometry`] reads either byte order, so this
/// crate's own output round-trips regardless of which order a peer prefers.
#[must_use]
pub fn write_geometry(geometry: &Geometry) -> Vec<u8> {
    let mut out = Vec::new();
    write_geometry_ordered(&mut out, ByteOrder::Little, geometry);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_round_trips(geometry: Geometry) {
        let bytes = write_geometry(&geometry);
        assert_eq!(parse_geometry(&bytes).unwrap(), geometry);
    }

    // -- Manual byte-level reference tests (independent of the writer/reader's
    // -- own internals) --

    #[test]
    fn point_writes_expected_le_bytes() {
        let mut expected = vec![1u8]; // little-endian
        expected.extend_from_slice(&1u32.to_le_bytes()); // type = Point
        expected.extend_from_slice(&30.0f64.to_le_bytes());
        expected.extend_from_slice(&10.0f64.to_le_bytes());
        assert_eq!(write_geometry(&Geometry::Point(Point::new(30.0, 10.0))), expected);
    }

    #[test]
    fn point_parses_from_manual_le_bytes() {
        let mut bytes = vec![1u8];
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&30.0f64.to_le_bytes());
        bytes.extend_from_slice(&10.0f64.to_le_bytes());
        assert_eq!(parse_geometry(&bytes).unwrap(), Geometry::Point(Point::new(30.0, 10.0)));
    }

    // -- Round trips for all 7 geometry variants --

    #[test]
    fn point_round_trips() {
        assert_round_trips(Geometry::Point(Point::new(1.5, -2.5)));
    }

    #[test]
    fn line_string_round_trips() {
        assert_round_trips(Geometry::LineString(vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(2.0, 0.0),
        ]));
    }

    #[test]
    fn polygon_with_hole_round_trips() {
        let exterior = vec![
            Point::new(35.0, 10.0),
            Point::new(45.0, 45.0),
            Point::new(15.0, 40.0),
            Point::new(10.0, 20.0),
            Point::new(35.0, 10.0),
        ];
        let hole = vec![
            Point::new(20.0, 30.0),
            Point::new(35.0, 35.0),
            Point::new(30.0, 20.0),
            Point::new(20.0, 30.0),
        ];
        assert_round_trips(Geometry::Polygon(Polygon { rings: vec![exterior, hole] }));
    }

    #[test]
    fn multi_point_round_trips() {
        assert_round_trips(Geometry::MultiPoint(vec![
            Point::new(10.0, 40.0),
            Point::new(40.0, 30.0),
            Point::new(20.0, 20.0),
        ]));
    }

    #[test]
    fn multi_line_string_round_trips() {
        assert_round_trips(Geometry::MultiLineString(vec![
            vec![Point::new(10.0, 10.0), Point::new(20.0, 20.0)],
            vec![Point::new(40.0, 40.0), Point::new(30.0, 30.0), Point::new(40.0, 20.0)],
        ]));
    }

    #[test]
    fn multi_polygon_round_trips() {
        let a = Polygon::from_exterior(vec![
            Point::new(30.0, 20.0),
            Point::new(45.0, 40.0),
            Point::new(10.0, 40.0),
            Point::new(30.0, 20.0),
        ]);
        let b = Polygon::from_exterior(vec![
            Point::new(15.0, 5.0),
            Point::new(40.0, 10.0),
            Point::new(10.0, 20.0),
            Point::new(5.0, 10.0),
            Point::new(15.0, 5.0),
        ]);
        assert_round_trips(Geometry::MultiPolygon(vec![a, b]));
    }

    #[test]
    fn geometry_collection_round_trips_mixed_types() {
        let polygon = Polygon::from_exterior(vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 0.0),
        ]);
        assert_round_trips(Geometry::GeometryCollection(vec![
            Geometry::Point(Point::new(4.0, 6.0)),
            Geometry::LineString(vec![Point::new(4.0, 6.0), Point::new(7.0, 10.0)]),
            Geometry::Polygon(polygon),
            Geometry::MultiPoint(vec![Point::new(1.0, 1.0), Point::new(2.0, 2.0)]),
        ]));
    }

    // -- Big-endian support --

    #[test]
    fn big_endian_point_parses_same_as_little_endian() {
        let mut be_bytes = vec![0u8]; // big-endian marker
        be_bytes.extend_from_slice(&1u32.to_be_bytes()); // type = Point
        be_bytes.extend_from_slice(&30.0f64.to_be_bytes());
        be_bytes.extend_from_slice(&10.0f64.to_be_bytes());

        assert_eq!(parse_geometry(&be_bytes).unwrap(), Geometry::Point(Point::new(30.0, 10.0)));
    }

    #[test]
    fn big_endian_round_trips_nested_geometry() {
        // Exercise the nested-header logic (nested nested LineStrings each carry
        // their own byte-order marker) with the writer forced to big-endian.
        let geometry = Geometry::MultiLineString(vec![
            vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)],
            vec![Point::new(5.0, 6.0)],
        ]);
        let mut bytes = Vec::new();
        write_geometry_ordered(&mut bytes, ByteOrder::Big, &geometry);
        assert_eq!(parse_geometry(&bytes).unwrap(), geometry);
    }

    // -- Malformed input --

    #[test]
    fn rejects_invalid_byte_order_marker() {
        let bytes = vec![2u8, 1, 0, 0, 0];
        assert!(matches!(parse_geometry(&bytes), Err(WkbError::InvalidByteOrder(2))));
    }

    #[test]
    fn rejects_unknown_geometry_type() {
        let mut bytes = vec![1u8];
        bytes.extend_from_slice(&99u32.to_le_bytes());
        assert!(matches!(parse_geometry(&bytes), Err(WkbError::UnknownGeometryType(99))));
    }

    #[test]
    fn rejects_truncated_point() {
        let mut bytes = vec![1u8];
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&30.0f64.to_le_bytes());
        // y coordinate is missing entirely.
        assert!(matches!(parse_geometry(&bytes), Err(WkbError::UnexpectedEndOfInput)));
    }

    #[test]
    fn rejects_line_string_shorter_than_declared_count() {
        let mut bytes = vec![1u8];
        bytes.extend_from_slice(&2u32.to_le_bytes()); // type = LineString
        bytes.extend_from_slice(&3u32.to_le_bytes()); // claims 3 points
        bytes.extend_from_slice(&1.0f64.to_le_bytes());
        bytes.extend_from_slice(&2.0f64.to_le_bytes()); // only 1 point's worth of data follows
        assert!(matches!(parse_geometry(&bytes), Err(WkbError::UnexpectedEndOfInput)));
    }

    #[test]
    fn rejects_empty_polygon() {
        let mut bytes = vec![1u8];
        bytes.extend_from_slice(&3u32.to_le_bytes()); // type = Polygon
        bytes.extend_from_slice(&0u32.to_le_bytes()); // 0 rings
        assert!(matches!(parse_geometry(&bytes), Err(WkbError::EmptyPolygon)));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut bytes = write_geometry(&Geometry::Point(Point::new(1.0, 2.0)));
        bytes.push(0xFF);
        assert!(matches!(parse_geometry(&bytes), Err(WkbError::TrailingBytes(1))));
    }

    #[test]
    fn rejects_wrong_element_type_inside_multi_point() {
        // A MultiPoint with one element, but that element is a LineString rather
        // than a Point — the nested-header mistake this format makes easy to get
        // wrong in the *writer*; this checks the *reader* rejects it too.
        let mut bytes = vec![1u8];
        bytes.extend_from_slice(&4u32.to_le_bytes()); // type = MultiPoint
        bytes.extend_from_slice(&1u32.to_le_bytes()); // 1 element
        bytes.push(1u8); // element byte order
        bytes.extend_from_slice(&2u32.to_le_bytes()); // element type = LineString
        bytes.extend_from_slice(&0u32.to_le_bytes()); // 0 points in the LineString
        assert!(matches!(
            parse_geometry(&bytes),
            Err(WkbError::UnexpectedElementType { expected: 1, found: 2 })
        ));
    }
}
