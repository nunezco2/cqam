//! Complex arithmetic helpers for `(f64, f64)` tuples.
//!
//! Provides the `C64` type alias and free arithmetic functions. Using
//! `(f64, f64)` tuples directly avoids a newtype wrapper and keeps
//! compatibility with `ComplexRegFile`, without requiring an external
//! complex-number crate.

/// Type alias for complex numbers represented as (real, imaginary).
pub type C64 = (f64, f64);

/// Complex zero: 0 + 0i.
pub const ZERO: C64 = (0.0, 0.0);

/// Complex one: 1 + 0i.
pub const ONE: C64 = (1.0, 0.0);

/// Imaginary unit: 0 + 1i.
pub const I: C64 = (0.0, 1.0);

/// Complex addition: (a+bi) + (c+di) = (a+c) + (b+d)i.
#[inline]
pub fn cx_add(a: C64, b: C64) -> C64 {
    (a.0 + b.0, a.1 + b.1)
}

/// Complex subtraction: (a+bi) - (c+di) = (a-c) + (b-d)i.
#[inline]
pub fn cx_sub(a: C64, b: C64) -> C64 {
    (a.0 - b.0, a.1 - b.1)
}

/// Complex multiplication: (a+bi)(c+di) = (ac-bd) + (ad+bc)i.
#[inline]
pub fn cx_mul(a: C64, b: C64) -> C64 {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

/// Scalar-complex multiplication: r * (a+bi) = (r*a) + (r*b)i.
#[inline]
pub fn cx_scale(r: f64, z: C64) -> C64 {
    (r * z.0, r * z.1)
}

/// Complex conjugate: conj(a+bi) = a - bi.
#[inline]
pub fn cx_conj(z: C64) -> C64 {
    (z.0, -z.1)
}

/// Squared norm (modulus squared): |a+bi|^2 = a^2 + b^2.
///
/// Preferred over `cx_norm` when the square root is unnecessary,
/// since it avoids the sqrt() cost.
#[inline]
pub fn cx_norm_sq(z: C64) -> f64 {
    z.0 * z.0 + z.1 * z.1
}

/// Norm (modulus): |a+bi| = sqrt(a^2 + b^2).
#[inline]
pub fn cx_norm(z: C64) -> f64 {
    cx_norm_sq(z).sqrt()
}

/// Complex exponential of a pure imaginary argument:
/// exp(i * theta) = cos(theta) + i * sin(theta).
///
/// Used by the QFT kernel to construct rotation entries.
#[inline]
pub fn cx_exp_i(theta: f64) -> C64 {
    (theta.cos(), theta.sin())
}

/// Approximate equality test for two complex numbers.
///
/// Returns true if both the real and imaginary parts differ by less than `eps`.
#[inline]
pub fn cx_approx_eq(a: C64, b: C64, eps: f64) -> bool {
    (a.0 - b.0).abs() < eps && (a.1 - b.1).abs() < eps
}
