//! [`Band<T>`]: a [`Grid`] that knows where it is on the planet.
//!
//! A band is the unit every other layer of this crate operates on. It bundles
//! three things a bare grid has no opinion about:
//!
//! * a [`GeoTransform`], mapping cell `(x, y)` to a world coordinate;
//! * an optional EPSG code naming the CRS those world coordinates are in;
//! * an optional nodata value, the sentinel that marks "no measurement here".
//!
//! The nodata value is what makes map algebra and resampling more than
//! `Vec::iter().zip()`. A DEM that uses `-9999` for ocean is not a DEM with
//! very deep trenches, and averaging `-9999` into a focal mean poisons every
//! cell within the kernel radius of the coast. Every operation in
//! [`algebra`](crate::algebra) and [`resample`](crate::resample) therefore
//! checks [`Band::is_nodata`] first and propagates the sentinel unchanged.
//!
//! # `GeoTransform` conventions
//!
//! [`GeoTransform`] follows the GDAL six-coefficient convention, restricted to
//! the axis-aligned (north-up) case that covers essentially every GeoTIFF in
//! the wild: the origin is the *outer* corner of the top-left cell, `pixel_width`
//! is positive eastward, and `pixel_height` is stored positive but applied
//! downward (row 0 is the northernmost row). That last part is the classic
//! footgun — GDAL writes it as a negative coefficient and half the bugs in
//! raster code come from applying the sign twice — so this type stores the
//! magnitude and owns the direction itself.

use alloc::vec::Vec;

use crate::cell::{CellKind, CellType};
use crate::error::RasterError;
use crate::grid::Grid;

/// Maps cell coordinates to world coordinates for a north-up raster.
///
/// The mapping is
///
/// ```text
/// world_x = origin_x + x * pixel_width
/// world_y = origin_y - y * pixel_height
/// ```
///
/// where `(x, y)` is the top-left corner of a cell, `(origin_x, origin_y)` is
/// the outer corner of cell `(0, 0)`, and `pixel_height` is positive but
/// applied downward (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoTransform {
    origin_x: f64,
    origin_y: f64,
    pixel_width: f64,
    pixel_height: f64,
}

impl GeoTransform {
    /// Builds a transform from the top-left corner and the pixel size.
    ///
    /// `pixel_width` and `pixel_height` are magnitudes: pass them positive even
    /// though `y` grows southward. A non-finite or non-positive size is
    /// rejected by [`GeoTransform::try_new`]; this constructor clamps nothing
    /// and is meant for literals known to be valid.
    ///
    /// # Panics
    /// Panics if either pixel size is not a positive, finite number.
    #[must_use]
    pub fn new(origin_x: f64, origin_y: f64, pixel_width: f64, pixel_height: f64) -> Self {
        Self::try_new(origin_x, origin_y, pixel_width, pixel_height)
            .expect("pixel sizes must be positive and finite")
    }

    /// Builds a transform, validating the pixel size.
    ///
    /// # Errors
    /// Returns [`RasterError::InvalidPixelSize`] if either size is zero,
    /// negative, or not finite. A zero pixel size would make
    /// [`world_to_cell`](GeoTransform::world_to_cell) divide by zero, and a
    /// negative one would double-apply the north-up sign convention.
    pub fn try_new(
        origin_x: f64,
        origin_y: f64,
        pixel_width: f64,
        pixel_height: f64,
    ) -> Result<Self, RasterError> {
        for size in [pixel_width, pixel_height] {
            if !size.is_finite() || size <= 0.0 {
                return Err(RasterError::InvalidPixelSize(size));
            }
        }
        Ok(Self { origin_x, origin_y, pixel_width, pixel_height })
    }

