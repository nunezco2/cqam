//! Complex number newtype for 64-bit complex arithmetic.
//!
//! Provides the `C64` struct with operator overloading for natural
//! arithmetic syntax (`a * b + c` instead of `cx_add(cx_mul(a, b), c)`).
//! Using a tuple struct preserves `.0` / `.1` field access for minimal
//! migration churn, without requiring an external complex-number crate.

use std::fmt;
use std::ops::{Add, Sub, Mul, Neg, AddAssign, SubAssign, MulAssign};

/// 64-bit complex number: re + im*i.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C64(pub f64, pub f64);

// =============================================================================
// Associated Constants
// =============================================================================

impl C64 {
    /// Complex zero: 0 + 0i.
    pub const ZERO: C64 = C64(0.0, 0.0);

    /// Complex one: 1 + 0i.
    pub const ONE: C64 = C64(1.0, 0.0);

    /// Imaginary unit: 0 + 1i.
    pub const I: C64 = C64(0.0, 1.0);
}

// =============================================================================
// Methods
// =============================================================================

impl C64 {
    /// Construct from real and imaginary parts.
    #[inline]
    pub const fn new(re: f64, im: f64) -> Self { C64(re, im) }

    /// Real part.
    #[inline]
    pub const fn re(self) -> f64 { self.0 }

    /// Imaginary part.
    #[inline]
    pub const fn im(self) -> f64 { self.1 }

    /// Complex conjugate: conj(a + bi) = a - bi.
    #[inline]
    pub fn conj(self) -> C64 { C64(self.0, -self.1) }

    /// Squared norm: |z|^2 = a^2 + b^2.
    #[inline]
    pub fn norm_sq(self) -> f64 { self.0 * self.0 + self.1 * self.1 }

    /// Norm (modulus): |z| = sqrt(a^2 + b^2).
    #[inline]
    pub fn norm(self) -> f64 { self.norm_sq().sqrt() }

    /// Scalar multiplication: r * (a + bi).
    #[inline]
    pub fn scale(self, r: f64) -> C64 { C64(r * self.0, r * self.1) }

    /// exp(i * theta) = cos(theta) + i * sin(theta).
    #[inline]
    pub fn exp_i(theta: f64) -> C64 { C64(theta.cos(), theta.sin()) }

    /// Approximate equality within epsilon on both components.
    #[inline]
    pub fn approx_eq(self, other: C64, eps: f64) -> bool {
        (self.0 - other.0).abs() < eps && (self.1 - other.1).abs() < eps
    }
}

// =============================================================================
// Operator Trait Implementations
// =============================================================================

impl Add for C64 {
    type Output = C64;
    #[inline]
    fn add(self, rhs: C64) -> C64 { C64(self.0 + rhs.0, self.1 + rhs.1) }
}

impl Sub for C64 {
    type Output = C64;
    #[inline]
    fn sub(self, rhs: C64) -> C64 { C64(self.0 - rhs.0, self.1 - rhs.1) }
}

impl Mul for C64 {
    type Output = C64;
    #[inline]
    fn mul(self, rhs: C64) -> C64 {
        C64(self.0 * rhs.0 - self.1 * rhs.1,
            self.0 * rhs.1 + self.1 * rhs.0)
    }
}

impl Mul<f64> for C64 {
    type Output = C64;
    #[inline]
    fn mul(self, rhs: f64) -> C64 { C64(self.0 * rhs, self.1 * rhs) }
}

impl Mul<C64> for f64 {
    type Output = C64;
    #[inline]
    fn mul(self, rhs: C64) -> C64 { C64(self * rhs.0, self * rhs.1) }
}

impl Neg for C64 {
    type Output = C64;
    #[inline]
    fn neg(self) -> C64 { C64(-self.0, -self.1) }
}

impl AddAssign for C64 {
    #[inline]
    fn add_assign(&mut self, rhs: C64) { self.0 += rhs.0; self.1 += rhs.1; }
}

impl SubAssign for C64 {
    #[inline]
    fn sub_assign(&mut self, rhs: C64) { self.0 -= rhs.0; self.1 -= rhs.1; }
}

impl MulAssign for C64 {
    #[inline]
    fn mul_assign(&mut self, rhs: C64) {
        let re = self.0 * rhs.0 - self.1 * rhs.1;
        let im = self.0 * rhs.1 + self.1 * rhs.0;
        self.0 = re;
        self.1 = im;
    }
}

impl MulAssign<f64> for C64 {
    #[inline]
    fn mul_assign(&mut self, rhs: f64) { self.0 *= rhs; self.1 *= rhs; }
}

impl fmt::Display for C64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 >= 0.0 {
            write!(f, "{}+{}i", self.0, self.1)
        } else {
            write!(f, "{}{}i", self.0, self.1)
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let a = C64(1.0, 2.0);
        let b = C64(3.0, 4.0);
        assert_eq!(a + b, C64(4.0, 6.0));
    }

    #[test]
    fn test_sub() {
        let a = C64(5.0, 3.0);
        let b = C64(2.0, 1.0);
        assert_eq!(a - b, C64(3.0, 2.0));
    }

    #[test]
    fn test_mul() {
        let a = C64(1.0, 2.0);
        let b = C64(3.0, 4.0);
        // (1+2i)(3+4i) = 3+4i+6i+8i^2 = -5+10i
        assert_eq!(a * b, C64(-5.0, 10.0));
    }

    #[test]
    fn test_mul_scalar() {
        let z = C64(1.0, 2.0);
        assert_eq!(z * 3.0, C64(3.0, 6.0));
        assert_eq!(3.0 * z, C64(3.0, 6.0));
    }

    #[test]
    fn test_conj() {
        assert_eq!(C64(1.0, 2.0).conj(), C64(1.0, -2.0));
    }

    #[test]
    fn test_norm_sq() {
        let z = C64(3.0, 4.0);
        assert!((z.norm_sq() - 25.0).abs() < 1e-12);
    }

    #[test]
    fn test_norm() {
        let z = C64(3.0, 4.0);
        assert!((z.norm() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_scale() {
        let z = C64(1.0, 2.0);
        assert_eq!(z.scale(3.0), C64(3.0, 6.0));
    }

    #[test]
    fn test_exp_i() {
        let z = C64::exp_i(0.0);
        assert!(z.approx_eq(C64::ONE, 1e-12));
        let z = C64::exp_i(std::f64::consts::FRAC_PI_2);
        assert!(z.approx_eq(C64::I, 1e-12));
    }

    #[test]
    fn test_neg() {
        assert_eq!(-C64(1.0, -2.0), C64(-1.0, 2.0));
    }

    #[test]
    fn test_add_assign() {
        let mut a = C64(1.0, 2.0);
        a += C64(3.0, 4.0);
        assert_eq!(a, C64(4.0, 6.0));
    }

    #[test]
    fn test_constants() {
        assert_eq!(C64::ZERO, C64(0.0, 0.0));
        assert_eq!(C64::ONE, C64(1.0, 0.0));
        assert_eq!(C64::I, C64(0.0, 1.0));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", C64(1.0, 2.0)), "1+2i");
        assert_eq!(format!("{}", C64(1.0, -2.0)), "1-2i");
    }
}
