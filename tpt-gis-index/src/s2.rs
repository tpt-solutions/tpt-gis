//! S2 spherical grid integration.
//!
//! S2 subdivides the unit sphere into six cube faces, each recursively
//! quartered into a quadtree of cells. A cell is identified by a 64-bit
//! [`S2CellId`] that packs the face (3 bits) and, level by level, the
//! two bits of the cell's `(i, j)` position on its face, with a trailing marker
//! bit that records the level.
//!
//! **Compatibility note:** this implementation orders the `(i, j)` bits with a
//! simple Z-order (Morton) interleave, *not* the Hilbert curve Google's S2 uses.
//! The cell math (face selection, the quadratic `s`/`t` ↔ `u`/`v` transform, and
//! parent/child navigation) is self-consistent and round-trips internally, but the
//! tokens produced here are **not** byte-compatible with Google's S2 cell ids.
//! Treat the token format as internal to this crate unless/until a true
//! Hilbert-curve mapping is added.
//!
//! The integration here is intentionally small: turn a latitude/longitude into a
//! cell id (optionally at a chosen level), go back the other way, walk up to the
//! parent and down to the four children, and render the canonical hex token.
//!
//! # Example
//!
//! ```
//! use tpt_gis_index::s2::{S2CellId, MAX_LEVEL};
//!
//! let id = S2CellId::from_lat_lon_degrees(40.743, -74.0008);
//! // The id's level is the finest by default (MAX_LEVEL = 30).
//! assert_eq!(id.level(), MAX_LEVEL);
//! // A parent at a coarser level still tokens round-trip.
//! let coarse = id.parent(10);
//! assert_eq!(S2CellId::from_token(&coarse.to_token()), Some(coarse));
//! // And it really does contain the point.
//! let (lat, lon) = coarse.to_lat_lon_degrees();
//! let back = S2CellId::from_lat_lon_degrees(lat, lon).parent(10);
//! assert_eq!(back, coarse);
//! ```

use core::fmt;

/// The number of quadtree levels in an S2 cell id. Level 30 cells are roughly
/// 1 cm across at the equator.
pub const MAX_LEVEL: u8 = 30;
/// The number of cube faces of the S2 projection.
pub const NUM_FACES: u8 = 6;

/// A 64-bit S2 cell identifier.
///
/// The bit layout, from the most significant end, is three face bits, then for
/// each of the 30 levels two bits of the cell's `(i, j)` position interleaved as
/// `(i_bit, j_bit)`, with a single `1` marker bit whose position records the
/// cell's level. Cells at coarser levels leave the lower bits zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct S2CellId(pub u64);

impl S2CellId {
    /// The raw 64-bit value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Whether the raw value is a well-formed cell id: face `< 6`, and a single
    /// level marker bit at an even position no higher than bit 60. Id `0` and any
    /// value with a face of 6 or 7 are invalid.
    #[must_use]
    pub fn is_valid(self) -> bool {
        if self.0 == 0 || self.face() >= NUM_FACES {
            return false;
        }
        let marker = self.0.trailing_zeros();
        marker.is_multiple_of(2) && marker <= 2 * MAX_LEVEL as u32
    }

    /// The cell's level, 0 (largest) .. [`MAX_LEVEL`] (smallest).
    ///
    /// For an invalid id (see [`S2CellId::is_valid`]) the result is unspecified but
    /// never panics; it saturates at `0`.
    #[must_use]
    pub fn level(self) -> u8 {
        // The trailing `1` marker sits at bit 2 * (MAX_LEVEL - level).
        let trailing = (self.0.trailing_zeros() / 2) as u8;
        MAX_LEVEL.saturating_sub(trailing)
    }

    /// The cube face this cell lies on, 0..6.
    #[must_use]
    pub fn face(self) -> u8 {
        (self.0 >> 61) as u8 & 0b111
    }

    /// The cell's `(i, j)` position on its face at its own level, each in
    /// `0..2^level`.
    #[must_use]
    pub fn ij(self) -> (u64, u64) {
        let level = self.level() as u32;
        // The interleaved position sits just above the level marker bit. Within it,
        // level-0 bits are nearest the marker (LSB-first), and bit `2k` holds i's
        // bit `k` while bit `2k + 1` holds j's bit `k` — matching `from_face_ij`.
        let shift = 2 * (MAX_LEVEL as u32 - level) + 1;
        let bits = self.0 >> shift;
        let mut i = 0u64;
        let mut j = 0u64;
        for k in 0..level {
            let pair = bits >> (2 * k);
            i |= (pair & 1) << k;
            j |= ((pair >> 1) & 1) << k;
        }
        (i, j)
    }

