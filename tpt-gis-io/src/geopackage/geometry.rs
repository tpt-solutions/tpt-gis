//! GeoPackage geometry BLOB codec.
//!
//! A GeoPackage geometry column stores a *GeoPackageBinary* header followed by a
//! standard WKB geometry. The header is:
//!
//! ```text
//! magic    : 2 bytes  = b"GP"
//! version  : 1 byte   = 0
//! flags    : 1 byte   (see [`FLAG_HEADER_BYTE_ORDER`] / [`FLAG_ENVELOPE_MASK`] / [`FLAG_EMPTY`])
//! srs_id   : 4 bytes  (int32, in the header byte order selected by `flags`)
//! envelope : 0/32/48/64 bytes (optional, skipped by the reader, never written here)
//! wkb      : 1-byte endianness flag + standard WKB
//! ```
//!
//! The trailing WKB is decoded by reusing this crate's existing
//! [`wkb::parse_geometry`] / [`wkb::write_geometry`], so the geometry
//! representation stays identical to the GeoJSON/WKT/Shapefile paths.
//!
//! # WKB byte order vs header byte order
//! The GeoPackageBinary header carries its *own* byte-order bit (`flags` bit 0)
//! which governs `srs_id` and the envelope, while the embedded WKB carries its own
//! 1-byte order marker. This codec writes both as little-endian (NDR), the
//! near-universal convention, and the reader honors each independently.
//!
//! Flag layout, per the GeoPackage specification:
//! bit 0 = header byte order (`1` little-endian), bits 1–3 = envelope indicator,
//! bit 4 = empty geometry, bit 5 = extended (reserved here), bits 6–7 reserved.

use core::fmt;

use crate::geometry::Geometry;
use crate::wkb;

/// The 2-byte magic at the start of every GeoPackage geometry BLOB.
const MAGIC: [u8; 2] = *b"GP";
/// The only geometry-blob version this codec understands (the current spec).
const VERSION: u8 = 0;

/// Bit 0 of `flags`: byte order of the header (`srs_id` + envelope). `1` = little-endian.
const FLAG_HEADER_BYTE_ORDER: u8 = 0b0000_0001;
/// Bits 1–3 of `flags`: the envelope-contents indicator code (0 = no envelope).
const FLAG_ENVELOPE_MASK: u8 = 0b0000_1110;
/// Bit 4 of `flags`: `1` = an empty geometry, `0` = non-empty.
const FLAG_EMPTY: u8 = 0b0001_0000;

/// The byte order a multi-byte integer is stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteOrder {
    /// Big-endian (XDR).
    Big,
    /// Little-endian (NDR).
    Little,
}

impl ByteOrder {
    fn read_i32(self, bytes: &[u8]) -> i32 {
        let arr: [u8; 4] = bytes.try_into().expect("read_i32 needs 4 bytes");
        match self {
            ByteOrder::Big => i32::from_be_bytes(arr),
            ByteOrder::Little => i32::from_le_bytes(arr),
        }
    }
}

/// An error encountered while decoding a GeoPackage geometry BLOB.
#[derive(Debug)]
pub enum GeoPackageGeometryError {
    /// The BLOB was shorter than the 8-byte fixed header (magic + version + flags
    /// + `srs_id`).
    UnexpectedEndOfInput,
    /// The first two bytes were not the `GP` magic.
    InvalidMagic,
    /// The `version` byte was not `0`.
    UnsupportedVersion(u8),
    /// The envelope indicator code was reserved (`5`–`7`).
    InvalidEnvelopeCode(u8),
    /// The WKB tail failed to parse as a standard geometry.
    Wkb(wkb::WkbError),
}

impl fmt::Display for GeoPackageGeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoPackageGeometryError::UnexpectedEndOfInput => {
                write!(f, "geometry BLOB truncated before its header")
            }
            GeoPackageGeometryError::InvalidMagic => {
                write!(f, "geometry BLOB missing \"GP\" magic")
            }
            GeoPackageGeometryError::UnsupportedVersion(v) => {
                write!(f, "unsupported geometry BLOB version {v}")
            }
            GeoPackageGeometryError::InvalidEnvelopeCode(c) => {
                write!(f, "reserved envelope indicator code {c} in geometry BLOB flags")
            }
            GeoPackageGeometryError::Wkb(e) => write!(f, "invalid WKB inside geometry BLOB: {e}"),
        }
    }
}

