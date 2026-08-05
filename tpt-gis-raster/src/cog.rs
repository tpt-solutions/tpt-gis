//! COG (Cloud Optimized GeoTIFF) layout validation.
//!
//! A COG is a GeoTIFF rearranged so a client can read only the parts it needs
//! over HTTP range requests: the IFD and the tile offset/byte-count arrays sit
//! near the start, every image is tiled, tile data is laid out in order
//! (*internal tiling*), and a pyramid of overviews (SubIFDs) lets a client pick a
//! resolution that already fits its viewport. This module inspects those
//! properties without decoding any pixels.
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use tpt_gis_raster::cog::validate;
//! // `bytes` is a Cloud Optimized GeoTIFF.
//! # let bytes = tpt_gis_raster::geotiff::testsupport::make_uncompressed_tiff();
//! let report = validate(&bytes).unwrap();
//! // The fixture is a stripped, single-image TIFF, so it is *not* a valid COG
//! // (a COG must be tiled). The report says so explicitly.
//! assert!(!report.is_valid());
//! # }
//! ```

use crate::geotiff::{GeoTiff, GeoTiffError, ImageLayout};

/// A property of a (candidate) COG that is wrong or merely suboptimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CogIssue {
    /// The image is stored as strips, not tiles. A COG must be tiled. (error)
    NotTiled,
    /// Tile/strip offsets are not monotonically increasing, so the file cannot be
    /// streamed sequentially. (error)
    NotInternal,
    /// A tile/strip offset or byte count falls outside the file. (error)
    DataOutOfRange,
    /// The full-resolution image is larger than 512x512 but has no overviews.
    /// (warning)
    MissingOverviews,
    /// An overview image is itself not tiled. (warning)
    OverviewNotTiled,
    /// No georeferencing (model pixel scale / tiepoint / transformation) was
    /// found. (warning)
    MissingGeoreferencing,
}

impl CogIssue {
    /// Whether this issue is fatal to COG validity (as opposed to a warning).
    #[must_use]
    pub const fn is_error(self) -> bool {
        matches!(self, CogIssue::NotTiled | CogIssue::NotInternal | CogIssue::DataOutOfRange)
    }
}

/// The result of validating a COG.
#[derive(Debug, Clone)]
pub struct CogReport {
    /// The full-resolution image's layout.
    pub full_resolution: ImageLayout,
    /// The pyramid of overviews, ordered finest-to-coarsest (the order they appear
    /// in the SubIFD list).
    pub overviews: Vec<ImageLayout>,
    /// Issues found, paired with whether each is an error or a warning.
    pub issues: Vec<(CogIssue, bool)>,
}

impl CogReport {
    /// Whether the file satisfies the hard requirements of a COG (no error-level
    /// issues). Warnings do not make it invalid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.issues.iter().all(|(_, is_error)| !*is_error)
    }
}

/// Validates the COG layout of the (Geo)TIFF bytes in `data`.
///
/// # Errors
/// Returns the underlying [`GeoTiffError`] if the file cannot be parsed at all.
pub fn validate(data: &[u8]) -> Result<CogReport, GeoTiffError> {
    let tiff = GeoTiff::parse(data)?;
    let file_len = tiff.file_len();
    let full = tiff.layout(0)?;
    let mut issues = Vec::new();

    if !full.tiled {
        issues.push((CogIssue::NotTiled, true));
    }
    if full.tiled && !full.internal_tiling {
        issues.push((CogIssue::NotInternal, true));
    }
    if full.last_data_offset > file_len {
        issues.push((CogIssue::DataOutOfRange, true));
    }
    if !full.has_geo_transform {
        issues.push((CogIssue::MissingGeoreferencing, false));
    }

    // Walk the overview (SubIFD) pyramid, finest-to-coarsest.
    let mut overviews = Vec::new();
    let mut frontier = full.subifds.clone();
    for _ in 0..64 {
        if frontier.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for offset in frontier {
            let layout = match tiff.layout_at(offset) {
                Ok(layout) => layout,
                Err(_) => continue,
            };
            if layout.last_data_offset > file_len {
                issues.push((CogIssue::DataOutOfRange, true));
            }
            if !layout.tiled {
                issues.push((CogIssue::OverviewNotTiled, false));
            }
            next.extend(layout.subifds.iter().copied());
            overviews.push(layout);
        }
        frontier = next;
    }

    // A COG is expected to carry overviews once the full image is larger than a
    // single 512x512 footprint. This is a warning, not an error: a small COG (or a
    // deliberately flat one) can be valid without them. It is only checked for the
    // full-resolution image, not for overviews.
    let large = full.width > 512 || full.height > 512;
    if large && overviews.is_empty() {
        issues.push((CogIssue::MissingOverviews, false));
    }

    Ok(CogReport { full_resolution: full, overviews, issues })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geotiff::testsupport::make_cog;

    #[test]
    fn a_stripped_single_image_is_not_a_valid_cog() {
        // The fixture is stripped and has no overviews, so it fails the hard
        // tiling requirement.
        let report = validate(&crate::geotiff::testsupport::make_uncompressed_tiff()).unwrap();
        assert!(!report.is_valid());
        assert!(report
            .issues
            .iter()
            .any(|(issue, is_error)| *issue == CogIssue::NotTiled && *is_error));
    }

    #[test]
    fn a_tiled_georeferenced_image_passes_cog_validation() {
        // 256x256, tiled 128x128, georeferenced -> a minimal but valid COG.
        let bytes = make_cog(256, 256, 128, 128, |c, r| ((c + r) % 256) as u8);
        let report = validate(&bytes).unwrap();
        assert!(report.is_valid(), "issues: {:?}", report.issues);
        assert!(report.full_resolution.tiled);
        assert!(report.full_resolution.has_geo_transform);
        assert_eq!(report.full_resolution.epsg, Some(3857));
    }

    #[test]
    fn cog_validation_reports_data_out_of_range() {
        let mut bytes = make_cog(256, 256, 128, 128, |c, r| ((c + r) % 256) as u8);
        // Truncate the file so the last tile's data falls outside the file.
        bytes.truncate(bytes.len() - 100);
        let report = validate(&bytes).unwrap();
        assert!(!report.is_valid());
        assert!(report
            .issues
            .iter()
            .any(|(issue, is_error)| *issue == CogIssue::DataOutOfRange && *is_error));
    }
}
