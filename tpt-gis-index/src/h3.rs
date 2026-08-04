//! H3 hexagonal discrete global grid integration, via the [`h3o`] crate — see the
//! module-level doc comment in `lib.rs` for why `h3o` (rather than a from-scratch
//! reimplementation) was chosen.

use core::fmt;

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
