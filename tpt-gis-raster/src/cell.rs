//! [`CellType`]: the numeric types a raster cell can hold.
//!
//! Map algebra, resampling, and GeoTIFF decoding are all generic over this
//! trait so that an operation is written once and works for every sample type a
//! real raster uses. The trait is deliberately closed in practice (it is
//! implemented for `u8`, `u16`, `i16`, `u32`, `i32`, `f32`, and `f64`, the
//! sample types GDAL and the TIFF `SampleFormat` tag between them describe),
//! but nothing stops a downstream crate implementing it for another numeric
//! type.
//!
//! Two details are worth calling out:
//!
//! * [`CellType::same_value`] rather than `PartialEq`, because a nodata value of
//!   `NaN` is common in floating-point rasters and `NaN != NaN` would make every
//!   such cell look like valid data.
//! * [`CellType::saturating_add`] and friends, because wrapping an `u8` DEM cell
//!   from 255 to 0 turns a mountain into a valley. Integer arithmetic clamps;
//!   floating-point arithmetic keeps IEEE semantics (including infinities).

use core::fmt::Debug;

/// The concrete sample type of a [`Grid`](crate::Grid), as a runtime value.
///
/// Used by [`AnyBand`](crate::AnyBand) to report what a file reader actually
/// decoded without the caller having to match on every enum variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellKind {
    /// 8-bit unsigned integer.
    U8,
    /// 16-bit unsigned integer.
    U16,
    /// 16-bit signed integer.
    I16,
    /// 32-bit unsigned integer.
    U32,
    /// 32-bit signed integer.
    I32,
    /// 32-bit IEEE 754 floating point.
    F32,
    /// 64-bit IEEE 754 floating point.
    F64,
}

impl CellKind {
    /// Width of one cell of this kind, in bytes.
    #[must_use]
    pub const fn size_bytes(self) -> usize {
        match self {
            CellKind::U8 => 1,
            CellKind::U16 | CellKind::I16 => 2,
            CellKind::U32 | CellKind::I32 | CellKind::F32 => 4,
            CellKind::F64 => 8,
        }
    }

    /// The Rust type name of this kind, e.g. `"i16"`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            CellKind::U8 => "u8",
            CellKind::U16 => "u16",
            CellKind::I16 => "i16",
            CellKind::U32 => "u32",
            CellKind::I32 => "i32",
            CellKind::F32 => "f32",
            CellKind::F64 => "f64",
        }
    }

    /// Whether this kind is a floating-point type.
    #[must_use]
    pub const fn is_float(self) -> bool {
        matches!(self, CellKind::F32 | CellKind::F64)
    }
}

/// A numeric type that can be stored in a [`Grid`](crate::Grid) cell.
pub trait CellType: Copy + Debug + PartialOrd + Sized + 'static {
    /// The additive identity, used as the fill value when a grid is allocated
    /// before being written into and as the fallback when no nodata value is
    /// declared.
    const ZERO: Self;
    /// The smallest finite value of this type.
    const MIN: Self;
    /// The largest finite value of this type.
    const MAX: Self;
    /// Width of one cell in an external byte buffer (a TIFF strip or tile).
    const SIZE_BYTES: usize;
    /// The runtime tag for this type.
    const KIND: CellKind;

    /// Widens the cell to `f64`, the accumulator type used by focal statistics
    /// and bilinear interpolation.
    ///
    /// `u32`/`i32` values above 2^53 lose precision, as they would in any
    /// f64-based statistic.
    fn to_f64(self) -> f64;

    /// Narrows an `f64` back to the cell type, rounding to nearest for integers
    /// and saturating at [`MIN`](CellType::MIN)/[`MAX`](CellType::MAX). `NaN`
    /// becomes [`ZERO`](CellType::ZERO) for integer types.
    fn from_f64(value: f64) -> Self;

    /// Addition that clamps at the type's bounds instead of wrapping.
    fn saturating_add(self, rhs: Self) -> Self;

    /// Subtraction that clamps at the type's bounds instead of wrapping.
    fn saturating_sub(self, rhs: Self) -> Self;

    /// Multiplication that clamps at the type's bounds instead of wrapping.
    fn saturating_mul(self, rhs: Self) -> Self;

    /// Equality for nodata comparison: like `==`, except that two `NaN`s are
    /// considered the same value so a `NaN` nodata marker can be matched.
    fn same_value(self, other: Self) -> bool;

    /// Reads one cell from the start of `bytes`, little-endian. Returns `None`
    /// if `bytes` is shorter than [`SIZE_BYTES`](CellType::SIZE_BYTES).
    fn from_le_bytes_slice(bytes: &[u8]) -> Option<Self>;

    /// Reads one cell from the start of `bytes`, big-endian. Returns `None` if
    /// `bytes` is shorter than [`SIZE_BYTES`](CellType::SIZE_BYTES).
    fn from_be_bytes_slice(bytes: &[u8]) -> Option<Self>;
}

