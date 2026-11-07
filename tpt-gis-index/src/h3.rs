//! H3 hexagonal discrete global grid integration, via the [`h3o`] crate — see the
//! module-level doc comment in `lib.rs` for why `h3o` (rather than a from-scratch
//! reimplementation) was chosen.

use core::fmt;
use std::collections::HashSet;

use h3o::{LatLng, Resolution};
use tpt_gis_geom::Point;

pub use h3o::CellIndex;

/// An error encountered while working with H3 cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H3Error {
    /// A latitude/longitude was not a finite number (e.g. NaN or infinite).
    InvalidCoordinate,
    /// A resolution was requested outside H3's valid range of 0-15.
    InvalidResolution(u8),
    /// A string was not a valid H3 cell index.
    InvalidCellIndexString,
}

impl fmt::Display for H3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            H3Error::InvalidCoordinate => write!(f, "latitude/longitude must be finite"),
            H3Error::InvalidResolution(r) => write!(f, "invalid H3 resolution {r} (must be 0-15)"),
            H3Error::InvalidCellIndexString => write!(f, "invalid H3 cell index string"),
        }
    }
}

impl std::error::Error for H3Error {}

/// Finds the H3 cell containing a geographic coordinate at the given resolution
/// (0 = coarsest, ~4,250km average hexagon edge length; 15 = finest, ~0.5m).
pub fn cell_at(lat_deg: f64, lon_deg: f64, resolution: u8) -> Result<CellIndex, H3Error> {
    let resolution =
        Resolution::try_from(resolution).map_err(|_| H3Error::InvalidResolution(resolution))?;
    let latlng = LatLng::new(lat_deg, lon_deg).map_err(|_| H3Error::InvalidCoordinate)?;
    Ok(latlng.to_cell(resolution))
}

/// The cell's boundary as a closed ring of `(longitude, latitude)` points,
/// suitable for use as a `tpt_gis_geom::Polygon` exterior ring (via
/// [`Polygon::from_exterior`](tpt_gis_geom::Polygon::from_exterior) — note that
/// ring is expected closed, so push the first point again at the end if your
/// consumer requires that).
#[must_use]
pub fn cell_boundary(cell: CellIndex) -> Vec<Point> {
    cell.boundary().iter().map(|ll| Point::new(ll.lng(), ll.lat())).collect()
}

/// The cell's center, as `(longitude, latitude)`.
#[must_use]
pub fn cell_center(cell: CellIndex) -> Point {
    let ll = LatLng::from(cell);
    Point::new(ll.lng(), ll.lat())
}

/// All cells within grid distance `k` of `cell` (the "k-ring"), including `cell`
/// itself at `k = 0`.
#[must_use]
pub fn k_ring(cell: CellIndex, k: u32) -> Vec<CellIndex> {
    cell.grid_disk::<Vec<CellIndex>>(k)
}

/// The H3 cells at `resolution` whose centres lie inside a polygon, given as
/// `(longitude, latitude)` degrees: an exterior ring and any holes.
///
/// This is the `ContainsCentroid` rule: a cell is included when its centre is inside the
/// polygon (outside every hole). Cells are found by flood fill from the cells around the
/// exterior's vertices, growing through neighbouring cells. So a polygon's parts are found
/// only if each part contains an included cell near its vertices; a polygon smaller than one
/// cell may have no included cell at all, and the result is then empty.
///
/// # Errors
/// Returns [`H3Error::InvalidResolution`] for a resolution outside 0-15, and
/// [`H3Error::InvalidCoordinate`] if any coordinate is not finite or a latitude is outside
/// `[-90, 90]`.
pub fn polygon_cells(
    exterior: &[Point],
    holes: &[&[Point]],
    resolution: u8,
) -> Result<Vec<CellIndex>, H3Error> {
    Resolution::try_from(resolution).map_err(|_| H3Error::InvalidResolution(resolution))?;
    let valid = |p: &Point| p.x.is_finite() && p.y.is_finite() && (-90.0..=90.0).contains(&p.y);
    if !exterior.iter().all(valid) || !holes.iter().all(|h| h.iter().all(valid)) {
        return Err(H3Error::InvalidCoordinate);
    }
    if exterior.len() < 3 {
        return Ok(Vec::new());
    }

    let polygon = tpt_gis_geom::Polygon::new(exterior, holes);
    let inside = |cell: CellIndex| polygon.contains_point(cell_center(cell));

    // Seeds: the included cells around each exterior vertex.
    let mut seen: HashSet<CellIndex> = HashSet::new();
    let mut stack: Vec<CellIndex> = Vec::new();
    for vertex in exterior {
        let Ok(cell) = cell_at(vertex.y, vertex.x, resolution) else { continue };
        for candidate in k_ring(cell, 1) {
            if seen.insert(candidate) && inside(candidate) {
                stack.push(candidate);
            }
        }
    }

    // Flood fill through neighbours whose centres are inside.
    let mut out = Vec::new();
    while let Some(cell) = stack.pop() {
        out.push(cell);
        for neighbour in k_ring(cell, 1) {
            if seen.insert(neighbour) && inside(neighbour) {
                stack.push(neighbour);
            }
        }
    }
    out.sort_unstable();
    Ok(out)
}

