//! [`RasterError`]: the failure mode shared by grid construction, map algebra,
//! and resampling.
//!
//! The file readers have their own error types
//! ([`GeoTiffError`](crate::geotiff::GeoTiffError),
//! [`CogError`](crate::cog::CogError)) because their failure modes are entirely
//! about malformed input rather than about a caller passing mismatched grids.

use core::fmt;

/// An error produced by an in-memory grid, band, map algebra, or resampling
/// operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RasterError {
    /// `width * height` overflowed `usize`, so a grid of that size can never be
    /// allocated.
    DimensionsTooLarge {
        /// Requested width in cells.
        width: usize,
        /// Requested height in cells.
        height: usize,
    },
    /// The supplied cell buffer's length is not exactly `width * height`.
    CellCountMismatch {
        /// Declared width in cells.
        width: usize,
        /// Declared height in cells.
        height: usize,
        /// Length of the supplied cell buffer.
        cells: usize,
    },
    /// A cell coordinate fell outside the grid.
    OutOfBounds {
        /// Requested column.
        x: usize,
        /// Requested row.
        y: usize,
        /// Grid width in cells.
        width: usize,
        /// Grid height in cells.
        height: usize,
    },
    /// A binary operation was given two grids of different sizes. Map algebra
    /// is cell-wise and has no resampling built in, so the operands must line
    /// up exactly — resample one of them first.
    ShapeMismatch {
        /// Size of the left-hand operand, as `(width, height)`.
        lhs: (usize, usize),
        /// Size of the right-hand operand, as `(width, height)`.
        rhs: (usize, usize),
    },
    /// An operation was asked to produce a grid with a zero width or height.
    EmptyOutput {
        /// Requested output width in cells.
        width: usize,
        /// Requested output height in cells.
        height: usize,
    },
    /// An operation that reads from its input (resampling) was given an empty
    /// grid, which has no cells to read.
    EmptyInput,
    /// A pixel size was zero, negative, or not finite.
    InvalidPixelSize(f64),
    /// A value range parameter had its lower bound above its upper bound (for
    /// example [`algebra::clamp`](crate::algebra::clamp) called with
    /// `low > high`), which has no sensible interpretation.
    InvalidRange,
    /// A convolution kernel had an even width or height, so it has no centre
    /// cell to align with the output cell.
    EvenKernel {
        /// Kernel width in cells.
        width: usize,
        /// Kernel height in cells.
        height: usize,
    },
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RasterError::DimensionsTooLarge { width, height } => {
                write!(f, "grid dimensions {width}x{height} overflow usize")
            }
            RasterError::CellCountMismatch { width, height, cells } => write!(
                f,
                "grid of {width}x{height} needs {} cells, got {cells}",
                width.saturating_mul(*height)
            ),
            RasterError::OutOfBounds { x, y, width, height } => {
                write!(f, "cell ({x}, {y}) is outside a {width}x{height} grid")
            }
            RasterError::ShapeMismatch { lhs, rhs } => {
                write!(f, "grid size mismatch: {}x{} vs {}x{}", lhs.0, lhs.1, rhs.0, rhs.1)
            }
            RasterError::EmptyOutput { width, height } => {
                write!(f, "requested output grid {width}x{height} has no cells")
            }
            RasterError::EmptyInput => write!(f, "input grid has no cells"),
            RasterError::InvalidPixelSize(size) => {
                write!(f, "pixel size {size} is not a positive, finite number")
            }
            RasterError::InvalidRange => {
                write!(f, "range lower bound is greater than its upper bound")
            }
            RasterError::EvenKernel { width, height } => {
                write!(
                    f,
                    "convolution kernel {width}x{height} has no centre cell (needs odd sides)"
                )
            }
        }
    }
}

impl core::error::Error for RasterError {}