    /// Builds a transform from GDAL's six-coefficient array
    /// `[origin_x, pixel_width, row_rotation, origin_y, column_rotation,
    /// pixel_height]`, as returned by `GDALGetGeoTransform`.
    ///
    /// GDAL's `pixel_height` (coefficient 5) is negative for a north-up raster;
    /// its magnitude is taken here. Rotation terms must be zero: this type only
    /// models axis-aligned rasters, and silently dropping a rotation would
    /// misplace every cell.
    ///
    /// # Errors
    /// Returns [`RasterError::InvalidPixelSize`] if a pixel size is zero or not
    /// finite, or if either rotation coefficient is non-zero (reported as the
    /// offending rotation value).
    pub fn from_gdal(coefficients: [f64; 6]) -> Result<Self, RasterError> {
        let [origin_x, pixel_width, row_rotation, origin_y, column_rotation, pixel_height] =
            coefficients;
        for rotation in [row_rotation, column_rotation] {
            if rotation != 0.0 {
                return Err(RasterError::InvalidPixelSize(rotation));
            }
        }
        Self::try_new(origin_x, origin_y, pixel_width, pixel_height.abs())
    }

    /// The six-coefficient GDAL form of this transform, with `pixel_height`
    /// negated back to GDAL's north-up convention.
    #[must_use]
    pub const fn to_gdal(self) -> [f64; 6] {
        [self.origin_x, self.pixel_width, 0.0, self.origin_y, 0.0, -self.pixel_height]
    }

    /// World `x` of the outer (left) edge of column 0.
    #[must_use]
    pub const fn origin_x(self) -> f64 {
        self.origin_x
    }

    /// World `y` of the outer (top) edge of row 0.
    #[must_use]
    pub const fn origin_y(self) -> f64 {
        self.origin_y
    }

    /// Cell width in world units, always positive.
    #[must_use]
    pub const fn pixel_width(self) -> f64 {
        self.pixel_width
    }

    /// Cell height in world units, always positive (applied downward).
    #[must_use]
    pub const fn pixel_height(self) -> f64 {
        self.pixel_height
    }

    /// World coordinate of the top-left corner of cell `(x, y)`.
    ///
    /// `x` and `y` are `f64` so that a fractional cell coordinate — what
    /// resampling works in — maps as well as an integer one.
    #[must_use]
    pub fn cell_to_world(self, x: f64, y: f64) -> (f64, f64) {
        (self.origin_x + x * self.pixel_width, self.origin_y - y * self.pixel_height)
    }

    /// World coordinate of the *centre* of cell `(x, y)`.
    ///
    /// This is the one to use when a cell is treated as a sample of a
    /// continuous surface (bilinear resampling, contouring); `cell_to_world`
    /// is the one to use when it is treated as an area (extent maths).
    #[must_use]
    pub fn cell_center_to_world(self, x: usize, y: usize) -> (f64, f64) {
        let (x, y) = (x as f64, y as f64);
        self.cell_to_world(x + 0.5, y + 0.5)
    }

    /// Fractional cell coordinate of a world position — the inverse of
    /// [`cell_to_world`](GeoTransform::cell_to_world).
    ///
    /// The result is not clamped to the grid; a position west or north of the
    /// origin yields a negative coordinate. Callers that need a cell index
    /// should use [`Band::world_to_cell`], which bounds-checks.
    #[must_use]
    pub fn world_to_cell(self, world_x: f64, world_y: f64) -> (f64, f64) {
        (
            (world_x - self.origin_x) / self.pixel_width,
            (self.origin_y - world_y) / self.pixel_height,
        )
    }

    /// The transform of an overview (downsampled) level, where one output cell
    /// covers `factor` by `factor` input cells.
    ///
    /// The origin is unchanged — overviews share the top-left corner of the
    /// full-resolution image — and the pixel size grows by `factor`.
    ///
    /// # Errors
    /// Returns [`RasterError::InvalidPixelSize`] if `factor` is zero or not
    /// finite.
    pub fn downsampled(self, factor: f64) -> Result<Self, RasterError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(RasterError::InvalidPixelSize(factor));
        }
        Self::try_new(
            self.origin_x,
            self.origin_y,
            self.pixel_width * factor,
            self.pixel_height * factor,
        )
    }
}

