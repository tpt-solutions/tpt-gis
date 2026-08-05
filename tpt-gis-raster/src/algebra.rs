//! Map algebra: cell-wise (local) operations, reclassification, and
//! neighbourhood (focal) statistics.
//!
//! Every function here is nodata-aware, which is the whole point. The naive
//! version of `add` is `a.zip(b).map(|(a, b)| a + b)`; run that over a DEM that
//! encodes ocean as `-9999` and you get `-19998`, a number that is neither a
//! depth nor a nodata marker, and the error spreads from there. The rules used
//! throughout:
//!
//! * **Local operations** produce nodata wherever *either* input cell is
//!   nodata.
//! * **Focal operations** ignore nodata neighbours rather than counting them as
//!   zero, and produce nodata only when the whole window is nodata (or, for
//!   [`focal_kernel`], when any contributing cell is missing — see below).
//! * The output's sentinel is the left/only input's nodata value, falling back
//!   to [`CellType::ZERO`](crate::CellType::ZERO) when the input declares none.
//!   With no sentinel there is no way to say "unknown", so an operation that
//!   would produce one writes zero; declare a nodata value if that matters.
//!
//! Integer arithmetic saturates instead of wrapping
//! ([`CellType::saturating_add`](crate::CellType::saturating_add) and friends):
//! wrapping a `u8` slope raster from 255 to 0 turns a cliff into a plain.
//!
//! # Example
//!
//! ```
//! use tpt_gis_raster::{algebra, Band, GeoTransform, Grid};
//!
//! // A 3x3 elevation band with a nodata hole in the middle.
//! let grid = Grid::new(3, 3, vec![1i16, 2, 3, 4, -9999, 6, 7, 8, 9]).unwrap();
//! let dem = Band::new(grid, GeoTransform::new(0.0, 0.0, 1.0, 1.0)).with_nodata(-9999);
//!
//! // A 3x3 focal mean fills the hole from its neighbours...
//! let smoothed = algebra::focal_mean(&dem, 1).unwrap();
//! assert_eq!(smoothed.get(1, 1), Some(5)); // mean of 1,2,3,4,6,7,8,9
//!
//! // ...but a local operation keeps it a hole.
//! let doubled = algebra::add(&dem, &dem).unwrap();
//! assert_eq!(doubled.get(1, 1), Some(-9999));
//! ```

use alloc::vec::Vec;

use crate::band::Band;
use crate::cell::CellType;
use crate::error::RasterError;
use crate::grid::Grid;

/// Applies a cell-wise operation to two bands of the same size.
///
/// `op` is called only for cell pairs where both inputs are valid; wherever
/// either is nodata the output is `lhs`'s sentinel (or zero, see the module
/// docs) and `op` is not called at all — so `op` never has to recognise a
/// sentinel itself.
///
/// # Errors
/// Returns [`RasterError::ShapeMismatch`] if the two bands differ in size. Map
/// algebra is strictly cell-wise; resample one operand onto the other's grid
/// first with [`resample`](crate::resample).
pub fn binary<T: CellType>(
    lhs: &Band<T>,
    rhs: &Band<T>,
    mut op: impl FnMut(T, T) -> T,
) -> Result<Band<T>, RasterError> {
    if lhs.width() != rhs.width() || lhs.height() != rhs.height() {
        return Err(RasterError::ShapeMismatch {
            lhs: (lhs.width(), lhs.height()),
            rhs: (rhs.width(), rhs.height()),
        });
    }

    let fill = lhs.nodata_or_zero();
    let mut cells = Vec::with_capacity(lhs.grid().len());
    for (&left, &right) in lhs.grid().cells().iter().zip(rhs.grid().cells()) {
        let value =
            if lhs.is_nodata(left) || rhs.is_nodata(right) { fill } else { op(left, right) };
        cells.push(value);
    }
    lhs.with_same_shape(cells)
}

/// Applies a cell-wise operation to every valid cell of one band.
///
/// Nodata cells pass through untouched, so `op` only ever sees real data.
#[must_use]
pub fn unary<T: CellType>(band: &Band<T>, mut op: impl FnMut(T) -> T) -> Band<T> {
    let cells: Vec<T> = band
        .grid()
        .cells()
        .iter()
        .map(|&cell| if band.is_nodata(cell) { cell } else { op(cell) })
        .collect();
    band.with_same_shape(cells).expect("mapped cell count matches the input")
}

