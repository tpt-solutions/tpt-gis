//! Resampling: reading a band onto a different grid.
//!
//! Three kernels are provided, and the choice matters more than it looks:
//!
//! * [`Resampling::Nearest`] copies the value of the source cell containing the
//!   target cell's centre. It is the only correct choice for *categorical*
//!   data — land-cover class 3 averaged with class 5 is not class 4 — and it
//!   preserves the exact set of values present.
//! * [`Resampling::Bilinear`] interpolates the four source cells surrounding
//!   the target centre. It is the right choice for continuous surfaces
//!   (elevation, temperature) when upsampling or resampling at a similar scale.
//! * [`Resampling::Average`] takes the unweighted mean of every source cell
//!   overlapping the target cell. This is the one to use when *downsampling* by
//!   a large factor — bilinear only ever looks at 4 cells, so reducing a
//!   10 000-cell-wide image to 100 cells throws away 99 % of the data and
//!   aliases badly. It is what overview pyramids are built with.
//!
//! # Nodata
//!
//! Nearest and average skip nodata: a target cell is nodata only if every
//! source cell it draws on is. Bilinear is deliberately strict — if any of its
//! four contributors is nodata the result is nodata — because interpolating
//! from a partial stencil invents plausible-looking values along the edge of
//! every data hole. Fill the holes first (with
//! [`algebra::focal_mean`](crate::algebra::focal_mean), say) if that is not
//! wanted.
//!
//! # Example
//!
//! ```
//! use tpt_gis_raster::{resample, Band, GeoTransform, Grid, Resampling};
//!
//! // 4x4 of 1-unit cells, values 0..16.
//! let cells: Vec<f32> = (0..16).map(|i| i as f32).collect();
//! let band = Band::new(Grid::new(4, 4, cells).unwrap(), GeoTransform::new(0.0, 4.0, 1.0, 1.0));
//!
//! // Halve the resolution: each output cell averages a 2x2 source block.
//! let overview = resample::downsample(&band, 2, Resampling::Average).unwrap();
//! assert_eq!(overview.width(), 2);
//! assert_eq!(overview.transform().pixel_width(), 2.0);
//! assert_eq!(overview.get(0, 0), Some(2.5)); // mean of 0, 1, 4, 5
//! ```

use alloc::vec::Vec;

use crate::band::{Band, GeoTransform};
use crate::cell::CellType;
use crate::error::RasterError;
use crate::grid::Grid;

/// The interpolation kernel used when reading a band onto a different grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Resampling {
    /// Copy the source cell containing the target cell's centre. Preserves
    /// exact values; the only safe choice for categorical rasters.
    #[default]
    Nearest,
    /// Weighted average of the four source cells around the target centre.
    /// Strict about nodata (see the module docs).
    Bilinear,
    /// Unweighted mean of every source cell overlapping the target cell. The
    /// right choice for large downsampling factors.
    Average,
}

/// The source value nearest a world position, or `None` outside the band or on
/// a nodata cell.
#[must_use]
pub fn sample_nearest<T: CellType>(band: &Band<T>, world_x: f64, world_y: f64) -> Option<T> {
    band.sample_world(world_x, world_y)
}