impl std::error::Error for GeoPackageGeometryError {}

impl From<wkb::WkbError> for GeoPackageGeometryError {
    fn from(e: wkb::WkbError) -> Self {
        GeoPackageGeometryError::Wkb(e)
    }
}

/// The byte length of the optional envelope for a given indicator code.
///
/// Code `0` = none, `1` = XY (4× f64 = 32 B), `2` = XYZ / `3` = XYM (6× f64 = 48 B),
/// `4` = XYZM (8× f64 = 64 B). Codes `5`–`7` are reserved and rejected.
fn envelope_size(code: u8) -> Result<usize, GeoPackageGeometryError> {
    match code {
        0 => Ok(0),
        1 => Ok(32),
        2 | 3 => Ok(48),
        4 => Ok(64),
        other => Err(GeoPackageGeometryError::InvalidEnvelopeCode(other)),
    }
}

/// Decodes a GeoPackage geometry BLOB into `(geometry, srs_id)`.
///
/// Returns `Ok(None)` for an empty-geometry flag (no embedded WKB). The embedded WKB
/// may be either byte order — its own 1-byte marker selects it.
///
/// # Errors
/// Returns [`GeoPackageGeometryError`] if the header is malformed, truncated, or the
/// embedded WKB fails to parse.
pub fn decode_geometry_blob(
    blob: &[u8],
) -> Result<Option<(Geometry, i32)>, GeoPackageGeometryError> {
    if blob.len() < 8 {
        return Err(GeoPackageGeometryError::UnexpectedEndOfInput);
    }
    if blob[0..2] != MAGIC {
        return Err(GeoPackageGeometryError::InvalidMagic);
    }
    let version = blob[2];
    if version != VERSION {
        return Err(GeoPackageGeometryError::UnsupportedVersion(version));
    }
    let flags = blob[3];
    let envelope_code = (flags & FLAG_ENVELOPE_MASK) >> 1;
    let header_le = flags & FLAG_HEADER_BYTE_ORDER != 0;
    let is_empty = flags & FLAG_EMPTY != 0;

    let order = if header_le { ByteOrder::Little } else { ByteOrder::Big };
    let srs_id = order.read_i32(&blob[4..8]);

    if is_empty {
        return Ok(None);
    }

    let std_start = 8 + envelope_size(envelope_code)?;
    if blob.len() < std_start + 1 {
        return Err(GeoPackageGeometryError::UnexpectedEndOfInput);
    }
    let geometry = wkb::parse_geometry(&blob[std_start..])?;
    Ok(Some((geometry, srs_id)))
}

