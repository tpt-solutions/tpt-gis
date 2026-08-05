//! [`Grid<T>`]: a dense, row-major 2D array of cells.
//!
//! A grid is pure geometry-free storage: `width * height` cells of a single
//! [`CellType`], laid out one row after another, with no georeferencing and no
//! nodata value. Pair it with a [`GeoTransform`](crate::GeoTransform) in a
//! [`Band`](crate::Band) to get something that knows where it is on the planet.
//!
//! Row-major with no per-row indirection matters here: a GeoTIFF tile decodes
//! into a contiguous run of bytes, and resampling and focal statistics both walk
//! whole rows, so a `Vec<Vec<T>>` would cost a pointer chase per row for
//! nothing.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Index, IndexMut};

use crate::error::RasterError;

/// A dense, row-major 2D array of cells.
///
/// Cell `(x, y)` — column `x`, row `y`, both zero-based from the top-left — is
/// stored at index `y * width + x`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Grid<T> {
    width: usize,
    height: usize,
    cells: Vec<T>,
}

/// Computes `width * height`, reporting an error rather than overflowing.
fn cell_count(width: usize, height: usize) -> Result<usize, RasterError> {
    width.checked_mul(height).ok_or(RasterError::DimensionsTooLarge { width, height })
}

impl<T> Grid<T> {
    /// Builds a grid from an existing row-major cell buffer.
    ///
    /// # Errors
    /// Returns [`RasterError::CellCountMismatch`] if `cells.len()` is not
    /// `width * height`, or [`RasterError::DimensionsTooLarge`] if that product
    /// overflows `usize`.
    pub fn new(width: usize, height: usize, cells: Vec<T>) -> Result<Self, RasterError> {
        let expected = cell_count(width, height)?;
        if cells.len() != expected {
            return Err(RasterError::CellCountMismatch { width, height, cells: cells.len() });
        }
        Ok(Self { width, height, cells })
    }

    /// Builds a grid by calling `f` for every cell, in row-major order.
    ///
    /// # Errors
    /// Returns [`RasterError::DimensionsTooLarge`] if `width * height`
    /// overflows `usize`.
    pub fn from_fn(
        width: usize,
        height: usize,
        mut f: impl FnMut(usize, usize) -> T,
    ) -> Result<Self, RasterError> {
        let count = cell_count(width, height)?;
        let mut cells = Vec::with_capacity(count);
        for y in 0..height {
            for x in 0..width {
                cells.push(f(x, y));
            }
        }
        Ok(Self { width, height, cells })
    }

    /// Width of the grid, in cells.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Height of the grid, in cells.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Total number of cells (`width * height`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the grid has no cells at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The flat buffer index of cell `(x, y)`, or `None` if it is out of
    /// bounds.
    #[must_use]
    pub fn index_of(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y * self.width + x)
    }

    /// A reference to cell `(x, y)`, or `None` if it is out of bounds.
    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<&T> {
        self.cells.get(self.index_of(x, y)?)
    }

    /// A mutable reference to cell `(x, y)`, or `None` if it is out of bounds.
    #[must_use]
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut T> {
        let index = self.index_of(x, y)?;
        self.cells.get_mut(index)
    }

    /// Overwrites cell `(x, y)`.
    ///
    /// # Errors
    /// Returns [`RasterError::OutOfBounds`] if the coordinate is outside the
    /// grid.
    pub fn set(&mut self, x: usize, y: usize, value: T) -> Result<(), RasterError> {
        let width = self.width;
        let height = self.height;
        let slot = self
            .get_mut(x, y)
            .ok_or(RasterError::OutOfBounds { x, y, width, height })?;
        *slot = value;
        Ok(())
    }

    /// Row `y` as a contiguous slice, or `None` if `y` is out of bounds.
    #[must_use]
    pub fn row(&self, y: usize) -> Option<&[T]> {
        if y >= self.height {
            return None;
        }
        self.cells.get(y * self.width..(y + 1) * self.width)
    }

    /// Row `y` as a contiguous mutable slice, or `None` if `y` is out of
    /// bounds.
    #[must_use]
    pub fn row_mut(&mut self, y: usize) -> Option<&mut [T]> {
        if y >= self.height {
            return None;
        }
        self.cells.get_mut(y * self.width..(y + 1) * self.width)
    }

    /// Iterates the rows of the grid, top to bottom.
    pub fn rows(&self) -> impl Iterator<Item = &[T]> {
        // `chunks` panics on a zero chunk size; a zero-width grid has no cells,
        // so any non-zero size yields the same (empty) iterator.
        self.cells.chunks(self.width.max(1))
    }

    /// All cells, in row-major order.
    #[must_use]
    pub fn cells(&self) -> &[T] {
        &self.cells
    }

    /// All cells, in row-major order, mutably.
    #[must_use]
    pub fn cells_mut(&mut self) -> &mut [T] {
        &mut self.cells
    }

    /// Consumes the grid, returning its cell buffer.
    #[must_use]
    pub fn into_cells(self) -> Vec<T> {
        self.cells
    }

    /// Applies `f` to every cell, producing a grid of the same size holding the
    /// results.
    pub fn map<U>(&self, mut f: impl FnMut(&T) -> U) -> Grid<U> {
        Grid {
            width: self.width,
            height: self.height,
            cells: self.cells.iter().map(&mut f).collect(),
        }
    }
}

