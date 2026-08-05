//! S2 spherical grid integration.
//!
//! S2 subdivides the unit sphere into six cube faces, each recursively
//! quartered into a quadtree of cells. A cell is identified by a 64-bit
//! [`S2CellId`](S2CellId) that packs the face (3 bits) and, level by level, the
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

    /// The cell's level, 0 (largest) .. [`MAX_LEVEL`] (smallest).
    #[must_use]
    pub fn level(self) -> u8 {
        // The trailing `1` marker sits at bit 2 * (MAX_LEVEL - level).
        let trailing = self.0.trailing_zeros() / 2;
        MAX_LEVEL - trailing as u8
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
    #[must_use]
    pub fn from_lat_lon_degrees(lat: f64, lon: f64) -> Self {
        Self::from_lat_lon(lat.to_radians(), lon.to_radians(), MAX_LEVEL)
    }

    /// The cell containing the given latitude/longitude (radians) at `level`.
    #[must_use]
    pub fn from_lat_lon(lat: f64, lon: f64, level: u8) -> Self {
        let x = lat.cos() * lon.cos();
        let y = lat.cos() * lon.sin();
        let z = lat.sin();
        let (face, u, v) = face_uv_from_xyz(x, y, z);
        // `u`/`v` are cube-space coordinates; convert them back to the linear `s`/`t`
        // space that `st_to_ij` maps to cell indices. Skipping this inverse quadratic
        // transform would map sphere points to the wrong cells.
        let s = uv_to_st(u);
        let t = uv_to_st(v);
        let shift = MAX_LEVEL - level;
        let i = st_to_ij(s) >> shift as u32;
        let j = st_to_ij(t) >> shift as u32;
        Self::from_face_ij(face, i, j, level)
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
    /// string is not 16 hex digits.
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
        Some(S2CellId(value))
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
        _ => (-u, -v, 1.0),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