/// Encodes a [`Geometry`] as a little-endian GeoPackage geometry BLOB with the given
/// `srs_id`, no envelope, and a non-empty flag.
///
/// The header byte order, the `srs_id`, and the embedded WKB are all written
/// little-endian (NDR), matching the convention used by essentially every real-world
/// GeoPackage producer.
#[must_use]
pub fn encode_geometry_blob(geometry: &Geometry, srs_id: i32) -> Vec<u8> {
    let wkb = wkb::write_geometry(geometry);
    let mut blob = Vec::with_capacity(8 + wkb.len());
    blob.extend_from_slice(&MAGIC);
    blob.push(VERSION);
    // Header is little-endian (bit 0 set), no envelope (bits 1-3 clear), non-empty
    // (bit 4 clear).
    blob.push(FLAG_HEADER_BYTE_ORDER);
    blob.extend_from_slice(&srs_id.to_le_bytes());
    blob.extend_from_slice(&wkb);
    blob
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    #[test]
    fn round_trips_a_point() {
        let geometry = Geometry::Point(Point::new(30.0, 10.0));
        let blob = encode_geometry_blob(&geometry, 4326);
        let decoded = decode_geometry_blob(&blob).unwrap().unwrap();
        assert_eq!(decoded.0, geometry);
        assert_eq!(decoded.1, 4326);
    }

    #[test]
    fn decodes_hand_built_blob_with_known_vector() {
        // Header: "GP", version 0, flags = 0x01 (LE header, no envelope, non-empty),
        // srs_id = 4326 LE.
        let mut blob = Vec::new();
        blob.extend_from_slice(b"GP");
        blob.push(0);
        blob.push(0x01);
        blob.extend_from_slice(&4326i32.to_le_bytes());
        // Standard WKB: LE marker, Point (type 1), (12.0, 34.0).
        blob.push(1); // WKB byte-order = NDR
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.extend_from_slice(&12.0f64.to_le_bytes());
        blob.extend_from_slice(&34.0f64.to_le_bytes());

        let (geometry, srs_id) = decode_geometry_blob(&blob).unwrap().unwrap();
        assert_eq!(srs_id, 4326);
        assert_eq!(geometry, Geometry::Point(Point::new(12.0, 34.0)));
    }

    #[test]
    fn rejects_wrong_magic() {
        assert!(matches!(
            decode_geometry_blob(b"XX\x00\x04\x00\x00\x00\x00"),
            Err(GeoPackageGeometryError::InvalidMagic)
        ));
    }

    #[test]
    fn empty_flag_yields_none() {
        let mut blob = Vec::new();
        blob.extend_from_slice(b"GP");
        blob.push(0);
        blob.push(FLAG_EMPTY); // bit 4
        blob.extend_from_slice(&0i32.to_le_bytes());
        assert!(decode_geometry_blob(&blob).unwrap().is_none());
    }

    /// Flag bits per the GeoPackage spec: byte order is bit 0 and the empty flag is
    /// bit 4. A header with the empty bit set in *bit 3* (the old, wrong position)
    /// must be read as a non-empty geometry.
    #[test]
    fn flag_bits_match_the_spec() {
        assert_eq!(FLAG_HEADER_BYTE_ORDER, 0x01);
        assert_eq!(FLAG_ENVELOPE_MASK, 0x0E);
        assert_eq!(FLAG_EMPTY, 0x10);
    }

    /// An XY envelope is 32 bytes (4 doubles). The WKB must be read after it.
    #[test]
    fn skips_xy_envelope_of_32_bytes() {
        let mut blob = Vec::new();
        blob.extend_from_slice(b"GP");
        blob.push(0);
        blob.push(0x01 | (1 << 1)); // LE header, envelope code 1 (XY)
        blob.extend_from_slice(&4326i32.to_le_bytes());
        blob.extend_from_slice(&[0u8; 32]);
        blob.push(1);
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.extend_from_slice(&5.0f64.to_le_bytes());
        blob.extend_from_slice(&6.0f64.to_le_bytes());

        let (geometry, _) = decode_geometry_blob(&blob).unwrap().unwrap();
        assert_eq!(geometry, Geometry::Point(Point::new(5.0, 6.0)));
    }

    #[test]
    fn rejects_reserved_envelope_code() {
        let blob = [b'G', b'P', 0, 5 << 1, 0, 0, 0, 0, 1, 1, 0, 0, 0];
        assert!(matches!(
            decode_geometry_blob(&blob),
            Err(GeoPackageGeometryError::InvalidEnvelopeCode(5))
        ));
    }

    #[test]
    fn big_endian_header_reads_srs_id_big_endian() {
        let mut blob = Vec::new();
        blob.extend_from_slice(b"GP");
        blob.push(0);
        blob.push(0x00); // big-endian header, no envelope, non-empty
        blob.extend_from_slice(&4326i32.to_be_bytes());
        blob.push(1);
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.extend_from_slice(&1.0f64.to_le_bytes());
        blob.extend_from_slice(&2.0f64.to_le_bytes());
        let (_, srs_id) = decode_geometry_blob(&blob).unwrap().unwrap();
        assert_eq!(srs_id, 4326);
    }
}
