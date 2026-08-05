//! Well-Known Text (WKT) reader/writer, per the OGC Simple Features textual
//! representation of geometry.
//!
//! Unlike [`crate::geojson`], WKT has no off-the-shelf `serde`-style structure to
//! deserialize into — it's a small custom grammar (`KEYWORD (coordinate lists)`,
//! arbitrarily nested for polygons/multi-geometries/collections) — so this module
//! hand-rolls a recursive-descent parser over a character buffer rather than
//! pulling in a parsing-library dependency. The grammar is small enough that
//! `std` string/char methods are all it needs.
//!
//! Only 2D geometries are supported: `Point`/`Polygon`/etc. in [`crate::geometry`]
//! store plain `x`/`y` pairs, so there is nowhere to put a Z or M ordinate. The WKT
//! `EMPTY` keyword (e.g. `POINT EMPTY`) is also not supported, since none of this
//! crate's geometry variants have a well-defined "empty" representation to parse it
//! into; both limitations produce a [`WktError`] rather than a panic.

use core::fmt;

use crate::geometry::{Geometry, Point, Polygon};

/// An error encountered while parsing a WKT string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WktError {
    /// The input ended before a complete geometry could be parsed (e.g. a missing
    /// closing parenthesis).
    UnexpectedEndOfInput,
    /// A token did not match what the grammar expected at that position.
    UnexpectedToken(String),
    /// The geometry type keyword (e.g. `POINT`, `LINESTRING`) was not one of the
    /// seven supported types.
    UnknownGeometryType(String),
    /// The input used the WKT `EMPTY` keyword for the given geometry type, which
    /// this parser does not support (see the module documentation).
    EmptyGeometryUnsupported(String),
    /// The geometry parsed successfully but the input had leftover, non-whitespace
    /// text after it.
    TrailingInput(String),
    /// A nested `GEOMETRYCOLLECTION` exceeded the maximum allowed nesting depth (a
    /// guard against malicious, deeply-recursive input).
    MaxNestingDepth,
    /// A coordinate contained a non-finite value (NaN or infinity), which this
    /// crate's geometry types do not represent.
    NonFiniteCoordinate,
}

impl fmt::Display for WktError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WktError::UnexpectedEndOfInput => write!(f, "unexpected end of input"),
            WktError::UnexpectedToken(msg) => write!(f, "unexpected token: {msg}"),
            WktError::UnknownGeometryType(name) => write!(f, "unknown geometry type \"{name}\""),
            WktError::EmptyGeometryUnsupported(name) => write!(
                f,
                "\"{name} EMPTY\" is not supported; this parser does not represent empty geometries"
            ),
            WktError::TrailingInput(text) => write!(f, "unexpected trailing input: \"{text}\""),
            WktError::MaxNestingDepth => write!(f, "WKT nesting depth exceeded maximum"),
            WktError::NonFiniteCoordinate => {
                write!(f, "coordinate is not a finite number (NaN or infinity)")
            }
        }
    }
}

impl std::error::Error for WktError {}

/// The maximum nesting depth allowed for `GEOMETRYCOLLECTION` geometries. Beyond
/// this, parsing is rejected to bound stack usage on hostile input.
const MAX_NESTING_DEPTH: u32 = 32;