    /// Builds an id from a face and `(i, j)` position at `level`.
    ///
    /// # Panics
    /// Panics if `level > MAX_LEVEL` or `face >= NUM_FACES`.
    #[must_use]
    pub fn from_face_ij(face: u8, i: u64, j: u64, level: u8) -> Self {
        assert!(level <= MAX_LEVEL, "level exceeds MAX_LEVEL");
        assert!(face < NUM_FACES, "face out of range");
        // Interleave the `level` bits of `i` and `j`, LSB-first: bit `k` of the
        // position sits at cell-id bit `2k` (i) / `2k + 1` (j).
        let mut pos = 0u64;
        for k in 0..level as u32 {
            pos |= ((i >> k) & 1) << (2 * k) | ((j >> k) & 1) << (2 * k + 1);
        }
        let shift = 2 * (MAX_LEVEL as u32 - level as u32);
        S2CellId(((face as u64) << 61) | (pos << (shift + 1)) | (1u64 << shift))
    }

    /// The id's level-`level` ancestor. `level` must not be greater than the
    /// cell's own level.
    ///
    /// # Panics
    /// Panics if `level > self.level()`.
    #[must_use]
    pub fn parent(self, level: u8) -> Self {
        assert!(level <= self.level(), "parent level exceeds cell level");
        if level == self.level() {
            return self;
        }
        let shift = (self.level() - level) as u32;
        let (i, j) = self.ij();
        let i = i >> shift;
        let j = j >> shift;
        Self::from_face_ij(self.face(), i, j, level)
    }

    /// The four immediate children of this cell, in `(i, j)` order.
    #[must_use]
    pub fn children(self) -> [S2CellId; 4] {
        let level = self.level();
        assert!(level < MAX_LEVEL, "cannot descend past MAX_LEVEL");
        let child_level = level + 1;
        let (i, j) = self.ij();
        let i = i << 1;
        let j = j << 1;
        [
            Self::from_face_ij(self.face(), i, j, child_level),
            Self::from_face_ij(self.face(), i + 1, j, child_level),
            Self::from_face_ij(self.face(), i, j + 1, child_level),
            Self::from_face_ij(self.face(), i + 1, j + 1, child_level),
        ]
    }

    /// The cell containing the given latitude/longitude (degrees) at the finest
    /// level.
    ///
    /// # Panics
    /// Panics if the coordinates are not finite or are out of range. Use
    /// [`S2CellId::try_from_lat_lon_degrees`] for untrusted input.
    #[must_use]
    pub fn from_lat_lon_degrees(lat: f64, lon: f64) -> Self {
        Self::try_from_lat_lon_degrees(lat, lon).expect("latitude/longitude out of range")
    }

    /// Like [`S2CellId::from_lat_lon_degrees`], but returns `None` instead of
    /// panicking when `lat` is outside `[-90, 90]`, `lon` is outside `[-180, 180]`,
    /// or either value is not finite.
    #[must_use]
    pub fn try_from_lat_lon_degrees(lat: f64, lon: f64) -> Option<Self> {
        Self::try_from_lat_lon(lat.to_radians(), lon.to_radians(), MAX_LEVEL)
    }

    /// The cell containing the given latitude/longitude (radians) at `level`.
    ///
    /// # Panics
    /// Panics if `level > MAX_LEVEL` or the coordinates are invalid. Use
    /// [`S2CellId::try_from_lat_lon`] for untrusted input.
    #[must_use]
    pub fn from_lat_lon(lat: f64, lon: f64, level: u8) -> Self {
        Self::try_from_lat_lon(lat, lon, level).expect("invalid latitude/longitude or level")
    }