/// Cell-wise sum, saturating at the cell type's bounds.
///
/// # Errors
/// Returns [`RasterError::ShapeMismatch`] if the bands differ in size.
pub fn add<T: CellType>(lhs: &Band<T>, rhs: &Band<T>) -> Result<Band<T>, RasterError> {
    binary(lhs, rhs, CellType::saturating_add)
}

/// Cell-wise difference (`lhs - rhs`), saturating at the cell type's bounds.
///
/// # Errors
/// Returns [`RasterError::ShapeMismatch`] if the bands differ in size.
pub fn subtract<T: CellType>(lhs: &Band<T>, rhs: &Band<T>) -> Result<Band<T>, RasterError> {
    binary(lhs, rhs, CellType::saturating_sub)
}

/// Cell-wise product, saturating at the cell type's bounds.
///
/// # Errors
/// Returns [`RasterError::ShapeMismatch`] if the bands differ in size.
pub fn multiply<T: CellType>(lhs: &Band<T>, rhs: &Band<T>) -> Result<Band<T>, RasterError> {
    binary(lhs, rhs, CellType::saturating_mul)
}

/// Cell-wise quotient (`lhs / rhs`), computed in `f64` and narrowed back.
///
/// Division by zero yields nodata rather than a panic (integers) or an infinity
/// (floats): a ratio against an empty denominator is genuinely unknown, which
/// is what nodata means.
///
/// # Errors
/// Returns [`RasterError::ShapeMismatch`] if the bands differ in size.
pub fn divide<T: CellType>(lhs: &Band<T>, rhs: &Band<T>) -> Result<Band<T>, RasterError> {
    let fill = lhs.nodata_or_zero();
    binary(lhs, rhs, move |left, right| {
        let divisor = right.to_f64();
        if divisor == 0.0 {
            fill
        } else {
            T::from_f64(left.to_f64() / divisor)
        }
    })
}

/// Cell-wise minimum.
///
/// # Errors
/// Returns [`RasterError::ShapeMismatch`] if the bands differ in size.
pub fn minimum<T: CellType>(lhs: &Band<T>, rhs: &Band<T>) -> Result<Band<T>, RasterError> {
    binary(lhs, rhs, |left, right| if right < left { right } else { left })
}

/// Cell-wise maximum.
///
/// # Errors
/// Returns [`RasterError::ShapeMismatch`] if the bands differ in size.
pub fn maximum<T: CellType>(lhs: &Band<T>, rhs: &Band<T>) -> Result<Band<T>, RasterError> {
    binary(lhs, rhs, |left, right| if right > left { right } else { left })
}

/// Multiplies every valid cell by a scalar, in `f64`.
///
/// The usual use is applying a TIFF `GDAL_SCALE`/`GDAL_OFFSET` pair or
/// converting units (centimetres to metres, Kelvin to Celsius when paired with
/// [`offset`]).
#[must_use]
pub fn scale<T: CellType>(band: &Band<T>, factor: f64) -> Band<T> {
    unary(band, |cell| T::from_f64(cell.to_f64() * factor))
}

/// Adds a scalar to every valid cell, in `f64`.
#[must_use]
pub fn offset<T: CellType>(band: &Band<T>, delta: f64) -> Band<T> {
    unary(band, |cell| T::from_f64(cell.to_f64() + delta))
}

/// Clamps every valid cell into `[low, high]`.
///
/// # Errors
/// Returns [`RasterError::InvalidRange`] if `low > high`, which would otherwise
/// clamp everything to a single arbitrary end.
pub fn clamp<T: CellType>(band: &Band<T>, low: T, high: T) -> Result<Band<T>, RasterError> {
    if low > high {
        return Err(RasterError::InvalidRange);
    }
    Ok(unary(band, |cell| {
        if cell < low {
            low
        } else if cell > high {
            high
        } else {
            cell
        }
    }))
}

/// One rule of a [`reclassify`] table: the half-open input range
/// `[min, max)` maps to `to`.
///
/// Half-open ranges are what make a contiguous table like
/// `0..500 => 1, 500..1000 => 2` unambiguous — with inclusive upper bounds,
/// elevation 500 would match both rules.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReclassRule<T> {
    /// Inclusive lower bound.
    pub min: T,
    /// Exclusive upper bound.
    pub max: T,
    /// Value written when a cell falls in the range.
    pub to: T,
}