/// A cursor over the characters of a WKT string.
///
/// Characters (not bytes) are the unit of position here: WKT is small enough in
/// practice that owning a `Vec<char>` up front is cheap, and it sidesteps any need
/// to reason about UTF-8 character-boundary slicing while scanning.
struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self { chars: input.chars().collect(), pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    /// A short preview of the remaining input, for error messages.
    fn remaining_preview(&self) -> String {
        const MAX_LEN: usize = 40;
        let rest = &self.chars[self.pos..];
        if rest.len() > MAX_LEN {
            format!("{}…", rest[..MAX_LEN].iter().collect::<String>())
        } else {
            rest.iter().collect()
        }
    }

    /// Consumes whitespace, then a single expected literal character.
    fn expect_char(&mut self, expected: char) -> Result<(), WktError> {
        self.skip_whitespace();
        match self.peek() {
            Some(c) if c == expected => {
                self.pos += 1;
                Ok(())
            }
            Some(_) => Err(WktError::UnexpectedToken(format!(
                "expected '{expected}', found \"{}\"",
                self.remaining_preview()
            ))),
            None => Err(WktError::UnexpectedEndOfInput),
        }
    }

    /// Consumes whitespace, then an alphabetic word (a geometry keyword or `EMPTY`).
    fn read_word(&mut self) -> Result<String, WktError> {
        self.skip_whitespace();
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            self.pos += 1;
        }
        if self.pos == start {
            return match self.peek() {
                Some(_) => Err(WktError::UnexpectedToken(format!(
                    "expected a keyword, found \"{}\"",
                    self.remaining_preview()
                ))),
                None => Err(WktError::UnexpectedEndOfInput),
            };
        }
        Ok(self.chars[start..self.pos].iter().collect())
    }

    /// Consumes whitespace, then a signed decimal number (optionally with an
    /// exponent), returning its parsed `f64` value.
    fn read_number(&mut self) -> Result<f64, WktError> {
        self.skip_whitespace();
        let start = self.pos;

        if matches!(self.peek(), Some('-' | '+')) {
            self.pos += 1;
        }
        let mut saw_digit = false;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
            saw_digit = true;
        }
        if self.peek() == Some('.') {
            self.pos += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
                saw_digit = true;
            }
        }
        if !saw_digit {
            self.pos = start;
            return Err(WktError::UnexpectedToken(format!(
                "expected a number, found \"{}\"",
                self.remaining_preview()
            )));
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            let exponent_mark = self.pos;
            self.pos += 1;
            if matches!(self.peek(), Some('-' | '+')) {
                self.pos += 1;
            }
            let digits_start = self.pos;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == digits_start {
                // Not actually an exponent (e.g. a trailing 'e' with no digits) —
                // back off and let whatever follows be tokenized separately.
                self.pos = exponent_mark;
            }
        }

        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse::<f64>()
            .map_err(|_| WktError::UnexpectedToken(format!("invalid number \"{text}\"")))
    }
}

/// Consumes whitespace, then either an opening `(` (returning `Ok(())`, ready for
/// the caller to parse the body) or the `EMPTY` keyword (returning the
/// unsupported-EMPTY error tagged with `type_name`).
fn expect_open_paren(p: &mut Parser, type_name: &str) -> Result<(), WktError> {
    p.skip_whitespace();
    match p.peek() {
        Some('(') => {
            p.pos += 1;
            Ok(())
        }
        Some(c) if c.is_ascii_alphabetic() => {
            let word = p.read_word()?;
            if word.eq_ignore_ascii_case("EMPTY") {
                Err(WktError::EmptyGeometryUnsupported(type_name.to_string()))
            } else {
                Err(WktError::UnexpectedToken(format!("expected '(' or EMPTY, found \"{word}\"")))
            }
        }
        Some(_) => Err(WktError::UnexpectedToken(format!(
            "expected '(', found \"{}\"",
            p.remaining_preview()
        ))),
        None => Err(WktError::UnexpectedEndOfInput),
    }
}

/// Parses a comma-separated list of items up to (and consuming) a closing `)`.
/// The opening `(` must already have been consumed by the caller.
fn parse_list<T>(
    p: &mut Parser,
    mut parse_item: impl FnMut(&mut Parser) -> Result<T, WktError>,
) -> Result<Vec<T>, WktError> {
    p.skip_whitespace();
    if p.peek() == Some(')') {
        p.pos += 1;
        return Ok(Vec::new());
    }

    let mut items = vec![parse_item(p)?];
    loop {
        p.skip_whitespace();
        match p.peek() {
            Some(',') => {
                p.pos += 1;
                items.push(parse_item(p)?);
            }
            Some(')') => {
                p.pos += 1;
                return Ok(items);
            }
            Some(_) => {
                return Err(WktError::UnexpectedToken(format!(
                    "expected ',' or ')', found \"{}\"",
                    p.remaining_preview()
                )));
            }
            None => return Err(WktError::UnexpectedEndOfInput),
        }
    }
}