/// The world-space rectangle a band covers, as
/// `(min_x, min_y, max_x, max_y)`.
///
/// Kept as a plain tuple struct rather than reusing `tpt-gis-geom`'s `BBox` so
/// that this crate does not depend on the geometry crate for one type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    /// Westernmost world `x` (the left edge of column 0).
    pub min_x: f64,
    /// Southernmost world `y` (the bottom edge of the last row).
    pub min_y: f64,
    /// Easternmost world `x` (the right edge of the last column).
    pub max_x: f64,
    /// Northernmost world `y` (the top edge of row 0).
    pub max_y: f64,
}

impl Extent {
    /// Whether a world position falls inside the extent, edges included.
    #[must_use]
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    /// Width in world units.
    #[must_use]
    pub fn width(self) -> f64 {
        self.max_x - self.min_x
    }

    /// Height in world units.
    #[must_use]
    pub fn height(self) -> f64 {
        self.max_y - self.min_y
    }
}

/// A georeferenced, single-band raster: a [`Grid`] plus a [`GeoTransform`], an
/// optional EPSG code, and an optional nodata value.
///
/// [`Raster<T>`] is an alias for this type, for callers who find that name more
/// natural.
#[derive(Debug, Clone, PartialEq)]
pub struct Band<T> {
    grid: Grid<T>,
    transform: GeoTransform,
    epsg: Option<u32>,
    nodata: Option<T>,
}

/// Alias for [`Band<T>`].
pub type Raster<T> = Band<T>;

impl<T> Band<T> {
    /// Wraps a grid with a geotransform, with no CRS and no nodata value.
    ///
    /// Use the `with_*` builders to add them.
    #[must_use]
    pub const fn new(grid: Grid<T>, transform: GeoTransform) -> Self {
        Self { grid, transform, epsg: None, nodata: None }
    }

    /// Sets the EPSG code of the CRS the geotransform's world coordinates are
    /// in.
    #[must_use]
    pub fn with_epsg(mut self, epsg: u32) -> Self {
        self.epsg = Some(epsg);
        self
    }

    /// Sets the nodata sentinel.
    #[must_use]
    pub fn with_nodata(mut self, nodata: T) -> Self {
        self.nodata = Some(nodata);
        self
    }

    /// Removes the nodata sentinel, making every cell valid data.
    #[must_use]
    pub fn without_nodata(mut self) -> Self {
        self.nodata = None;
        self
    }

    /// The underlying grid.
    #[must_use]
    pub const fn grid(&self) -> &Grid<T> {
        &self.grid
    }

    /// The underlying grid, mutably. The geotransform is unaffected, so
    /// resizing the grid through this handle would desynchronise the two —
    /// [`Grid`]'s API deliberately offers no resize.
    #[must_use]
    pub fn grid_mut(&mut self) -> &mut Grid<T> {
        &mut self.grid
    }

    /// Consumes the band, returning its grid.
    #[must_use]
    pub fn into_grid(self) -> Grid<T> {
        self.grid
    }

    /// The geotransform.
    #[must_use]
    pub const fn transform(&self) -> GeoTransform {
        self.transform
    }

    /// The EPSG code, if one was set.
    #[must_use]
    pub const fn epsg(&self) -> Option<u32> {
        self.epsg
    }

    /// The nodata sentinel, if one was set.
    #[must_use]
    pub const fn nodata(&self) -> Option<&T> {
        self.nodata.as_ref()
    }