impl<T: CellType> ReclassRule<T> {
    /// Builds a rule mapping `[min, max)` to `to`.
    #[must_use]
    pub const fn new(min: T, max: T, to: T) -> Self {
        Self { min, max, to }
    }

    /// Whether `value` falls in this rule's range.
    #[must_use]
    pub fn matches(&self, value: T) -> bool {
        value >= self.min && value < self.max
    }
}

/// Reclassifies a band through a rule table.
///
/// Rules are tried in order and the first match wins, so overlapping ranges
/// resolve by priority rather than being rejected. A valid cell matching no
/// rule takes `fallback`, or becomes nodata if `fallback` is `None`. Nodata
/// cells stay nodata — reclassification is about data values, and a table that
/// happened to span `-9999` should not resurrect the ocean.
#[must_use]
pub fn reclassify<T: CellType>(
    band: &Band<T>,
    rules: &[ReclassRule<T>],
    fallback: Option<T>,
) -> Band<T> {
    let fill = band.nodata_or_zero();
    unary(band, |cell| {
        rules
            .iter()
            .find(|rule| rule.matches(cell))
            .map_or_else(|| fallback.unwrap_or(fill), |rule| rule.to)
    })
}

/// A neighbourhood statistic computed by [`focal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocalStat {
    /// Arithmetic mean of the valid cells in the window — a low-pass smoother.
    Mean,
    /// Smallest valid cell in the window (grey-scale erosion).
    Min,
    /// Largest valid cell in the window (grey-scale dilation).
    Max,
    /// Sum of the valid cells in the window.
    Sum,
    /// `Max - Min`, a cheap local-relief / roughness measure.
    Range,
    /// Population standard deviation of the valid cells — local texture.
    StdDev,
    /// Median of the valid cells. Unlike [`Mean`](FocalStat::Mean) it removes
    /// salt-and-pepper speckle without smearing edges, at the cost of sorting
    /// each window.
    Median,
}

/// Computes a neighbourhood statistic over a square window of side
/// `2 * radius + 1`.
///
/// The window shrinks at the edges of the band rather than being padded: a
/// corner cell of a `radius = 1` window sees 4 neighbours, not 9 with 5 invented
/// zeros. Nodata neighbours are skipped the same way. An output cell is nodata
/// only when its window contains no valid cell at all — which, note, means a
/// nodata *hole* smaller than the window gets filled in from its surroundings.
/// That is the standard behaviour and is usually what a smoothing pass is for,
/// but if holes must stay holes, mask the result against the input afterwards.
///
/// A `radius` of 0 makes every window a single cell, so the result equals the
/// input for every statistic except [`FocalStat::Range`] (0) and
/// [`FocalStat::StdDev`] (0).
///
/// # Errors
/// Returns [`RasterError::EmptyInput`] if the band has no cells.
pub fn focal<T: CellType>(
    band: &Band<T>,
    radius: usize,
    stat: FocalStat,
) -> Result<Band<T>, RasterError> {
    if band.is_empty() {
        return Err(RasterError::EmptyInput);
    }

    let fill = band.nodata_or_zero();
    let (width, height) = (band.width(), band.height());
    // Reused across windows so a large band does not allocate per cell.
    let mut window = Vec::with_capacity((2 * radius + 1) * (2 * radius + 1));
    let mut cells = Vec::with_capacity(band.grid().len());

    for y in 0..height {
        let y_lo = y.saturating_sub(radius);
        let y_hi = (y + radius).min(height - 1);
        for x in 0..width {
            let x_lo = x.saturating_sub(radius);
            let x_hi = (x + radius).min(width - 1);

            window.clear();
            for wy in y_lo..=y_hi {
                for wx in x_lo..=x_hi {
                    if let Some(value) = band.value_at(wx, wy) {
                        window.push(value.to_f64());
                    }
                }
            }
            cells.push(window_stat(&mut window, stat).map_or(fill, T::from_f64));
        }
    }

    band.with_same_shape(cells)
}