/// Parses a single `x y` coordinate pair (no parentheses).
fn parse_coord(p: &mut Parser) -> Result<Point, WktError> {
    let x = p.read_number()?;
    let y = p.read_number()?;
    if !x.is_finite() || !y.is_finite() {
        return Err(WktError::NonFiniteCoordinate);
    }
    Ok(Point::new(x, y))
}

/// Parses a parenthesized, comma-separated coordinate list: `(x y, x y, ...)`.
/// Used both for a `LINESTRING`'s points and for one ring of a `POLYGON` (rings
/// have no grammatical distinction from a plain coordinate list).
fn parse_paren_coord_list(p: &mut Parser) -> Result<Vec<Point>, WktError> {
    p.expect_char('(')?;
    parse_list(p, parse_coord)
}

/// Parses one `MULTIPOINT` element, which may be a bare coordinate pair
/// (`10 40`) or one parenthesized like a single-point ring (`(10 40)`) — both
/// forms are common in the wild, so both are accepted.
fn parse_multipoint_item(p: &mut Parser) -> Result<Point, WktError> {
    p.skip_whitespace();
    if p.peek() == Some('(') {
        p.pos += 1;
        let point = parse_coord(p)?;
        p.expect_char(')')?;
        Ok(point)
    } else {
        parse_coord(p)
    }
}

/// Parses one geometry, starting from its type keyword, recursing for
/// `GEOMETRYCOLLECTION` members.
fn parse_tagged_geometry(p: &mut Parser, depth: u32) -> Result<Geometry, WktError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(WktError::MaxNestingDepth);
    }
    let keyword = p.read_word()?;
    match keyword.to_ascii_uppercase().as_str() {
        "POINT" => {
            expect_open_paren(p, "POINT")?;
            let point = parse_coord(p)?;
            p.expect_char(')')?;
            Ok(Geometry::Point(point))
        }
        "LINESTRING" => {
            expect_open_paren(p, "LINESTRING")?;
            let points = parse_list(p, parse_coord)?;
            Ok(Geometry::LineString(points))
        }
        "POLYGON" => {
            expect_open_paren(p, "POLYGON")?;
            let rings = parse_list(p, parse_paren_coord_list)?;
            if rings.is_empty() {
                return Err(WktError::UnexpectedToken("polygon has no rings".to_string()));
            }
            Ok(Geometry::Polygon(Polygon { rings }))
        }
        "MULTIPOINT" => {
            expect_open_paren(p, "MULTIPOINT")?;
            let points = parse_list(p, parse_multipoint_item)?;
            Ok(Geometry::MultiPoint(points))
        }
        "MULTILINESTRING" => {
            expect_open_paren(p, "MULTILINESTRING")?;
            let lines = parse_list(p, parse_paren_coord_list)?;
            Ok(Geometry::MultiLineString(lines))
        }
        "MULTIPOLYGON" => {
            expect_open_paren(p, "MULTIPOLYGON")?;
            let polygons = parse_list(p, |p| {
                p.expect_char('(')?;
                let rings = parse_list(p, parse_paren_coord_list)?;
                if rings.is_empty() {
                    return Err(WktError::UnexpectedToken("polygon has no rings".to_string()));
                }
                Ok(Polygon { rings })
            })?;
            Ok(Geometry::MultiPolygon(polygons))
        }
        "GEOMETRYCOLLECTION" => {
            expect_open_paren(p, "GEOMETRYCOLLECTION")?;
            let geometries = parse_list(p, |p| parse_tagged_geometry(p, depth + 1))?;
            Ok(Geometry::GeometryCollection(geometries))
        }
        other => Err(WktError::UnknownGeometryType(other.to_string())),
    }
}