/// Parses an H3 cell index from its canonical hexadecimal string form (e.g.
/// `"8a1fb46622dffff"`).
pub fn parse_cell(s: &str) -> Result<CellIndex, H3Error> {
    s.parse().map_err(|_| H3Error::InvalidCellIndexString)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_gis_geom::Polygon;

    #[test]
    fn cell_at_uses_requested_resolution() {
        let cell = cell_at(37.7749, -122.4194, 9).unwrap();
        assert_eq!(u8::from(cell.resolution()), 9);
    }

    #[test]
    fn invalid_resolution_is_rejected() {
        assert_eq!(cell_at(0.0, 0.0, 16), Err(H3Error::InvalidResolution(16)));
    }

    #[test]
    fn non_finite_coordinate_is_rejected() {
        assert_eq!(cell_at(f64::NAN, 0.0, 5), Err(H3Error::InvalidCoordinate));
    }

    #[test]
    fn cell_boundary_encloses_its_own_center() {
        // Cross-check tying tpt-gis-index (H3) to tpt-gis-geom (point-in-polygon):
        // the boundary this module produces should form a ring that actually
        // contains the point that generated the cell in the first place.
        let lat = 37.7749;
        let lon = -122.4194;
        let cell = cell_at(lat, lon, 9).unwrap();

        let mut boundary = cell_boundary(cell);
        boundary.push(boundary[0]); // close the ring, per tpt-gis-geom's convention
        let polygon = Polygon::from_exterior(&boundary);

        assert!(polygon.contains_point(Point::new(lon, lat)));
    }

    #[test]
    fn k_ring_zero_is_just_the_cell_itself() {
        let cell = cell_at(0.0, 0.0, 5).unwrap();
        assert_eq!(k_ring(cell, 0), vec![cell]);
    }

    #[test]
    fn k_ring_one_has_seven_cells_for_a_hexagon() {
        // A cell far from a pentagon distortion: 1-ring is the cell plus its 6
        // neighbors.
        let cell = cell_at(37.7749, -122.4194, 9).unwrap();
        assert!(!cell.is_pentagon());
        assert_eq!(k_ring(cell, 1).len(), 7);
    }

    #[test]
    fn cell_index_string_round_trips() {
        let cell = cell_at(51.5074, -0.1278, 7).unwrap();
        let text = cell.to_string();
        assert_eq!(parse_cell(&text).unwrap(), cell);
    }

    #[test]
    fn invalid_cell_index_string_is_rejected() {
        assert_eq!(parse_cell("not-a-cell-index"), Err(H3Error::InvalidCellIndexString));
    }
}

#[cfg(test)]
mod covering_tests {
    use super::*;
    use std::collections::HashSet;

    /// Reference: every cell that a dense lattice over the polygon's box (plus one cell of
    /// margin) lands in, kept when its centre is inside the polygon.
    fn brute_force(exterior: &[Point], holes: &[&[Point]], resolution: u8) -> Vec<CellIndex> {
        let polygon = tpt_gis_geom::Polygon::new(exterior, holes);
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in exterior {
            x0 = x0.min(p.x);
            x1 = x1.max(p.x);
            y0 = y0.min(p.y);
            y1 = y1.max(p.y);
        }
        let margin = 0.25;
        let step = 0.004;
        let mut cells = HashSet::new();
        let mut y = y0 - margin;
        while y <= y1 + margin {
            let mut x = x0 - margin;
            while x <= x1 + margin {
                if let Ok(cell) = cell_at(y, x, resolution) {
                    cells.insert(cell);
                }
                x += step;
            }
            y += step;
        }
        let mut out: Vec<CellIndex> =
            cells.into_iter().filter(|&c| polygon.contains_point(cell_center(c))).collect();
        out.sort_unstable();
        out
    }

    fn pts(coords: &[(f64, f64)]) -> Vec<Point> {
        coords.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    #[test]
    fn convex_polygon_matches_brute_force() {
        // A triangle in western Europe.
        let ext = pts(&[(2.0, 48.0), (4.5, 48.5), (3.0, 50.0)]);
        let got = polygon_cells(&ext, &[], 5).unwrap();
        assert!(!got.is_empty());
        assert_eq!(got, brute_force(&ext, &[], 5));
    }

    #[test]
    fn concave_polygon_matches_brute_force() {
        // An L shape.
        let ext = pts(&[
            (10.0, 40.0),
            (11.5, 40.0),
            (11.5, 40.7),
            (10.7, 40.7),
            (10.7, 41.5),
            (10.0, 41.5),
        ]);
        let got = polygon_cells(&ext, &[], 5).unwrap();
        assert_eq!(got, brute_force(&ext, &[], 5));
    }

    #[test]
    fn hole_is_excluded() {
        let ext = pts(&[(20.0, 10.0), (21.5, 10.0), (21.5, 11.5), (20.0, 11.5)]);
        let hole = pts(&[(20.5, 10.5), (21.0, 10.5), (21.0, 11.0), (20.5, 11.0)]);
        let holes: [&[Point]; 1] = [&hole];
        let got = polygon_cells(&ext, &holes, 5).unwrap();
        assert_eq!(got, brute_force(&ext, &holes, 5));
        let without_hole = polygon_cells(&ext, &[], 5).unwrap();
        assert!(got.len() < without_hole.len());
    }

    #[test]
    fn polygon_smaller_than_a_cell_can_be_empty() {
        let tiny = pts(&[(0.0, 0.0), (0.0001, 0.0), (0.0001, 0.0001)]);
        assert!(polygon_cells(&tiny, &[], 2).unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_input() {
        let tri = pts(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]);
        assert_eq!(polygon_cells(&tri, &[], 16), Err(H3Error::InvalidResolution(16)));
        let nan = pts(&[(f64::NAN, 0.0), (1.0, 0.0), (0.0, 1.0)]);
        assert_eq!(polygon_cells(&nan, &[], 5), Err(H3Error::InvalidCoordinate));
    }
}