    /// Like [`S2CellId::from_lat_lon`], but returns `None` instead of panicking.
    ///
    /// Rejects non-finite values, a `level` above [`MAX_LEVEL`], and latitudes
    /// outside `[-π/2, π/2]` or longitudes outside `[-π, π]` (radians).
    #[must_use]
    pub fn try_from_lat_lon(lat: f64, lon: f64, level: u8) -> Option<Self> {
        use core::f64::consts::{FRAC_PI_2, PI};
        // Written so NaN fails every comparison and is rejected.
        let lat_ok = lat.is_finite() && (-FRAC_PI_2..=FRAC_PI_2).contains(&lat);
        let lon_ok = lon.is_finite() && (-PI..=PI).contains(&lon);
        if !lat_ok || !lon_ok || level > MAX_LEVEL {
            return None;
        }
        let x = lat.cos() * lon.cos();
        let y = lat.cos() * lon.sin();
        let z = lat.sin();
        let (face, u, v) = face_uv_from_xyz(x, y, z);
        // `u`/`v` are cube-space coordinates; convert them back to the linear `s`/`t`
        // space that `st_to_ij` maps to cell indices. Skipping this inverse quadratic
        // transform would map sphere points to the wrong cells.
        let s = uv_to_st(u);
        let t = uv_to_st(v);
        let shift = u32::from(MAX_LEVEL - level);
        let i = st_to_ij(s) >> shift;
        let j = st_to_ij(t) >> shift;
        Some(Self::from_face_ij(face, i, j, level))
    }

    /// The latitude/longitude (degrees) of the cell's centre.
    #[must_use]
    pub fn to_lat_lon_degrees(self) -> (f64, f64) {
        let (lat, lon) = self.to_lat_lon();
        (lat.to_degrees(), lon.to_degrees())
    }

    /// The latitude/longitude (radians) of the cell's centre.
    #[must_use]
    pub fn to_lat_lon(self) -> (f64, f64) {
        let level = self.level() as u32;
        let (i, j) = self.ij();
        let s = ij_to_st(i, level);
        let t = ij_to_st(j, level);
        let u = st_to_uv(s);
        let v = st_to_uv(t);
        let (x, y, z) = face_uv_to_xyz(self.face(), u, v);
        let r = (x * x + y * y + z * z).sqrt();
        let lat = (z / r).asin();
        let lon = y.atan2(x);
        (lat, lon)
    }

    /// The canonical lowercase hex token for this cell id.
    #[must_use]
    pub fn to_token(self) -> std::string::String {
        let mut s = std::string::String::with_capacity(16);
        // 16 hex digits, no leading zeros trimmed (S2 keeps them).
        for shift in (0..16).rev() {
            let nibble = ((self.0 >> (shift * 4)) & 0xf) as u8;
            s.push(core::char::from_digit(nibble as u32, 16).unwrap());
        }
        s
    }

    /// Parses a token produced by [`S2CellId::to_token`]. Returns `None` if the
    /// string is not 16 hex digits or does not decode to a [valid](S2CellId::is_valid)
    /// cell id.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        if token.len() != 16 {
            return None;
        }
        let mut value = 0u64;
        for ch in token.chars() {
            let digit = ch.to_digit(16)? as u64;
            value = (value << 4) | digit;
        }
        let id = S2CellId(value);
        id.is_valid().then_some(id)
    }
}

impl fmt::Display for S2CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_token())
    }
}

/// Maps a point on the unit sphere to a cube face and the (linear) `(u, v)`
/// coordinates on that face, each in `[-1, 1]`.
fn face_uv_from_xyz(x: f64, y: f64, z: f64) -> (u8, f64, f64) {
    let ax = x.abs();
    let ay = y.abs();
    let az = z.abs();
    if ax >= ay && ax >= az {
        if x > 0.0 {
            (0, y / x, z / x)
        } else {
            (3, y / x, z / x)
        }
    } else if ay >= az {
        if y > 0.0 {
            (1, -x / y, z / y)
        } else {
            (4, -x / y, z / y)
        }
    } else if z > 0.0 {
        (2, -x / z, -y / z)
    } else {
        (5, -x / z, -y / z)
    }
}

/// The inverse of [`face_uv_from_xyz`]: a face and `(u, v)` in `[-1, 1]` to an
/// (unnormalised) point on the sphere.
fn face_uv_to_xyz(face: u8, u: f64, v: f64) -> (f64, f64, f64) {
    match face {
        0 => (1.0, u, v),
        1 => (-u, 1.0, v),
        2 => (-u, -v, 1.0),
        // Faces 3/4/5: these must be the exact inverse of `face_uv_from_xyz` for the
        // round trip (xyz → face_uv → xyz) to reproduce the original point. The
        // previous assignments transposed/negated u and v, which silently rotated
        // every point on those three faces.
        3 => (-1.0, -u, -v),
        4 => (u, -1.0, -v),
        // Face 5 (z < 0): exact inverse of `(5, -x / z, -y / z)` with z = -1.
        _ => (u, v, -1.0),
    }
}