/// Parses a WKT geometry string (e.g. `"POINT (30 10)"`) into a [`Geometry`].
///
/// Supports the seven Simple Features geometry types: `POINT`, `LINESTRING`,
/// `POLYGON`, `MULTIPOINT`, `MULTILINESTRING`, `MULTIPOLYGON`, and
/// `GEOMETRYCOLLECTION`. Type keywords are case-insensitive, and whitespace
/// between tokens (including between a keyword and its opening parenthesis) is
/// flexible. `MULTIPOINT` accepts both the parenthesized-per-point form
/// (`MULTIPOINT ((10 40), (40 30))`) and the common unparenthesized form
/// (`MULTIPOINT (10 40, 40 30)`).
///
/// # Limitations
/// As documented at the module level: Z/M (3D or measured) coordinates are not
/// supported, only plain `x y` pairs; and the WKT `EMPTY` keyword (e.g.
/// `POINT EMPTY`) is not supported and produces
/// [`WktError::EmptyGeometryUnsupported`] rather than a panic.
///
/// # Errors
/// Returns [`WktError`] if `wkt` is not well-formed, uses an unrecognized
/// geometry keyword, uses `EMPTY`, or has trailing content after a complete
/// geometry.
pub fn parse_geometry(wkt: &str) -> Result<Geometry, WktError> {
    let mut parser = Parser::new(wkt);
    let geometry = parse_tagged_geometry(&mut parser, 0)?;
    parser.skip_whitespace();
    if !parser.eof() {
        return Err(WktError::TrailingInput(parser.remaining_preview()));
    }
    Ok(geometry)
}

/// Writes `x y` for one coordinate.
fn write_coord(point: Point, out: &mut String) {
    out.push_str(&point.x.to_string());
    out.push(' ');
    out.push_str(&point.y.to_string());
}

/// Writes a parenthesized, comma-separated coordinate list: `(x y, x y, ...)`.
fn write_coord_list(points: &[Point], out: &mut String) {
    out.push('(');
    for (i, &point) in points.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_coord(point, out);
    }
    out.push(')');
}

/// Writes a parenthesized, comma-separated list of coordinate lists:
/// `((x y, ...), (x y, ...))` — the shape shared by a polygon's rings and a
/// multi-linestring's component lines.
fn write_coord_lists(lists: &[Vec<Point>], out: &mut String) {
    out.push('(');
    for (i, list) in lists.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_coord_list(list, out);
    }
    out.push(')');
}

/// Appends the WKT representation of `geometry` to `out`, without a trailing
/// newline or separator (used to implement both [`write_geometry`] and
/// `GEOMETRYCOLLECTION` member serialization).
fn write_tagged_geometry(geometry: &Geometry, out: &mut String) {
    match geometry {
        Geometry::Point(point) => {
            out.push_str("POINT (");
            write_coord(*point, out);
            out.push(')');
        }
        Geometry::LineString(points) => {
            out.push_str("LINESTRING ");
            write_coord_list(points, out);
        }
        Geometry::Polygon(polygon) => {
            out.push_str("POLYGON ");
            write_coord_lists(&polygon.rings, out);
        }
        Geometry::MultiPoint(points) => {
            out.push_str("MULTIPOINT (");
            for (i, &point) in points.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push('(');
                write_coord(point, out);
                out.push(')');
            }
            out.push(')');
        }
        Geometry::MultiLineString(lines) => {
            out.push_str("MULTILINESTRING ");
            write_coord_lists(lines, out);
        }
        Geometry::MultiPolygon(polygons) => {
            out.push_str("MULTIPOLYGON (");
            for (i, polygon) in polygons.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_coord_lists(&polygon.rings, out);
            }
            out.push(')');
        }
        Geometry::GeometryCollection(geometries) => {
            out.push_str("GEOMETRYCOLLECTION (");
            for (i, geometry) in geometries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_tagged_geometry(geometry, out);
            }
            out.push(')');
        }
    }
}