/// Bilinearly interpolates a world position, in `f64`.
///
/// Returns `None` if the position is outside the band, or if any of the four
/// surrounding cells is nodata.
///
/// Interpolation runs between cell *centres*, so a position within half a cell
/// of the band's outer edge has no neighbour on that side; the stencil is
/// clamped to the edge there, which reproduces the edge cell's value rather
/// than extrapolating past it.
#[must_use]
pub fn sample_bilinear<T: CellType>(band: &Band<T>, world_x: f64, world_y: f64) -> Option<f64> {
    if band.is_empty() {
        return None;
    }
    let (cell_x, cell_y) = band.transform().world_to_cell(world_x, world_y);
    if !cell_x.is_finite() || !cell_y.is_finite() {
        return None;
    }
    // Reject positions outside the band's footprint before clamping, so that
    // "outside" and "just inside the edge" stay distinguishable.
    let (width, height) = (band.width() as f64, band.height() as f64);
    if cell_x < 0.0 || cell_y < 0.0 || cell_x >= width || cell_y >= height {
        return None;
    }

    // Shift to a centre-relative frame: cell centre (i, j) sits at (i, j) here.
    let (centre_x, centre_y) = (cell_x - 0.5, cell_y - 0.5);
    let (x0, fx) = split_index(centre_x, band.width());
    let (y0, fy) = split_index(centre_y, band.height());
    let x1 = (x0 + 1).min(band.width() - 1);
    let y1 = (y0 + 1).min(band.height() - 1);

    let top_left = band.value_at(x0, y0)?.to_f64();
    let top_right = band.value_at(x1, y0)?.to_f64();
    let bottom_left = band.value_at(x0, y1)?.to_f64();
    let bottom_right = band.value_at(x1, y1)?.to_f64();

    let top = top_left + (top_right - top_left) * fx;
    let bottom = bottom_left + (bottom_right - bottom_left) * fx;
    Some(top + (bottom - top) * fy)
}

/// Splits a centre-relative coordinate into the lower cell index and the
/// fraction towards the next one, clamping at both edges.
fn split_index(coordinate: f64, limit: usize) -> (usize, f64) {
    if coordinate <= 0.0 {
        return (0, 0.0);
    }
    let floor = libm::floor(coordinate);
    let index = floor as usize;
    if index >= limit - 1 {
        return (limit - 1, 0.0);
    }
    (index, coordinate - floor)
}