/// Reduces one window's valid values to a statistic, or `None` if the window is
/// entirely nodata. `values` may be reordered (the median sorts in place).
fn window_stat(values: &mut [f64], stat: FocalStat) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let count = values.len() as f64;
    let result = match stat {
        FocalStat::Sum => values.iter().sum(),
        FocalStat::Mean => values.iter().sum::<f64>() / count,
        FocalStat::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
        FocalStat::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        FocalStat::Range => {
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            max - min
        }
        FocalStat::StdDev => {
            let mean = values.iter().sum::<f64>() / count;
            let variance =
                values.iter().map(|value| (value - mean) * (value - mean)).sum::<f64>() / count;
            libm::sqrt(variance)
        }
        FocalStat::Median => {
            // Every value came from `value_at`, so no NaN reaches this point
            // unless the cell type is float and NaN is real data; `total_cmp`
            // keeps the sort total either way.
            values.sort_unstable_by(f64::total_cmp);
            let middle = values.len() / 2;
            if values.len() % 2 == 0 {
                (values[middle - 1] + values[middle]) / 2.0
            } else {
                values[middle]
            }
        }
    };
    Some(result)
}

/// Focal mean over a `2 * radius + 1` square window. See [`focal`].
///
/// # Errors
/// Returns [`RasterError::EmptyInput`] if the band has no cells.
pub fn focal_mean<T: CellType>(band: &Band<T>, radius: usize) -> Result<Band<T>, RasterError> {
    focal(band, radius, FocalStat::Mean)
}

/// Focal minimum over a `2 * radius + 1` square window. See [`focal`].
///
/// # Errors
/// Returns [`RasterError::EmptyInput`] if the band has no cells.
pub fn focal_min<T: CellType>(band: &Band<T>, radius: usize) -> Result<Band<T>, RasterError> {
    focal(band, radius, FocalStat::Min)
}

/// Focal maximum over a `2 * radius + 1` square window. See [`focal`].
///
/// # Errors
/// Returns [`RasterError::EmptyInput`] if the band has no cells.
pub fn focal_max<T: CellType>(band: &Band<T>, radius: usize) -> Result<Band<T>, RasterError> {
    focal(band, radius, FocalStat::Max)
}