/// Serializes a [`Geometry`] to its WKT string representation.
///
/// Geometry type keywords are written uppercase with exactly one space before
/// the opening parenthesis, regardless of how (or whether) whitespace appeared
/// in any input this value was originally parsed from. `MultiPoint` is always
/// written with parentheses around each point (e.g. `MULTIPOINT ((10 40), (40
/// 30))`).
#[must_use]
pub fn write_geometry(geometry: &Geometry) -> String {
    let mut out = String::new();
    write_tagged_geometry(geometry, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The eight example geometries are the canonical WKT examples from the OGC
    // Simple Features specification, as reproduced on Wikipedia's "Well-known
    // text representation of geometry" page — an independent reference, not
    // derived from this implementation.

    #[test]
    fn parses_point_example() {
        let geometry = parse_geometry("POINT (30 10)").unwrap();
        assert_eq!(geometry, Geometry::Point(Point::new(30.0, 10.0)));
    }

    #[test]
    fn parses_linestring_example() {
        let geometry = parse_geometry("LINESTRING (30 10, 10 30, 40 40)").unwrap();
        match geometry {
            Geometry::LineString(points) => {
                assert_eq!(
                    points,
                    vec![Point::new(30.0, 10.0), Point::new(10.0, 30.0), Point::new(40.0, 40.0)]
                );
            }
            other => panic!("expected LineString, got {other:?}"),
        }
    }

    #[test]
    fn parses_polygon_example() {
        let geometry = parse_geometry("POLYGON ((30 10, 40 40, 20 40, 10 20, 30 10))").unwrap();
        match geometry {
            Geometry::Polygon(polygon) => {
                assert_eq!(polygon.rings.len(), 1);
                assert_eq!(
                    polygon.exterior(),
                    &[
                        Point::new(30.0, 10.0),
                        Point::new(40.0, 40.0),
                        Point::new(20.0, 40.0),
                        Point::new(10.0, 20.0),
                        Point::new(30.0, 10.0),
                    ]
                );
            }
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    #[test]
    fn parses_polygon_with_hole_example() {
        let wkt = "POLYGON ((35 10, 45 45, 15 40, 10 20, 35 10), (20 30, 35 35, 30 20, 20 30))";
        let geometry = parse_geometry(wkt).unwrap();
        match geometry {
            Geometry::Polygon(polygon) => {
                assert_eq!(polygon.rings.len(), 2);
                assert_eq!(polygon.exterior().len(), 5);
                assert_eq!(
                    polygon.interiors(),
                    &[vec![
                        Point::new(20.0, 30.0),
                        Point::new(35.0, 35.0),
                        Point::new(30.0, 20.0),
                        Point::new(20.0, 30.0),
                    ]]
                );
            }
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    #[test]
    fn parses_multipoint_example() {
        let geometry = parse_geometry("MULTIPOINT ((10 40), (40 30), (20 20), (30 10))").unwrap();
        assert_eq!(
            geometry,
            Geometry::MultiPoint(vec![
                Point::new(10.0, 40.0),
                Point::new(40.0, 30.0),
                Point::new(20.0, 20.0),
                Point::new(30.0, 10.0),
            ])
        );
    }

    #[test]
    fn parses_multilinestring_example() {
        let wkt = "MULTILINESTRING ((10 10, 20 20, 10 40), (40 40, 30 30, 40 20, 30 10))";
        let geometry = parse_geometry(wkt).unwrap();
        match geometry {
            Geometry::MultiLineString(lines) => {
                assert_eq!(lines.len(), 2);
                assert_eq!(
                    lines[0],
                    vec![Point::new(10.0, 10.0), Point::new(20.0, 20.0), Point::new(10.0, 40.0)]
                );
                assert_eq!(
                    lines[1],
                    vec![
                        Point::new(40.0, 40.0),
                        Point::new(30.0, 30.0),
                        Point::new(40.0, 20.0),
                        Point::new(30.0, 10.0),
                    ]
                );
            }
            other => panic!("expected MultiLineString, got {other:?}"),
        }
    }

    #[test]
    fn parses_multipolygon_example() {
        let wkt =
            "MULTIPOLYGON (((30 20, 45 40, 10 40, 30 20)), ((15 5, 40 10, 10 20, 5 10, 15 5)))";
        let geometry = parse_geometry(wkt).unwrap();
        match geometry {
            Geometry::MultiPolygon(polygons) => {
                assert_eq!(polygons.len(), 2);
                assert_eq!(
                    polygons[0].exterior(),
                    &[
                        Point::new(30.0, 20.0),
                        Point::new(45.0, 40.0),
                        Point::new(10.0, 40.0),
                        Point::new(30.0, 20.0)
                    ]
                );
                assert_eq!(
                    polygons[1].exterior(),
                    &[
                        Point::new(15.0, 5.0),
                        Point::new(40.0, 10.0),
                        Point::new(10.0, 20.0),
                        Point::new(5.0, 10.0),
                        Point::new(15.0, 5.0),
                    ]
                );
            }
            other => panic!("expected MultiPolygon, got {other:?}"),
        }
    }

    #[test]
    fn parses_geometrycollection_example() {
        let wkt = "GEOMETRYCOLLECTION (POINT (40 10), LINESTRING (10 10, 20 20, 10 40))";
        let geometry = parse_geometry(wkt).unwrap();
        match geometry {
            Geometry::GeometryCollection(geometries) => {
                assert_eq!(geometries.len(), 2);
                assert_eq!(geometries[0], Geometry::Point(Point::new(40.0, 10.0)));
                assert_eq!(
                    geometries[1],
                    Geometry::LineString(vec![
                        Point::new(10.0, 10.0),
                        Point::new(20.0, 20.0),
                        Point::new(10.0, 40.0),
                    ])
                );
            }
            other => panic!("expected GeometryCollection, got {other:?}"),
        }
    }

    #[test]
    fn point_round_trips() {
        let geometry = Geometry::Point(Point::new(-122.4194, 37.7749));
        let wkt = write_geometry(&geometry);
        assert_eq!(wkt, "POINT (-122.4194 37.7749)");
        assert_eq!(parse_geometry(&wkt).unwrap(), geometry);
    }

    #[test]
    fn linestring_round_trips() {
        let geometry = Geometry::LineString(vec![Point::new(30.0, 10.0), Point::new(10.0, 30.0)]);
        assert_eq!(parse_geometry(&write_geometry(&geometry)).unwrap(), geometry);
    }

    #[test]
    fn polygon_with_hole_round_trips() {
        let geometry = Geometry::Polygon(Polygon {
            rings: vec![
                vec![
                    Point::new(35.0, 10.0),
                    Point::new(45.0, 45.0),
                    Point::new(15.0, 40.0),
                    Point::new(10.0, 20.0),
                    Point::new(35.0, 10.0),
                ],
                vec![
                    Point::new(20.0, 30.0),
                    Point::new(35.0, 35.0),
                    Point::new(30.0, 20.0),
                    Point::new(20.0, 30.0),
                ],
            ],
        });
        let wkt = write_geometry(&geometry);
        assert_eq!(parse_geometry(&wkt).unwrap(), geometry);
    }

    #[test]
    fn multipoint_round_trips() {
        let geometry = Geometry::MultiPoint(vec![Point::new(10.0, 40.0), Point::new(40.0, 30.0)]);
        let wkt = write_geometry(&geometry);
        assert_eq!(wkt, "MULTIPOINT ((10 40), (40 30))");
        assert_eq!(parse_geometry(&wkt).unwrap(), geometry);
    }

    #[test]
    fn multilinestring_round_trips() {
        let geometry = Geometry::MultiLineString(vec![
            vec![Point::new(10.0, 10.0), Point::new(20.0, 20.0)],
            vec![Point::new(40.0, 40.0), Point::new(30.0, 30.0)],
        ]);
        assert_eq!(parse_geometry(&write_geometry(&geometry)).unwrap(), geometry);
    }

    #[test]
    fn multipolygon_round_trips() {
        let geometry = Geometry::MultiPolygon(vec![
            Polygon::from_exterior(vec![
                Point::new(30.0, 20.0),
                Point::new(45.0, 40.0),
                Point::new(30.0, 20.0),
            ]),
            Polygon::from_exterior(vec![
                Point::new(15.0, 5.0),
                Point::new(40.0, 10.0),
                Point::new(15.0, 5.0),
            ]),
        ]);
        assert_eq!(parse_geometry(&write_geometry(&geometry)).unwrap(), geometry);
    }

    #[test]
    fn geometrycollection_round_trips() {
        let geometry = Geometry::GeometryCollection(vec![
            Geometry::Point(Point::new(40.0, 10.0)),
            Geometry::LineString(vec![Point::new(10.0, 10.0), Point::new(20.0, 20.0)]),
        ]);
        assert_eq!(parse_geometry(&write_geometry(&geometry)).unwrap(), geometry);
    }

    #[test]
    fn keyword_is_case_insensitive() {
        let expected = Geometry::Point(Point::new(30.0, 10.0));
        assert_eq!(parse_geometry("point (30 10)").unwrap(), expected);
        assert_eq!(parse_geometry("Point (30 10)").unwrap(), expected);
        assert_eq!(parse_geometry("PoInT (30 10)").unwrap(), expected);
    }

    #[test]
    fn multipoint_accepts_both_forms() {
        let with_parens =
            parse_geometry("MULTIPOINT ((10 40), (40 30), (20 20), (30 10))").unwrap();
        let without_parens = parse_geometry("MULTIPOINT (10 40, 40 30, 20 20, 30 10)").unwrap();
        assert_eq!(with_parens, without_parens);
        assert_eq!(
            with_parens,
            Geometry::MultiPoint(vec![
                Point::new(10.0, 40.0),
                Point::new(40.0, 30.0),
                Point::new(20.0, 20.0),
                Point::new(30.0, 10.0),
            ])
        );
    }

    #[test]
    fn flexible_whitespace_is_accepted() {
        // No space between keyword and paren, and irregular internal spacing.
        let geometry = parse_geometry("LINESTRING(30 10,10   30 ,40 40)").unwrap();
        assert_eq!(
            geometry,
            Geometry::LineString(vec![
                Point::new(30.0, 10.0),
                Point::new(10.0, 30.0),
                Point::new(40.0, 40.0)
            ])
        );
    }

    #[test]
    fn rejects_missing_closing_paren() {
        let result = parse_geometry("POINT (30 10");
        assert_eq!(result, Err(WktError::UnexpectedEndOfInput));
    }

    #[test]
    fn rejects_unknown_geometry_type() {
        let result = parse_geometry("SPHERE (0 0)");
        assert_eq!(result, Err(WktError::UnknownGeometryType("SPHERE".to_string())));
    }

    #[test]
    fn rejects_empty_keyword() {
        let result = parse_geometry("POINT EMPTY");
        assert_eq!(result, Err(WktError::EmptyGeometryUnsupported("POINT".to_string())));
    }

    #[test]
    fn rejects_trailing_input() {
        let result = parse_geometry("POINT (30 10) garbage");
        assert!(matches!(result, Err(WktError::TrailingInput(_))));
    }

    #[test]
    fn rejects_non_finite_coordinate() {
        // Textual "NaN"/"inf" are rejected as unparseable numbers first, but an
        // overflowing literal like `1e999` parses to f64::INFINITY, which the
        // finiteness guard must still reject.
        assert!(matches!(parse_geometry("POINT (1e999 10)"), Err(WktError::NonFiniteCoordinate)));
        assert!(matches!(parse_geometry("POINT (10 1e999)"), Err(WktError::NonFiniteCoordinate)));
    }

    #[test]
    fn rejects_excessive_nesting_depth() {
        // Build GEOMETRYCOLLECTION( GEOMETRYCOLLECTION( ... POINT(0 0) ... ) ) deeper
        // than the allowed limit.
        let mut wkt = String::new();
        for _ in 0..50 {
            wkt.push_str("GEOMETRYCOLLECTION (");
        }
        wkt.push_str("POINT (0 0)");
        for _ in 0..50 {
            wkt.push(')');
        }
        assert!(matches!(parse_geometry(&wkt), Err(WktError::MaxNestingDepth)));
    }
}