    /// Width in cells.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.grid.width()
    }

    /// Height in cells.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.grid.height()
    }

    /// Whether the band has no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.grid.is_empty()
    }

    /// The world-space rectangle the band covers, from its outer cell edges.
    #[must_use]
    pub fn extent(&self) -> Extent {
        let (width, height) = (self.width() as f64, self.height() as f64);
        let (max_x, min_y) = self.transform.cell_to_world(width, height);
        Extent { min_x: self.transform.origin_x(), min_y, max_x, max_y: self.transform.origin_y() }
    }

    /// The integer cell containing a world position, or `None` if the position
    /// falls outside the band.
    ///
    /// The east and south edges are exclusive, so tiling a region by cell never
    /// double-counts a boundary position.
    #[must_use]
    pub fn world_to_cell(&self, world_x: f64, world_y: f64) -> Option<(usize, usize)> {
        let (x, y) = self.transform.world_to_cell(world_x, world_y);
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
            return None;
        }
        let (column, row) = (libm::floor(x) as usize, libm::floor(y) as usize);
        if column >= self.width() || row >= self.height() {
            return None;
        }
        Some((column, row))
    }
}

impl<T: CellType> Band<T> {
    /// The value of cell `(x, y)`, or `None` if it is out of bounds.
    ///
    /// A nodata cell still returns `Some(nodata)`; use
    /// [`value_at`](Band::value_at) to have nodata collapse to `None`.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<T> {
        self.grid.value(x, y)
    }

    /// Whether `value` is this band's nodata sentinel.
    ///
    /// Uses [`CellType::same_value`], so a `NaN` sentinel matches a `NaN` cell
    /// even though `NaN != NaN`.
    #[must_use]
    pub fn is_nodata(&self, value: T) -> bool {
        self.nodata.is_some_and(|nodata| value.same_value(nodata))
    }

    /// The *valid* value of cell `(x, y)`: `None` if the cell is out of bounds
    /// **or** holds nodata.
    #[must_use]
    pub fn value_at(&self, x: usize, y: usize) -> Option<T> {
        let value = self.get(x, y)?;
        (!self.is_nodata(value)).then_some(value)
    }

    /// The valid value at a world position, or `None` outside the band or on a
    /// nodata cell.
    #[must_use]
    pub fn sample_world(&self, world_x: f64, world_y: f64) -> Option<T> {
        let (x, y) = self.world_to_cell(world_x, world_y)?;
        self.value_at(x, y)
    }

    /// The nodata sentinel if one is set, otherwise [`CellType::ZERO`].
    ///
    /// This is what an operation writes into an output cell it cannot compute:
    /// with no sentinel declared there is nothing better to say than zero, and
    /// the alternative — refusing to run — would make nodata mandatory.
    #[must_use]
    pub fn nodata_or_zero(&self) -> T {
        self.nodata.unwrap_or(T::ZERO)
    }

    /// Number of cells holding valid (non-nodata) data.
    #[must_use]
    pub fn valid_count(&self) -> usize {
        match self.nodata {
            None => self.grid.len(),
            Some(nodata) => {
                self.grid.cells().iter().filter(|cell| !cell.same_value(nodata)).count()
            }
        }
    }

    /// Minimum and maximum of the valid cells, or `None` if every cell is
    /// nodata (or the band is empty).
    ///
    /// `NaN` cells in a float band are skipped even when nodata is not `NaN`:
    /// a `NaN` cannot participate in an ordering, and including it would make
    /// the result depend on iteration order.
    #[must_use]
    pub fn min_max(&self) -> Option<(T, T)> {
        let mut bounds: Option<(T, T)> = None;
        for &cell in self.grid.cells() {
            if self.is_nodata(cell) || cell.to_f64().is_nan() {
                continue;
            }
            bounds = Some(match bounds {
                None => (cell, cell),
                Some((min, max)) => {
                    (if cell < min { cell } else { min }, if cell > max { cell } else { max })
                }
            });
        }
        bounds
    }

    /// Mean of the valid cells, or `None` if there are none.
    ///
    /// Accumulated in `f64` regardless of the cell type, so a large `u8` band
    /// does not overflow partway through.
    #[must_use]
    pub fn mean(&self) -> Option<f64> {
        let mut sum = 0.0;
        let mut count = 0usize;
        for &cell in self.grid.cells() {
            if self.is_nodata(cell) || cell.to_f64().is_nan() {
                continue;
            }
            sum += cell.to_f64();
            count += 1;
        }
        (count > 0).then(|| sum / count as f64)
    }

    /// Builds a band of the same shape, geotransform, CRS, and nodata value,
    /// with `cells` as its contents.
    ///
    /// Used internally by map algebra to keep the metadata attached to a
    /// derived result.
    ///
    /// # Errors
    /// Returns [`RasterError::CellCountMismatch`] if `cells.len()` is not
    /// `width * height`.
    pub fn with_same_shape(&self, cells: Vec<T>) -> Result<Self, RasterError> {
        Ok(Self {
            grid: Grid::new(self.width(), self.height(), cells)?,
            transform: self.transform,
            epsg: self.epsg,
            nodata: self.nodata,
        })
    }

    /// The runtime cell type tag of this band.
    #[must_use]
    pub const fn kind(&self) -> CellKind {
        T::KIND
    }
}