impl<T: Clone> Grid<T> {
    /// Builds a grid with every cell set to `value`.
    ///
    /// # Errors
    /// Returns [`RasterError::DimensionsTooLarge`] if `width * height`
    /// overflows `usize`.
    pub fn filled(width: usize, height: usize, value: T) -> Result<Self, RasterError> {
        let count = cell_count(width, height)?;
        Ok(Self { width, height, cells: vec![value; count] })
    }
}

impl<T: Copy> Grid<T> {
    /// A copy of cell `(x, y)`, or `None` if it is out of bounds.
    #[must_use]
    pub fn value(&self, x: usize, y: usize) -> Option<T> {
        self.get(x, y).copied()
    }
}

impl<T> Index<(usize, usize)> for Grid<T> {
    type Output = T;

    /// Indexes cell `(x, y)`.
    ///
    /// # Panics
    /// Panics if the coordinate is outside the grid. Use [`Grid::get`] to
    /// handle that case instead.
    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        self.get(x, y).expect("grid index out of bounds")
    }
}

impl<T> IndexMut<(usize, usize)> for Grid<T> {
    /// Mutably indexes cell `(x, y)`.
    ///
    /// # Panics
    /// Panics if the coordinate is outside the grid. Use [`Grid::get_mut`] to
    /// handle that case instead.
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        self.get_mut(x, y).expect("grid index out of bounds")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Grid<i16> {
        // 3 wide, 2 tall:
        //   0 1 2
        //   3 4 5
        Grid::new(3, 2, vec![0, 1, 2, 3, 4, 5]).unwrap()
    }

    #[test]
    fn new_rejects_a_mismatched_cell_count() {
        let error = Grid::new(3, 2, vec![0u8; 5]).unwrap_err();
        assert_eq!(
            error,
            RasterError::CellCountMismatch { width: 3, height: 2, cells: 5 }
        );
    }

    #[test]
    fn new_rejects_dimensions_that_overflow() {
        let error = Grid::new(usize::MAX, 2, Vec::<u8>::new()).unwrap_err();
        assert_eq!(
            error,
            RasterError::DimensionsTooLarge { width: usize::MAX, height: 2 }
        );
    }

    #[test]
    fn indexing_is_row_major() {
        let grid = ramp();
        assert_eq!(grid.width(), 3);
        assert_eq!(grid.height(), 2);
        assert_eq!(grid.len(), 6);
        assert_eq!(grid.index_of(2, 1), Some(5));
        assert_eq!(grid.value(2, 1), Some(5));
        assert_eq!(grid[(0, 1)], 3);
        assert_eq!(grid.get(3, 0), None);
        assert_eq!(grid.get(0, 2), None);
        assert_eq!(grid.index_of(3, 0), None);
    }

    #[test]
    fn rows_are_contiguous_slices() {
        let grid = ramp();
        assert_eq!(grid.row(0), Some(&[0i16, 1, 2][..]));
        assert_eq!(grid.row(1), Some(&[3i16, 4, 5][..]));
        assert_eq!(grid.row(2), None);
        let collected: Vec<&[i16]> = grid.rows().collect();
        assert_eq!(collected, vec![&[0i16, 1, 2][..], &[3i16, 4, 5][..]]);
    }

    #[test]
    fn set_writes_in_bounds_and_reports_out_of_bounds() {
        let mut grid = ramp();
        grid.set(1, 1, 42).unwrap();
        assert_eq!(grid.value(1, 1), Some(42));
        assert_eq!(
            grid.set(9, 0, 1).unwrap_err(),
            RasterError::OutOfBounds { x: 9, y: 0, width: 3, height: 2 }
        );
        grid[(2, 0)] = 7;
        assert_eq!(grid.value(2, 0), Some(7));
    }

    #[test]
    fn from_fn_and_filled_build_expected_contents() {
        let grid = Grid::from_fn(3, 2, |x, y| (x + 10 * y) as u8).unwrap();
        assert_eq!(grid.cells(), &[0, 1, 2, 10, 11, 12]);
        let filled = Grid::filled(2, 2, 4u8).unwrap();
        assert_eq!(filled.cells(), &[4, 4, 4, 4]);
    }

    #[test]
    fn map_preserves_shape() {
        let grid = ramp().map(|value| f64::from(*value) * 0.5);
        assert_eq!(grid.width(), 3);
        assert_eq!(grid.height(), 2);
        assert_eq!(grid.cells(), &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5]);
    }

    #[test]
    fn zero_sized_grids_are_allowed_and_iterate_empty() {
        let grid = Grid::<u8>::filled(0, 5, 0).unwrap();
        assert!(grid.is_empty());
        assert_eq!(grid.rows().count(), 0);
    }
}