/// Mean of the valid source cells overlapping the world rectangle
/// `[min_x, max_x] x [min_y, max_y]`, or `None` if it covers no valid cell.
///
/// At least one cell is always considered (the one containing the rectangle's
/// centre), so this degrades to nearest-neighbour when upsampling.
#[must_use]
pub fn sample_average<T: CellType>(
    band: &Band<T>,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> Option<f64> {
    if band.is_empty() {
        return None;
    }
    let transform = band.transform();
    // World y is inverted relative to row order: the *north* edge is row 0.
    let (left, top) = transform.world_to_cell(min_x, max_y);
    let (right, bottom) = transform.world_to_cell(max_x, min_y);
    if ![left, top, right, bottom].iter().all(|value| value.is_finite()) {
        return None;
    }

    let x_start = floor_index(left, band.width())?;
    let y_start = floor_index(top, band.height())?;
    // The far edges are exclusive, so a rectangle landing exactly on a cell
    // boundary does not pull in the next cell along.
    let x_end = ceil_index(right, band.width())?.max(x_start + 1);
    let y_end = ceil_index(bottom, band.height())?.max(y_start + 1);

    let mut sum = 0.0;
    let mut count = 0usize;
    for y in y_start..y_end {
        for x in x_start..x_end {
            if let Some(value) = band.value_at(x, y) {
                sum += value.to_f64();
                count += 1;
            }
        }
    }
    (count > 0).then(|| sum / count as f64)
}

/// Clamps `coordinate` down to a valid cell index, or `None` if the band has no
/// cells in that dimension.
fn floor_index(coordinate: f64, limit: usize) -> Option<usize> {
    if limit == 0 {
        return None;
    }
    if coordinate <= 0.0 {
        return Some(0);
    }
    Some((libm::floor(coordinate) as usize).min(limit - 1))
}

/// Clamps `coordinate` up to an exclusive end index in `1..=limit`, or `None`
/// if the band has no cells in that dimension.
fn ceil_index(coordinate: f64, limit: usize) -> Option<usize> {
    if limit == 0 {
        return None;
    }
    if coordinate <= 0.0 {
        return Some(1);
    }
    Some((libm::ceil(coordinate) as usize).clamp(1, limit))
}

/// Resamples a band onto an arbitrary target grid.
///
/// The target grid is described by its own [`GeoTransform`] and size, so this
/// covers cropping, shifting to a different origin, and changing resolution in
/// one call. Both grids are assumed to be in the same CRS — this crate does not
/// reproject; run the coordinates through `tpt-gis-core` and resample onto the
/// resulting grid if a CRS change is needed.
///
/// The output inherits the source's EPSG code and nodata value. Target cells
/// falling outside the source keep the nodata value (or zero, if none is
/// declared).
///
/// # Errors
/// Returns [`RasterError::EmptyInput`] if the source has no cells,
/// [`RasterError::EmptyOutput`] if the requested size has a zero dimension, or
/// [`RasterError::DimensionsTooLarge`] if `width * height` overflows `usize`.
pub fn resample_to<T: CellType>(
    source: &Band<T>,
    transform: GeoTransform,
    width: usize,
    height: usize,
    method: Resampling,
) -> Result<Band<T>, RasterError> {
    if source.is_empty() {
        return Err(RasterError::EmptyInput);
    }
    if width == 0 || height == 0 {
        return Err(RasterError::EmptyOutput { width, height });
    }
    let count =
        width.checked_mul(height).ok_or(RasterError::DimensionsTooLarge { width, height })?;

    let fill = source.nodata_or_zero();
    let mut cells = Vec::with_capacity(count);
    for y in 0..height {
        for x in 0..width {
            let (centre_x, centre_y) = transform.cell_center_to_world(x, y);
            let value = match method {
                Resampling::Nearest => sample_nearest(source, centre_x, centre_y),
                Resampling::Bilinear => {
                    sample_bilinear(source, centre_x, centre_y).map(T::from_f64)
                }
                Resampling::Average => {
                    let (min_x, max_y) = transform.cell_to_world(x as f64, y as f64);
                    let (max_x, min_y) = transform.cell_to_world(x as f64 + 1.0, y as f64 + 1.0);
                    sample_average(source, min_x, min_y, max_x, max_y).map(T::from_f64)
                }
            };
            cells.push(value.unwrap_or(fill));
        }
    }

    let grid = Grid::new(width, height, cells)?;
    let mut output = Band::new(grid, transform);
    if let Some(epsg) = source.epsg() {
        output = output.with_epsg(epsg);
    }
    if let Some(&nodata) = source.nodata() {
        output = output.with_nodata(nodata);
    }
    Ok(output)
}

/// Resamples a band to a new cell size while covering the same world extent.
///
/// The pixel size is recomputed so that `width x height` cells span exactly the
/// source's extent, so the two rasters stay perfectly co-registered.
///
/// # Errors
/// As [`resample_to`], plus [`RasterError::InvalidPixelSize`] if the derived
/// pixel size is degenerate (only possible for an extent that is not finite).
pub fn resample<T: CellType>(
    source: &Band<T>,
    width: usize,
    height: usize,
    method: Resampling,
) -> Result<Band<T>, RasterError> {
    if source.is_empty() {
        return Err(RasterError::EmptyInput);
    }
    if width == 0 || height == 0 {
        return Err(RasterError::EmptyOutput { width, height });
    }
    let extent = source.extent();
    let transform = GeoTransform::try_new(
        extent.min_x,
        extent.max_y,
        extent.width() / width as f64,
        extent.height() / height as f64,
    )?;
    resample_to(source, transform, width, height, method)
}

/// Downsamples a band by an integer factor, the operation an overview pyramid
/// level is built with.
///
/// Output dimensions are rounded *up* (`ceil(width / factor)`), matching the
/// GDAL overview convention: a 5-cell-wide band at factor 2 becomes 3 cells,
/// with the last one covering only the remaining half block. Cell size grows by
/// exactly `factor` and the origin is unchanged, so every level of the pyramid
/// shares the full-resolution image's top-left corner.
///
/// # Errors
/// Returns [`RasterError::EmptyInput`] if the band has no cells, or
/// [`RasterError::InvalidPixelSize`] if `factor` is zero.
pub fn downsample<T: CellType>(
    source: &Band<T>,
    factor: usize,
    method: Resampling,
) -> Result<Band<T>, RasterError> {
    if source.is_empty() {
        return Err(RasterError::EmptyInput);
    }
    if factor == 0 {
        return Err(RasterError::InvalidPixelSize(0.0));
    }
    let width = source.width().div_ceil(factor);
    let height = source.height().div_ceil(factor);
    let transform = source.transform().downsampled(factor as f64)?;
    resample_to(source, transform, width, height, method)
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// 4x4, 1-unit cells, origin at the top-left of a 4x4 world square, values
    /// 0..16 in row-major order.
    fn ramp() -> Band<f32> {
        let cells: Vec<f32> = (0..16).map(|value| value as f32).collect();
        Band::new(Grid::new(4, 4, cells).unwrap(), GeoTransform::new(0.0, 4.0, 1.0, 1.0))
    }

    #[test]
    fn nearest_copies_the_containing_cell() {
        let band = ramp();
        // Cell (2, 1) spans x in [2, 3), y in [2, 3) world units.
        assert_eq!(sample_nearest(&band, 2.5, 2.5), Some(6.0));
        assert_eq!(sample_nearest(&band, 0.01, 3.99), Some(0.0));
        assert_eq!(sample_nearest(&band, -0.5, 2.0), None);
    }

    #[test]
    fn bilinear_interpolates_between_cell_centres() {
        let band = ramp();
        // Exactly on the centre of cell (1, 1) -> its own value, 5.
        let centre = sample_bilinear(&band, 1.5, 2.5).unwrap();
        assert!((centre - 5.0).abs() < 1e-9);
        // Halfway between the centres of (1, 1) = 5 and (2, 1) = 6.
        let halfway = sample_bilinear(&band, 2.0, 2.5).unwrap();
        assert!((halfway - 5.5).abs() < 1e-9);
        // Halfway diagonally between 5, 6, 9, 10 -> 7.5.
        let diagonal = sample_bilinear(&band, 2.0, 2.0).unwrap();
        assert!((diagonal - 7.5).abs() < 1e-9);
    }

    #[test]
    fn bilinear_clamps_at_the_outer_half_cell_instead_of_extrapolating() {
        let band = ramp();
        // Top-left corner is half a cell outside every centre; the stencil
        // collapses onto cell (0, 0).
        let corner = sample_bilinear(&band, 0.0, 4.0).unwrap();
        assert!((corner - 0.0).abs() < 1e-9);
        // Just outside the footprint is None, not a clamped edge value.
        assert_eq!(sample_bilinear(&band, -0.001, 4.0), None);
        assert_eq!(sample_bilinear(&band, 4.0, 4.0), None);
    }

    #[test]
    fn bilinear_refuses_a_partial_stencil() {
        let cells = vec![1.0f32, 2.0, 3.0, f32::NAN];
        let band =
            Band::new(Grid::new(2, 2, cells).unwrap(), GeoTransform::new(0.0, 2.0, 1.0, 1.0))
                .with_nodata(f32::NAN);
        // Dead centre: all four cells contribute, one of them nodata.
        assert_eq!(sample_bilinear(&band, 1.0, 1.0), None);
        // At the exact top-left corner the bilinear stencil still reaches across
        // into the nodata cell, so the documented strict behaviour applies: None.
        assert_eq!(sample_bilinear(&band, 0.0, 2.0), None);
    }

    #[test]
    fn average_downsampling_takes_the_block_mean() {
        let band = ramp();
        let overview = downsample(&band, 2, Resampling::Average).unwrap();
        assert_eq!(overview.width(), 2);
        assert_eq!(overview.height(), 2);
        assert_eq!(overview.transform().pixel_width(), 2.0);
        assert_eq!(overview.transform().origin_x(), 0.0);
        // Blocks: {0,1,4,5}, {2,3,6,7}, {8,9,12,13}, {10,11,14,15}.
        assert_eq!(overview.grid().cells(), &[2.5, 4.5, 10.5, 12.5]);
    }

    #[test]
    fn average_skips_nodata_within_a_block() {
        let cells = vec![1.0f32, 3.0, -1.0, -1.0];
        let band =
            Band::new(Grid::new(2, 2, cells).unwrap(), GeoTransform::new(0.0, 2.0, 1.0, 1.0))
                .with_nodata(-1.0);
        let overview = downsample(&band, 2, Resampling::Average).unwrap();
        assert_eq!(overview.grid().cells(), &[2.0]);
    }

    #[test]
    fn a_fully_nodata_block_stays_nodata() {
        let band =
            Band::new(Grid::filled(2, 2, -1.0f32).unwrap(), GeoTransform::new(0.0, 2.0, 1.0, 1.0))
                .with_nodata(-1.0);
        let overview = downsample(&band, 2, Resampling::Average).unwrap();
        assert_eq!(overview.grid().cells(), &[-1.0]);
    }

    #[test]
    fn downsampling_rounds_the_size_up_like_gdal_overviews() {
        let band =
            Band::new(Grid::filled(5, 3, 1u8).unwrap(), GeoTransform::new(0.0, 3.0, 1.0, 1.0));
        let overview = downsample(&band, 2, Resampling::Nearest).unwrap();
        assert_eq!((overview.width(), overview.height()), (3, 2));
        assert!(downsample(&band, 0, Resampling::Nearest).is_err());
    }

    #[test]
    fn resample_preserves_the_extent_and_the_metadata() {
        let band = ramp().with_epsg(3857).with_nodata(-1.0);
        let doubled = resample(&band, 8, 8, Resampling::Nearest).unwrap();
        assert_eq!(doubled.transform().pixel_width(), 0.5);
        let (before, after) = (band.extent(), doubled.extent());
        assert!((before.min_x - after.min_x).abs() < 1e-12);
        assert!((before.max_y - after.max_y).abs() < 1e-12);
        assert!((before.max_x - after.max_x).abs() < 1e-12);
        assert!((before.min_y - after.min_y).abs() < 1e-12);
        assert_eq!(doubled.epsg(), Some(3857));
        assert_eq!(doubled.nodata(), Some(&-1.0));
    }

    #[test]
    fn nearest_upsampling_replicates_values_exactly() {
        let band = ramp();
        let doubled = resample(&band, 8, 8, Resampling::Nearest).unwrap();
        assert_eq!(doubled.get(0, 0), Some(0.0));
        assert_eq!(doubled.get(1, 0), Some(0.0));
        assert_eq!(doubled.get(2, 0), Some(1.0));
        // No value was invented: every output cell came from the source.
        let (min, max) = doubled.min_max().unwrap();
        assert_eq!((min, max), (0.0, 15.0));
    }

    #[test]
    fn resample_to_a_shifted_grid_leaves_uncovered_cells_as_nodata() {
        let band = ramp().with_nodata(-1.0);
        // A grid offset 3 cells east: its right half hangs off the source.
        let target = GeoTransform::new(3.0, 4.0, 1.0, 1.0);
        let shifted = resample_to(&band, target, 3, 1, Resampling::Nearest).unwrap();
        assert_eq!(shifted.get(0, 0), Some(3.0));
        assert_eq!(shifted.get(1, 0), Some(-1.0));
        assert_eq!(shifted.get(2, 0), Some(-1.0));
    }

    #[test]
    fn degenerate_requests_are_errors() {
        let band = ramp();
        assert_eq!(
            resample(&band, 0, 4, Resampling::Nearest).unwrap_err(),
            RasterError::EmptyOutput { width: 0, height: 4 }
        );
        let empty = Band::new(
            Grid::<f32>::filled(0, 4, 0.0).unwrap(),
            GeoTransform::new(0.0, 0.0, 1.0, 1.0),
        );
        assert_eq!(
            resample(&empty, 2, 2, Resampling::Nearest).unwrap_err(),
            RasterError::EmptyInput
        );
    }

    #[test]
    fn average_degrades_to_nearest_when_upsampling() {
        let band = ramp();
        let upsampled = resample(&band, 8, 8, Resampling::Average).unwrap();
        // Each output cell covers half a source cell, so the block reduces to
        // the single containing cell.
        assert_eq!(upsampled.get(0, 0), Some(0.0));
        assert_eq!(upsampled.get(3, 0), Some(1.0));
    }
}