/// Convolves a band with a weight kernel.
///
/// The kernel must have odd width and height so it has a well-defined centre;
/// weight `(i, j)` multiplies the cell `(i - kw / 2, j - kh / 2)` away from the
/// output cell.
///
/// Unlike [`focal`], this is strict: if any cell the kernel reaches is off the
/// edge of the band or holds nodata, the output cell is nodata. Renormalising a
/// partial window is only meaningful for an averaging kernel — do it to a
/// Sobel or Laplacian kernel and the answer is silently wrong — so the choice
/// is left to the caller, who knows what the weights mean.
///
/// # Errors
/// Returns [`RasterError::EmptyInput`] if the band or the kernel is empty, or
/// [`RasterError::EvenKernel`] if a kernel dimension is even.
pub fn focal_kernel<T: CellType>(
    band: &Band<T>,
    kernel: &Grid<f64>,
) -> Result<Band<T>, RasterError> {
    if band.is_empty() {
        return Err(RasterError::EmptyInput);
    }
    if kernel.is_empty() {
        return Err(RasterError::EmptyInput);
    }
    if kernel.width() % 2 == 0 || kernel.height() % 2 == 0 {
        return Err(RasterError::EvenKernel { width: kernel.width(), height: kernel.height() });
    }

    let fill = band.nodata_or_zero();
    let (half_w, half_h) = (kernel.width() / 2, kernel.height() / 2);
    let mut cells = Vec::with_capacity(band.grid().len());

    for y in 0..band.height() {
        for x in 0..band.width() {
            let mut accumulator = 0.0;
            let mut complete = true;
            'window: for (ky, weights) in kernel.rows().enumerate() {
                let Some(sy) = (y + ky).checked_sub(half_h) else {
                    complete = false;
                    break 'window;
                };
                for (kx, &weight) in weights.iter().enumerate() {
                    let Some(sx) = (x + kx).checked_sub(half_w) else {
                        complete = false;
                        break 'window;
                    };
                    let Some(value) = band.value_at(sx, sy) else {
                        complete = false;
                        break 'window;
                    };
                    accumulator += weight * value.to_f64();
                }
            }
            cells.push(if complete { T::from_f64(accumulator) } else { fill });
        }
    }

    band.with_same_shape(cells)
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;
    use crate::band::GeoTransform;

    fn transform() -> GeoTransform {
        GeoTransform::new(0.0, 0.0, 1.0, 1.0)
    }

    fn band(width: usize, height: usize, cells: Vec<i16>) -> Band<i16> {
        Band::new(Grid::new(width, height, cells).unwrap(), transform()).with_nodata(-9999)
    }

    #[test]
    fn local_operations_propagate_nodata_instead_of_computing_with_it() {
        let a = band(2, 1, vec![10, -9999]);
        let b = band(2, 1, vec![5, 5]);

        assert_eq!(add(&a, &b).unwrap().grid().cells(), &[15, -9999]);
        assert_eq!(subtract(&a, &b).unwrap().grid().cells(), &[5, -9999]);
        assert_eq!(multiply(&a, &b).unwrap().grid().cells(), &[50, -9999]);
        assert_eq!(divide(&a, &b).unwrap().grid().cells(), &[2, -9999]);
        assert_eq!(minimum(&a, &b).unwrap().grid().cells(), &[5, -9999]);
        assert_eq!(maximum(&a, &b).unwrap().grid().cells(), &[10, -9999]);
    }

    #[test]
    fn local_operations_saturate_rather_than_wrap() {
        let grid = Grid::new(2, 1, vec![250u8, 5]).unwrap();
        let a = Band::new(grid, transform());
        let sum = add(&a, &a).unwrap();
        assert_eq!(sum.grid().cells(), &[255, 10]);
        let difference = subtract(
            &Band::new(Grid::new(1, 1, vec![5u8]).unwrap(), transform()),
            &Band::new(Grid::new(1, 1, vec![10u8]).unwrap(), transform()),
        )
        .unwrap();
        assert_eq!(difference.grid().cells(), &[0]);
    }

    #[test]
    fn division_by_zero_yields_nodata() {
        let a = band(2, 1, vec![10, 10]);
        let b = band(2, 1, vec![0, 2]);
        assert_eq!(divide(&a, &b).unwrap().grid().cells(), &[-9999, 5]);
    }

    #[test]
    fn shape_mismatch_is_an_error_not_a_truncated_result() {
        let a = band(2, 1, vec![1, 2]);
        let b = band(1, 2, vec![1, 2]);
        assert_eq!(
            add(&a, &b).unwrap_err(),
            RasterError::ShapeMismatch { lhs: (2, 1), rhs: (1, 2) }
        );
    }

    #[test]
    fn output_keeps_the_georeferencing_of_the_left_operand() {
        let a =
            Band::new(Grid::new(1, 1, vec![1i16]).unwrap(), GeoTransform::new(5.0, 6.0, 2.0, 2.0))
                .with_epsg(4326)
                .with_nodata(-1);
        let sum = add(&a, &a).unwrap();
        assert_eq!(sum.transform(), a.transform());
        assert_eq!(sum.epsg(), Some(4326));
        assert_eq!(sum.nodata(), Some(&-1));
    }

    #[test]
    fn without_a_declared_sentinel_unknown_results_fall_back_to_zero() {
        let a = Band::new(Grid::new(1, 1, vec![1i16]).unwrap(), transform());
        let zero = Band::new(Grid::new(1, 1, vec![0i16]).unwrap(), transform());
        assert_eq!(divide(&a, &zero).unwrap().grid().cells(), &[0]);
    }

    #[test]
    fn scale_and_offset_round_to_nearest_and_skip_nodata() {
        let a = band(3, 1, vec![1, 2, -9999]);
        assert_eq!(scale(&a, 2.5).grid().cells(), &[3, 5, -9999]);
        assert_eq!(offset(&a, 10.0).grid().cells(), &[11, 12, -9999]);
    }

    #[test]
    fn clamp_bounds_valid_cells_only() {
        let a = band(4, 1, vec![-5, 3, 20, -9999]);
        assert_eq!(clamp(&a, 0, 10).unwrap().grid().cells(), &[0, 3, 10, -9999]);
        assert!(clamp(&a, 10, 0).is_err());
    }

    #[test]
    fn reclassify_uses_half_open_ranges_and_first_match_wins() {
        let a = band(5, 1, vec![0, 499, 500, 1500, -9999]);
        let rules = [ReclassRule::new(0, 500, 1), ReclassRule::new(500, 1000, 2)];

        let classified = reclassify(&a, &rules, Some(9));
        assert_eq!(classified.grid().cells(), &[1, 1, 2, 9, -9999]);

        // No fallback: unmatched valid cells become nodata, and the existing
        // nodata cell is untouched.
        let classified = reclassify(&a, &rules, None);
        assert_eq!(classified.grid().cells(), &[1, 1, 2, -9999, -9999]);
    }

    #[test]
    fn focal_mean_ignores_nodata_neighbours_rather_than_treating_them_as_zero() {
        // 1 2 3
        // 4 X 6      X = nodata
        // 7 8 9
        let a = band(3, 3, vec![1, 2, 3, 4, -9999, 6, 7, 8, 9]);
        let smoothed = focal_mean(&a, 1).unwrap();
        // Centre: mean of the 8 valid neighbours = 40 / 8 = 5.
        assert_eq!(smoothed.get(1, 1), Some(5));
        // Top-left corner: window shrinks to 1,2,4 (X skipped) = 7 / 3 -> 2.
        assert_eq!(smoothed.get(0, 0), Some(2));
    }

    #[test]
    fn focal_statistics_over_a_shrinking_window() {
        let a = band(3, 1, vec![1, 5, 9]);
        assert_eq!(focal_min(&a, 1).unwrap().grid().cells(), &[1, 1, 5]);
        assert_eq!(focal_max(&a, 1).unwrap().grid().cells(), &[5, 9, 9]);
        assert_eq!(focal(&a, 1, FocalStat::Sum).unwrap().grid().cells(), &[6, 15, 14]);
        assert_eq!(focal(&a, 1, FocalStat::Range).unwrap().grid().cells(), &[4, 8, 4]);
        assert_eq!(focal(&a, 1, FocalStat::Median).unwrap().grid().cells(), &[3, 5, 7]);
    }

    #[test]
    fn focal_stddev_matches_the_population_formula() {
        let grid = Grid::new(3, 1, vec![2.0f64, 4.0, 6.0]).unwrap();
        let a = Band::new(grid, transform());
        let deviation = focal(&a, 1, FocalStat::StdDev).unwrap();
        // Full window {2, 4, 6}: mean 4, variance 8/3.
        let expected = libm::sqrt(8.0 / 3.0);
        assert!((deviation.get(1, 0).unwrap() - expected).abs() < 1e-12);
    }

    #[test]
    fn focal_over_an_all_nodata_window_stays_nodata() {
        let a = band(2, 1, vec![-9999, -9999]);
        assert_eq!(focal_mean(&a, 1).unwrap().grid().cells(), &[-9999, -9999]);
    }

    #[test]
    fn focal_radius_zero_is_the_identity_for_the_mean() {
        let a = band(3, 1, vec![1, 5, 9]);
        assert_eq!(focal_mean(&a, 0).unwrap().grid().cells(), a.grid().cells());
    }

    #[test]
    fn focal_on_an_empty_band_is_an_error() {
        let a = Band::new(Grid::<i16>::filled(0, 3, 0).unwrap(), transform());
        assert_eq!(focal_mean(&a, 1).unwrap_err(), RasterError::EmptyInput);
    }

    #[test]
    fn focal_kernel_convolves_and_refuses_partial_windows() {
        // 3x3 box blur with weights summing to 1.
        let ninth = 1.0 / 9.0;
        let kernel = Grid::filled(3, 3, ninth).unwrap();
        let a = band(3, 3, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let blurred = focal_kernel(&a, &kernel).unwrap();
        // Only the centre has a complete window; the border is nodata.
        assert_eq!(blurred.get(1, 1), Some(5));
        assert_eq!(blurred.get(0, 0), Some(-9999));
        assert_eq!(blurred.get(2, 2), Some(-9999));
    }

    #[test]
    fn focal_kernel_rejects_an_even_sized_kernel() {
        let kernel = Grid::filled(2, 3, 1.0).unwrap();
        let a = band(3, 3, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(
            focal_kernel(&a, &kernel).unwrap_err(),
            RasterError::EvenKernel { width: 2, height: 3 }
        );
    }

    #[test]
    fn focal_kernel_treats_a_nodata_neighbour_as_a_missing_contribution() {
        let kernel =
            Grid::new(3, 3, vec![0.0, -1.0, 0.0, -1.0, 4.0, -1.0, 0.0, -1.0, 0.0]).unwrap();
        let a = band(3, 3, vec![1, 2, 3, 4, 5, -9999, 7, 8, 9]);
        assert_eq!(focal_kernel(&a, &kernel).unwrap().get(1, 1), Some(-9999));
    }

    #[test]
    fn nan_nodata_propagates_through_local_operations() {
        let grid = Grid::new(2, 1, vec![1.0f32, f32::NAN]).unwrap();
        let a = Band::new(grid, transform()).with_nodata(f32::NAN);
        let sum = add(&a, &a).unwrap();
        assert_eq!(sum.get(0, 0), Some(2.0));
        assert!(sum.get(1, 0).unwrap().is_nan());
    }
}