/// Implements [`CellType`] for a primitive integer type.
macro_rules! impl_integer_cell_type {
    ($ty:ty, $kind:ident) => {
        impl CellType for $ty {
            const ZERO: Self = 0;
            const MIN: Self = <$ty>::MIN;
            const MAX: Self = <$ty>::MAX;
            const SIZE_BYTES: usize = core::mem::size_of::<$ty>();
            const KIND: CellKind = CellKind::$kind;

            fn to_f64(self) -> f64 {
                f64::from(self)
            }

            fn from_f64(value: f64) -> Self {
                // Rust's float-to-int `as` cast saturates and maps NaN to 0,
                // which is exactly the contract above.
                libm::round(value) as $ty
            }

            fn saturating_add(self, rhs: Self) -> Self {
                <$ty>::saturating_add(self, rhs)
            }

            fn saturating_sub(self, rhs: Self) -> Self {
                <$ty>::saturating_sub(self, rhs)
            }

            fn saturating_mul(self, rhs: Self) -> Self {
                <$ty>::saturating_mul(self, rhs)
            }

            fn same_value(self, other: Self) -> bool {
                self == other
            }

            fn from_le_bytes_slice(bytes: &[u8]) -> Option<Self> {
                let array: [u8; core::mem::size_of::<$ty>()] =
                    bytes.get(..core::mem::size_of::<$ty>())?.try_into().ok()?;
                Some(<$ty>::from_le_bytes(array))
            }

            fn from_be_bytes_slice(bytes: &[u8]) -> Option<Self> {
                let array: [u8; core::mem::size_of::<$ty>()] =
                    bytes.get(..core::mem::size_of::<$ty>())?.try_into().ok()?;
                Some(<$ty>::from_be_bytes(array))
            }
        }
    };
}

/// Implements [`CellType`] for a primitive floating-point type.
macro_rules! impl_float_cell_type {
    ($ty:ty, $kind:ident) => {
        impl CellType for $ty {
            const ZERO: Self = 0.0;
            const MIN: Self = <$ty>::MIN;
            const MAX: Self = <$ty>::MAX;
            const SIZE_BYTES: usize = core::mem::size_of::<$ty>();
            const KIND: CellKind = CellKind::$kind;

            fn to_f64(self) -> f64 {
                f64::from(self)
            }

            fn from_f64(value: f64) -> Self {
                value as $ty
            }

            fn saturating_add(self, rhs: Self) -> Self {
                self + rhs
            }

            fn saturating_sub(self, rhs: Self) -> Self {
                self - rhs
            }

            fn saturating_mul(self, rhs: Self) -> Self {
                self * rhs
            }

            fn same_value(self, other: Self) -> bool {
                self == other || (self.is_nan() && other.is_nan())
            }

            fn from_le_bytes_slice(bytes: &[u8]) -> Option<Self> {
                let array: [u8; core::mem::size_of::<$ty>()] =
                    bytes.get(..core::mem::size_of::<$ty>())?.try_into().ok()?;
                Some(<$ty>::from_le_bytes(array))
            }

            fn from_be_bytes_slice(bytes: &[u8]) -> Option<Self> {
                let array: [u8; core::mem::size_of::<$ty>()] =
                    bytes.get(..core::mem::size_of::<$ty>())?.try_into().ok()?;
                Some(<$ty>::from_be_bytes(array))
            }
        }
    };
}

impl_integer_cell_type!(u8, U8);
impl_integer_cell_type!(u16, U16);
impl_integer_cell_type!(i16, I16);
impl_integer_cell_type!(u32, U32);
impl_integer_cell_type!(i32, I32);
impl_float_cell_type!(f32, F32);
impl_float_cell_type!(f64, F64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_arithmetic_saturates_instead_of_wrapping() {
        assert_eq!(CellType::saturating_add(250u8, 10u8), 255);
        assert_eq!(CellType::saturating_sub(5u8, 10u8), 0);
        assert_eq!(CellType::saturating_sub(i16::MIN, 1), i16::MIN);
        assert_eq!(CellType::saturating_mul(30000i16, 2), i16::MAX);
    }

    #[test]
    fn from_f64_rounds_to_nearest_and_saturates() {
        assert_eq!(<u8 as CellType>::from_f64(2.5), 3);
        assert_eq!(<u8 as CellType>::from_f64(2.4), 2);
        assert_eq!(<u8 as CellType>::from_f64(-3.0), 0);
        assert_eq!(<u8 as CellType>::from_f64(1e9), 255);
        assert_eq!(<i16 as CellType>::from_f64(f64::NAN), 0);
        assert_eq!(<f32 as CellType>::from_f64(2.5), 2.5f32);
    }

    #[test]
    fn nan_matches_itself_so_it_can_be_a_nodata_marker() {
        assert!(CellType::same_value(f64::NAN, f64::NAN));
        assert!(CellType::same_value(f32::NAN, f32::NAN));
        assert!(!CellType::same_value(f64::NAN, 0.0));
        assert!(CellType::same_value(-9999.0f32, -9999.0f32));
    }

    #[test]
    fn reads_cells_from_both_byte_orders() {
        assert_eq!(<u16 as CellType>::from_le_bytes_slice(&[0x01, 0x02]), Some(0x0201));
        assert_eq!(<u16 as CellType>::from_be_bytes_slice(&[0x01, 0x02]), Some(0x0102));
        assert_eq!(<u32 as CellType>::from_le_bytes_slice(&[0x01, 0x02]), None);
        assert_eq!(<f32 as CellType>::from_le_bytes_slice(&1.5f32.to_le_bytes()), Some(1.5));
    }

    #[test]
    fn kind_reports_size_and_name() {
        assert_eq!(<i16 as CellType>::KIND, CellKind::I16);
        assert_eq!(CellKind::I16.size_bytes(), 2);
        assert_eq!(CellKind::F64.name(), "f64");
        assert!(CellKind::F32.is_float());
        assert!(!CellKind::U32.is_float());
    }
}