/// The S2 "quadratic" transform from a linear `s`/`t` value in `[-1, 1]` to a
/// cube-space `u`/`v`, used to make cells more uniform in area on the sphere.
fn st_to_uv(s: f64) -> f64 {
    if s >= 0.5 {
        (4.0 * s * s - 1.0) / 3.0
    } else {
        (1.0 - 4.0 * (1.0 - s) * (1.0 - s)) / 3.0
    }
}

/// The inverse of [`st_to_uv`]: maps a cube-space `u`/`v` in `[-1, 1]` back to the
/// linear `s`/`t` in `[-1, 1]`. This is the step [`S2CellId::from_lat_lon`] must
/// apply to the `(u, v)` returned by [`face_uv_from_xyz`] before converting to cell
/// indices.
fn uv_to_st(u: f64) -> f64 {
    if u >= 0.0 {
        (1.0 + 3.0 * u).sqrt() * 0.5
    } else {
        1.0 - (1.0 - 3.0 * u).sqrt() * 0.5
    }
}

/// The same-level cell `(di, dj)` steps away from `cell` in face-local `(i, j)` space.
///
/// Steps that leave the face are handled by projecting the offset centre back onto the
/// sphere and reading off the face it lands on, so the result is correct across cube
/// edges and corners as well.
fn neighbour(cell: S2CellId, di: i64, dj: i64) -> S2CellId {
    let level = cell.level() as u32;
    let n = (1u64 << level) as f64;
    let (i, j) = cell.ij();
    let s = (i as f64 + 0.5 + di as f64) / n;
    let t = (j as f64 + 0.5 + dj as f64) / n;
    let (x, y, z) = face_uv_to_xyz(cell.face(), st_to_uv(s), st_to_uv(t));
    let (face, u, v) = face_uv_from_xyz(x, y, z);
    let shift = u32::from(MAX_LEVEL) - level;
    let i = st_to_ij(uv_to_st(u)) >> shift;
    let j = st_to_ij(uv_to_st(v)) >> shift;
    S2CellId::from_face_ij(face, i, j, level as u8)
}

/// Maps a linear `s`/`t` in `[0, 1]` to a cell index in `0..2^MAX_LEVEL`. This is
/// the exact inverse of [`ij_to_st`] (which returns a centre in `[0, 1]`).
fn st_to_ij(s: f64) -> u64 {
    let max = (1u64 << MAX_LEVEL) - 1;
    let v = (s * (1u64 << MAX_LEVEL) as f64).floor();
    v.clamp(0.0, max as f64) as u64
}

/// Maps a cell index `pos` at `level` to the centre `s`/`t` in `(0, 1)`.
fn ij_to_st(pos: u64, level: u32) -> f64 {
    (pos as f64 + 0.5) / (1u64 << level) as f64
}