/// A [`Band`] whose cell type is only known at runtime — what a file reader
/// returns, since the sample type lives in the file's header rather than in the
/// caller's types.
///
/// Match on it, or use one of the `into_*` accessors when a specific type is
/// expected.
#[derive(Debug, Clone, PartialEq)]
pub enum AnyBand {
    /// An 8-bit unsigned band.
    U8(Band<u8>),
    /// A 16-bit unsigned band.
    U16(Band<u16>),
    /// A 16-bit signed band — the usual DEM encoding.
    I16(Band<i16>),
    /// A 32-bit unsigned band.
    U32(Band<u32>),
    /// A 32-bit signed band.
    I32(Band<i32>),
    /// A 32-bit float band.
    F32(Band<f32>),
    /// A 64-bit float band.
    F64(Band<f64>),
}

/// Generates the shared accessors and the `into_*`/`as_*` pairs for [`AnyBand`].
macro_rules! any_band_dispatch {
    ($($variant:ident => $ty:ty, $into:ident, $as:ident),+ $(,)?) => {
        impl AnyBand {
            /// The runtime cell type of the wrapped band.
            #[must_use]
            pub const fn kind(&self) -> CellKind {
                match self {
                    $(AnyBand::$variant(_) => CellKind::$variant,)+
                }
            }

            /// Width in cells.
            #[must_use]
            pub const fn width(&self) -> usize {
                match self {
                    $(AnyBand::$variant(band) => band.width(),)+
                }
            }

            /// Height in cells.
            #[must_use]
            pub const fn height(&self) -> usize {
                match self {
                    $(AnyBand::$variant(band) => band.height(),)+
                }
            }

            /// The geotransform of the wrapped band.
            #[must_use]
            pub const fn transform(&self) -> GeoTransform {
                match self {
                    $(AnyBand::$variant(band) => band.transform(),)+
                }
            }

            /// The EPSG code of the wrapped band, if known.
            #[must_use]
            pub const fn epsg(&self) -> Option<u32> {
                match self {
                    $(AnyBand::$variant(band) => band.epsg(),)+
                }
            }

            /// The world-space rectangle the wrapped band covers.
            #[must_use]
            pub fn extent(&self) -> Extent {
                match self {
                    $(AnyBand::$variant(band) => band.extent(),)+
                }
            }

            /// The value of cell `(x, y)` widened to `f64`, or `None` if the
            /// cell is out of bounds or holds nodata.
            ///
            /// The type-erased way to read a single cell when the caller only
            /// wants a number.
            #[must_use]
            pub fn value_at_f64(&self, x: usize, y: usize) -> Option<f64> {
                match self {
                    $(AnyBand::$variant(band) => band.value_at(x, y).map(CellType::to_f64),)+
                }
            }

            $(
                /// Unwraps the band if it holds this cell type, otherwise
                /// returns `self` unchanged so another variant can be tried.
                ///
                /// # Errors
                /// Returns the original `AnyBand` as the error value on a type
                /// mismatch.
                pub fn $into(self) -> Result<Band<$ty>, Self> {
                    match self {
                        AnyBand::$variant(band) => Ok(band),
                        other => Err(other),
                    }
                }

                /// Borrows the band if it holds this cell type.
                #[must_use]
                pub const fn $as(&self) -> Option<&Band<$ty>> {
                    match self {
                        AnyBand::$variant(band) => Some(band),
                        _ => None,
                    }
                }
            )+
        }

        $(
            impl From<Band<$ty>> for AnyBand {
                fn from(band: Band<$ty>) -> Self {
                    AnyBand::$variant(band)
                }
            }
        )+
    };
}

