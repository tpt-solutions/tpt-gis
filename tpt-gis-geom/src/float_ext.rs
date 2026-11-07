//! `no_std` float helper. `f64::abs` is an inherent method of `std` (and of `core` only from
//! Rust 1.85), so on older compilers it is missing without `std`. This trait provides the same
//! operation through `libm`, which is available everywhere.

/// Absolute value of a float, computed with `libm`.
pub(crate) trait Fabs {
    /// `|self|`.
    fn fabs(self) -> f64;
}

impl Fabs for f64 {
    #[inline]
    fn fabs(self) -> f64 {
        libm::fabs(self)
    }
}