/// Covers a polygon with S2 cells at `level`: the cells whose centres lie inside it.
///
/// `exterior` and `holes` are rings of `(longitude, latitude)` points in degrees, as for
/// `tpt_gis_geom::Polygon`; containment is tested in that planar lon/lat frame, so polygons
/// that cross the antimeridian are not handled. Rings need not be closed.
///
/// The cells are found by seeding from the cells around each exterior vertex and then
/// flood filling through 8-neighbours whose centres are inside. Cells that the polygon only
/// touches along an edge can be missed, as with `h3::polygon_cells`.
///
/// Returns `None` if a coordinate is not a finite, in-range latitude/longitude or if `level`
/// exceeds [`MAX_LEVEL`]. Returns an empty vector for an exterior with fewer than three points.
/// The result is sorted.
#[must_use]
pub fn polygon_cells(
    exterior: &[tpt_gis_geom::Point],
    holes: &[&[tpt_gis_geom::Point]],
    level: u8,
) -> Option<Vec<S2CellId>> {
    if level > MAX_LEVEL {
        return None;
    }
    let valid = |p: &tpt_gis_geom::Point| {
        p.x.is_finite()
            && p.y.is_finite()
            && (-90.0..=90.0).contains(&p.y)
            && (-180.0..=180.0).contains(&p.x)
    };
    if !exterior.iter().all(valid) || !holes.iter().all(|h| h.iter().all(valid)) {
        return None;
    }
    if exterior.len() < 3 {
        return Some(Vec::new());
    }

    let polygon = tpt_gis_geom::Polygon::new(exterior, holes);
    let inside = |cell: S2CellId| {
        let (lat, lon) = cell.to_lat_lon_degrees();
        polygon.contains_point(tpt_gis_geom::Point::new(lon, lat))
    };

    let mut seen: std::collections::HashSet<S2CellId> = std::collections::HashSet::new();
    let mut stack: Vec<S2CellId> = Vec::new();
    for vertex in exterior {
        let Some(cell) =
            S2CellId::try_from_lat_lon(vertex.y.to_radians(), vertex.x.to_radians(), level)
        else {
            continue;
        };
        for di in -1..=1 {
            for dj in -1..=1 {
                let candidate = neighbour(cell, di, dj);
                if seen.insert(candidate) && inside(candidate) {
                    stack.push(candidate);
                }
            }
        }
    }

    let mut out = Vec::new();
    while let Some(cell) = stack.pop() {
        out.push(cell);
        for di in -1..=1 {
            for dj in -1..=1 {
                if di == 0 && dj == 0 {
                    continue;
                }
                let candidate = neighbour(cell, di, dj);
                if seen.insert(candidate) && inside(candidate) {
                    stack.push(candidate);
                }
            }
        }
    }
    out.sort_unstable();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xyz_face_uv_round_trips_on_every_face() {
        // One direction vector per cube face, including the south-polar face 5.
        let dirs = [
            (1.0, 0.2, -0.3),
            (-0.2, 1.0, 0.3),
            (-0.3, 0.2, 1.0),
            (-1.0, 0.2, -0.3),
            (0.3, -1.0, 0.2),
            (0.3, -0.2, -1.0),
        ];
        for (expected_face, (x, y, z)) in dirs.into_iter().enumerate() {
            let (face, u, v) = face_uv_from_xyz(x, y, z);
            assert_eq!(face as usize, expected_face);
            let (rx, ry, rz) = face_uv_to_xyz(face, u, v);
            // Same direction: components are proportional to the input.
            let scale = rx / x;
            assert!((ry - y * scale).abs() < 1e-12, "face {face}");
            assert!((rz - z * scale).abs() < 1e-12, "face {face}");
            assert!(scale > 0.0, "face {face} flipped direction");
        }
    }

    #[test]
    fn southern_polar_point_stays_in_southern_hemisphere() {
        let id = S2CellId::from_lat_lon_degrees(-89.0, 10.0);
        let (lat, _lon) = id.to_lat_lon_degrees();
        assert!(lat < -80.0, "got lat {lat}");
    }

    #[test]
    fn origin_maps_to_a_cell_whose_centre_is_the_origin() {
        let id = S2CellId::from_lat_lon_degrees(0.0, 0.0);
        let (lat, lon) = id.to_lat_lon_degrees();
        // A level-30 cell is ~1 cm across, so its centre can sit up to half a cell
        // (~1e-7 deg) from the input point.
        assert!((lat).abs() < 1e-6, "lat {lat}");
        assert!((lon).abs() < 1e-6, "lon {lon}");
    }

    #[test]
    fn tokens_round_trip() {
        let id = S2CellId::from_lat_lon_degrees(40.743, -74.0008);
        let token = id.to_token();
        assert_eq!(token.len(), 16);
        assert_eq!(S2CellId::from_token(&token), Some(id));
        assert_eq!(S2CellId::from_token("zzzzzzzzzzzzzzzz"), None);
        assert_eq!(S2CellId::from_token("abc"), None);
    }

    #[test]
    fn parent_and_children_are_consistent() {
        let id = S2CellId::from_lat_lon_degrees(51.5, 0.0);
        let coarse = id.parent(8);
        assert_eq!(coarse.level(), 8);
        // Re-deriving from the coarse centre lands back on the same ancestor.
        let (lat, lon) = coarse.to_lat_lon_degrees();
        assert_eq!(S2CellId::from_lat_lon_degrees(lat, lon).parent(8), coarse);

        let kids = coarse.children();
        assert_eq!(kids.len(), 4);
        for child in kids {
            assert_eq!(child.level(), 9);
            assert_eq!(child.parent(8), coarse);
        }
    }

    #[test]
    fn level_and_face_are_well_defined() {
        let id = S2CellId::from_lat_lon_degrees(-33.8, 151.2); // Sydney
        assert_eq!(id.level(), MAX_LEVEL);
        assert!(id.face() < NUM_FACES);
        // The default cell is its own parent at MAX_LEVEL.
        assert_eq!(id.parent(MAX_LEVEL), id);
    }

    #[test]
    fn ij_round_trips_through_face_ij() {
        let id = S2CellId::from_lat_lon_degrees(12.34, -56.78).parent(12);
        let (i, j) = id.ij();
        let rebuilt = S2CellId::from_face_ij(id.face(), i, j, id.level());
        assert_eq!(rebuilt, id);
    }

    #[test]
    fn invalid_ids_are_detected_without_panicking() {
        // Id 0 has no marker bit: it must be rejected and must not panic in level().
        assert!(!S2CellId(0).is_valid());
        assert_eq!(S2CellId(0).level(), 0);
        // Face 6 and 7 are out of range even with a valid marker.
        let face6 = S2CellId((6u64 << 61) | (1u64 << 60));
        assert!(!face6.is_valid());
        // Marker at an odd bit position does not encode a level.
        assert!(!S2CellId((1u64 << 61) | (1u64 << 59)).is_valid());
        // A genuine id is valid.
        assert!(S2CellId::from_lat_lon_degrees(10.0, 20.0).is_valid());
    }

    #[test]
    fn from_token_rejects_invalid_ids() {
        assert_eq!(S2CellId::from_token("0000000000000000"), None);
        assert_eq!(S2CellId::from_token("c000000000000000"), None); // face 6
        assert_eq!(S2CellId::from_token("e000000000000000"), None); // face 7
    }

    #[test]
    fn try_constructors_reject_bad_input() {
        assert_eq!(S2CellId::try_from_lat_lon_degrees(f64::NAN, 0.0), None);
        assert_eq!(S2CellId::try_from_lat_lon_degrees(0.0, f64::INFINITY), None);
        assert_eq!(S2CellId::try_from_lat_lon_degrees(91.0, 0.0), None);
        assert_eq!(S2CellId::try_from_lat_lon_degrees(0.0, 181.0), None);
        assert_eq!(S2CellId::try_from_lat_lon(0.0, 0.0, MAX_LEVEL + 1), None);
        assert_eq!(S2CellId::try_from_lat_lon(0.0, 0.0, 255), None);
        assert!(S2CellId::try_from_lat_lon_degrees(90.0, 180.0).is_some());
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn infallible_constructor_still_panics_on_bad_input() {
        let _ = S2CellId::from_lat_lon_degrees(f64::NAN, 0.0);
    }
}

#[cfg(test)]
mod covering_tests {
    use super::*;
    use std::collections::HashSet;
    use tpt_gis_geom::Point;

    fn pts(coords: &[(f64, f64)]) -> Vec<Point> {
        coords.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    /// Reference: every cell a dense lon/lat lattice over the polygon's box (plus margin)
    /// lands in, kept when its centre is inside the polygon.
    fn brute_force(exterior: &[Point], holes: &[&[Point]], level: u8) -> Vec<S2CellId> {
        let polygon = tpt_gis_geom::Polygon::new(exterior, holes);
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in exterior {
            x0 = x0.min(p.x);
            x1 = x1.max(p.x);
            y0 = y0.min(p.y);
            y1 = y1.max(p.y);
        }
        let margin = 0.5;
        let step = 0.003;
        let mut cells = HashSet::new();
        let mut y = y0 - margin;
        while y <= y1 + margin {
            let mut x = x0 - margin;
            while x <= x1 + margin {
                if let Some(cell) =
                    S2CellId::try_from_lat_lon(y.to_radians(), x.to_radians(), level)
                {
                    cells.insert(cell);
                }
                x += step;
            }
            y += step;
        }
        let mut out: Vec<S2CellId> = cells
            .into_iter()
            .filter(|c| {
                let (lat, lon) = c.to_lat_lon_degrees();
                polygon.contains_point(Point::new(lon, lat))
            })
            .collect();
        out.sort_unstable();
        out
    }

    #[test]
    fn neighbour_is_symmetric_and_adjacent_on_every_face() {
        // Sample cells on all six faces, including the corners where four cells meet.
        let seeds = [
            (0.0, 0.0),
            (0.0, 90.0),
            (0.0, 180.0),
            (0.0, -90.0),
            (89.0, 0.0),
            (-89.0, 45.0),
            (45.0, 45.0),
        ];
        for (lat, lon) in seeds {
            let cell = S2CellId::from_lat_lon_degrees(lat, lon).parent(6);
            for (di, dj) in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, -1)] {
                let there = neighbour(cell, di, dj);
                assert_eq!(there.level(), 6);
                assert_eq!(neighbour(there, -di, -dj), cell, "lat {lat} lon {lon}");
            }
            // A direct neighbour's centre is about one cell width away, not across the sphere.
            // Measured as the angle between the centres, since longitude differences blow up
            // near the poles.
            let (clat, clon) = cell.to_lat_lon_degrees();
            let (nlat, nlon) = neighbour(cell, 1, 0).to_lat_lon_degrees();
            let unit = |la: f64, lo: f64| {
                let (la, lo) = (la.to_radians(), lo.to_radians());
                [la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()]
            };
            let (a, b) = (unit(clat, clon), unit(nlat, nlon));
            let angle = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0).acos();
            assert!(angle.to_degrees() < 15.0, "lat {lat} lon {lon}: {}", angle.to_degrees());
        }
    }

    #[test]
    fn convex_polygon_matches_brute_force() {
        // A triangle across the Atlantic coast of France.
        let ext = pts(&[(-2.0, 46.0), (0.5, 46.5), (-0.5, 48.0)]);
        let got = polygon_cells(&ext, &[], 8).unwrap();
        assert!(!got.is_empty());
        assert_eq!(got, brute_force(&ext, &[], 8));
    }

    #[test]
    fn concave_polygon_with_hole_matches_brute_force() {
        let ext = pts(&[
            (10.0, 40.0),
            (11.5, 40.0),
            (11.5, 40.7),
            (10.7, 40.7),
            (10.7, 41.5),
            (10.0, 41.5),
        ]);
        let hole = pts(&[(10.2, 40.2), (10.4, 40.2), (10.4, 40.4), (10.2, 40.4)]);
        let holes: [&[Point]; 1] = [&hole];
        let got = polygon_cells(&ext, &holes, 8).unwrap();
        assert_eq!(got, brute_force(&ext, &holes, 8));
    }

    #[test]
    fn polygon_around_a_cube_corner_matches_brute_force() {
        // The cube corner (1, 1, 1) sits at lat 35.26, lon 45. Cells here are not
        // mutually inverse under neighbour steps, so this checks the covering itself.
        let ext = pts(&[(40.0, 30.0), (50.0, 30.0), (50.0, 40.0), (40.0, 40.0)]);
        let got = polygon_cells(&ext, &[], 5).unwrap();
        assert_eq!(got, brute_force(&ext, &[], 5));
    }

    #[test]
    fn polygon_across_a_cube_edge_is_covered() {
        // Straddles the lon 45 edge between faces 0 and 1 on the equator.
        let ext = pts(&[(40.0, -3.0), (50.0, -3.0), (50.0, 3.0), (40.0, 3.0)]);
        let got = polygon_cells(&ext, &[], 4).unwrap();
        let faces: HashSet<u8> = got.iter().map(|c| c.face()).collect();
        assert!(faces.len() >= 2, "expected several faces, got {faces:?}");
        assert_eq!(got, brute_force(&ext, &[], 4));
    }

    #[test]
    fn degenerate_and_invalid_inputs() {
        let two = pts(&[(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(polygon_cells(&two, &[], 5), Some(Vec::new()));
        let ext = pts(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]);
        assert_eq!(polygon_cells(&ext, &[], MAX_LEVEL + 1), None);
        let bad = pts(&[(0.0, f64::NAN), (1.0, 0.0), (1.0, 1.0)]);
        assert_eq!(polygon_cells(&bad, &[], 5), None);
        let out_of_range = pts(&[(0.0, 95.0), (1.0, 0.0), (1.0, 1.0)]);
        assert_eq!(polygon_cells(&out_of_range, &[], 5), None);
    }
}