any_band_dispatch! {
    U8 => u8, into_u8, as_u8,
    U16 => u16, into_u16, as_u16,
    I16 => i16, into_i16, as_i16,
    U32 => u32, into_u32, as_u32,
    I32 => i32, into_i32, as_i32,
    F32 => f32, into_f32, as_f32,
    F64 => f64, into_f64, as_f64,
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// A 3x2 band at 10-unit resolution with its top-left corner at
    /// (100, 200), `-9999` as nodata, and one nodata cell at (1, 0):
    ///
    /// ```text
    ///   1  -9999  3
    ///   4      5  6
    /// ```
    fn sample() -> Band<i16> {
        let grid = Grid::new(3, 2, vec![1, -9999, 3, 4, 5, 6]).unwrap();
        Band::new(grid, GeoTransform::new(100.0, 200.0, 10.0, 10.0))
            .with_epsg(32633)
            .with_nodata(-9999)
    }

    #[test]
    fn cell_and_world_coordinates_round_trip() {
        let transform = GeoTransform::new(100.0, 200.0, 10.0, 10.0);
        // Row 1 is *south* of row 0, so world y decreases as y increases.
        assert_eq!(transform.cell_to_world(2.0, 1.0), (120.0, 190.0));
        assert_eq!(transform.cell_center_to_world(0, 0), (105.0, 195.0));
        let (x, y) = transform.world_to_cell(120.0, 190.0);
        assert!((x - 2.0).abs() < 1e-12 && (y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn gdal_coefficients_round_trip_with_a_negative_pixel_height() {
        let transform = GeoTransform::from_gdal([100.0, 10.0, 0.0, 200.0, 0.0, -10.0]).unwrap();
        assert_eq!(transform.pixel_height(), 10.0);
        assert_eq!(transform.to_gdal(), [100.0, 10.0, 0.0, 200.0, 0.0, -10.0]);
    }

    #[test]
    fn rotated_or_degenerate_transforms_are_rejected() {
        // A rotation term would misplace every cell, so it is an error rather
        // than something to silently drop.
        assert!(GeoTransform::from_gdal([0.0, 1.0, 0.5, 0.0, 0.0, -1.0]).is_err());
        assert_eq!(
            GeoTransform::try_new(0.0, 0.0, 0.0, 1.0).unwrap_err(),
            RasterError::InvalidPixelSize(0.0)
        );
        assert!(GeoTransform::try_new(0.0, 0.0, 1.0, -1.0).is_err());
        assert!(GeoTransform::try_new(0.0, 0.0, 1.0, f64::NAN).is_err());
    }

    #[test]
    fn extent_spans_outer_cell_edges() {
        let extent = sample().extent();
        assert_eq!(extent.min_x, 100.0);
        assert_eq!(extent.max_x, 130.0);
        assert_eq!(extent.max_y, 200.0);
        assert_eq!(extent.min_y, 180.0);
        assert_eq!(extent.width(), 30.0);
        assert!(extent.contains(100.0, 200.0));
        assert!(!extent.contains(131.0, 200.0));
    }

    #[test]
    fn world_to_cell_excludes_the_south_and_east_edges() {
        let band = sample();
        assert_eq!(band.world_to_cell(100.0, 200.0), Some((0, 0)));
        assert_eq!(band.world_to_cell(129.9, 180.1), Some((2, 1)));
        // Exactly on the far edges: outside, so adjacent tiles don't overlap.
        assert_eq!(band.world_to_cell(130.0, 200.0), None);
        assert_eq!(band.world_to_cell(100.0, 180.0), None);
        assert_eq!(band.world_to_cell(99.0, 200.0), None);
        assert_eq!(band.world_to_cell(f64::NAN, 200.0), None);
    }

    #[test]
    fn nodata_cells_read_as_none_but_are_still_present() {
        let band = sample();
        assert_eq!(band.get(1, 0), Some(-9999));
        assert_eq!(band.value_at(1, 0), None);
        assert_eq!(band.value_at(0, 0), Some(1));
        assert_eq!(band.value_at(9, 9), None);
        assert!(band.is_nodata(-9999));
        assert!(!band.is_nodata(1));
        assert_eq!(band.sample_world(105.0, 195.0), Some(1));
        assert_eq!(band.sample_world(115.0, 195.0), None);
    }

    #[test]
    fn statistics_skip_nodata() {
        let band = sample();
        assert_eq!(band.valid_count(), 5);
        assert_eq!(band.min_max(), Some((1, 6)));
        assert_eq!(band.mean(), Some((1.0 + 3.0 + 4.0 + 5.0 + 6.0) / 5.0));

        let all_nodata =
            Band::new(Grid::filled(2, 2, -9999i16).unwrap(), GeoTransform::new(0.0, 0.0, 1.0, 1.0))
                .with_nodata(-9999);
        assert_eq!(all_nodata.min_max(), None);
        assert_eq!(all_nodata.mean(), None);
        assert_eq!(all_nodata.valid_count(), 0);
    }

    #[test]
    fn nan_nodata_matches_nan_cells() {
        let grid = Grid::new(2, 1, vec![1.5f32, f32::NAN]).unwrap();
        let band = Band::new(grid, GeoTransform::new(0.0, 0.0, 1.0, 1.0)).with_nodata(f32::NAN);
        assert!(band.is_nodata(f32::NAN));
        assert_eq!(band.value_at(1, 0), None);
        assert_eq!(band.valid_count(), 1);
        assert_eq!(band.min_max(), Some((1.5, 1.5)));
    }

    #[test]
    fn nan_is_excluded_from_statistics_even_without_a_nan_sentinel() {
        let grid = Grid::new(3, 1, vec![1.0f64, f64::NAN, 3.0]).unwrap();
        let band = Band::new(grid, GeoTransform::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(band.min_max(), Some((1.0, 3.0)));
        assert_eq!(band.mean(), Some(2.0));
    }

    #[test]
    fn downsampled_transform_keeps_the_origin_and_scales_the_pixel() {
        let overview = GeoTransform::new(100.0, 200.0, 10.0, 10.0).downsampled(4.0).unwrap();
        assert_eq!(overview.origin_x(), 100.0);
        assert_eq!(overview.pixel_width(), 40.0);
        assert!(GeoTransform::new(0.0, 0.0, 1.0, 1.0).downsampled(0.0).is_err());
    }

    #[test]
    fn any_band_dispatches_without_the_caller_knowing_the_type() {
        let erased: AnyBand = sample().into();
        assert_eq!(erased.kind(), CellKind::I16);
        assert_eq!(erased.width(), 3);
        assert_eq!(erased.height(), 2);
        assert_eq!(erased.epsg(), Some(32633));
        assert_eq!(erased.value_at_f64(0, 0), Some(1.0));
        assert_eq!(erased.value_at_f64(1, 0), None);
        assert!(erased.as_u8().is_none());
        assert!(erased.as_i16().is_some());
        let erased = erased.into_u8().unwrap_err();
        assert!(erased.into_i16().is_ok());
    }
}
