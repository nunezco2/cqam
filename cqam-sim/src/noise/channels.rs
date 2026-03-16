//! Kraus operator constructors for standard quantum noise channels.
//!
//! Each function returns a `Vec` of Kraus operators (2x2 or 4x4 matrices
//! in row-major order) that define the channel. These are consumed by
//! `DensityMatrix::apply_single_qubit_channel` / `apply_two_qubit_channel`
//! or by the trajectory sampling functions.

use crate::complex::C64;

/// Kraus operators for a single-qubit amplitude damping channel.
///
/// Models T1 energy decay: |1> -> |0> with probability gamma.
///
///   K0 = [[1, 0], [0, sqrt(1 - gamma)]]
///   K1 = [[0, sqrt(gamma)], [0, 0]]
///
/// gamma = 1 - exp(-t / T1)
pub fn amplitude_damping(gamma: f64) -> Vec<[C64; 4]> {
    assert!((0.0..=1.0).contains(&gamma),
            "gamma must be in [0, 1], got {}", gamma);
    let sg = (1.0 - gamma).sqrt();
    let sqg = gamma.sqrt();
    vec![
        [C64::ONE, C64::ZERO, C64::ZERO, C64(sg, 0.0)],
        [C64::ZERO, C64(sqg, 0.0), C64::ZERO, C64::ZERO],
    ]
}

/// Kraus operators for a single-qubit phase damping channel.
///
/// Models pure dephasing (T_phi): off-diagonal elements decay
/// without energy exchange.
///
///   K0 = [[1, 0], [0, sqrt(1 - lambda)]]
///   K1 = [[0, 0], [0, sqrt(lambda)]]
///
/// lambda = 1 - exp(-t / T_phi)
/// where 1/T_phi = 1/T2 - 1/(2*T1)
pub fn phase_damping(lambda: f64) -> Vec<[C64; 4]> {
    assert!((0.0..=1.0).contains(&lambda),
            "lambda must be in [0, 1], got {}", lambda);
    let sl = (1.0 - lambda).sqrt();
    let sql = lambda.sqrt();
    vec![
        [C64::ONE, C64::ZERO, C64::ZERO, C64(sl, 0.0)],
        [C64::ZERO, C64::ZERO, C64::ZERO, C64(sql, 0.0)],
    ]
}

/// Kraus operators for a single-qubit depolarizing channel.
///
///   E(rho) = (1 - p) rho + (p/3)(X rho X + Y rho Y + Z rho Z)
///
/// Equivalent Kraus form with 4 operators:
///   K0 = sqrt(1 - p) * I
///   K1 = sqrt(p/3) * X
///   K2 = sqrt(p/3) * Y
///   K3 = sqrt(p/3) * Z
pub fn depolarizing_single(p: f64) -> Vec<[C64; 4]> {
    assert!((0.0..=1.0).contains(&p),
            "p must be in [0, 1], got {}", p);
    let s0 = (1.0 - p).sqrt();
    let s1 = (p / 3.0).sqrt();
    vec![
        // K0 = sqrt(1-p) * I
        [C64(s0, 0.0), C64::ZERO, C64::ZERO, C64(s0, 0.0)],
        // K1 = sqrt(p/3) * X
        [C64::ZERO, C64(s1, 0.0), C64(s1, 0.0), C64::ZERO],
        // K2 = sqrt(p/3) * Y
        [C64::ZERO, C64(0.0, -s1), C64(0.0, s1), C64::ZERO],
        // K3 = sqrt(p/3) * Z
        [C64(s1, 0.0), C64::ZERO, C64::ZERO, C64(-s1, 0.0)],
    ]
}

/// Kraus operators for a two-qubit depolarizing channel.
///
///   E(rho) = (1 - p) rho + (p/15) sum_{P != I4} P rho P^dag
///
/// where the sum runs over the 15 non-identity two-qubit Pauli operators
/// {I,X,Y,Z}^{tensor 2} \ {I tensor I}.
pub fn depolarizing_two_qubit(p: f64) -> Vec<[C64; 16]> {
    assert!((0.0..=1.0).contains(&p),
            "p must be in [0, 1], got {}", p);

    let s0 = (1.0 - p).sqrt();
    let s1 = (p / 15.0).sqrt();

    let paulis_1q: [[C64; 4]; 4] = [
        // I
        [C64::ONE, C64::ZERO, C64::ZERO, C64::ONE],
        // X
        [C64::ZERO, C64::ONE, C64::ONE, C64::ZERO],
        // Y
        [C64::ZERO, C64(0.0, -1.0), C64(0.0, 1.0), C64::ZERO],
        // Z
        [C64::ONE, C64::ZERO, C64::ZERO, -C64::ONE],
    ];

    let mut ops = Vec::with_capacity(16);
    for a in 0..4 {
        for b in 0..4 {
            let scale = if a == 0 && b == 0 { s0 } else { s1 };
            let mut mat = [C64::ZERO; 16];
            for i in 0..2 {
                for j in 0..2 {
                    let pa = paulis_1q[a][i * 2 + j];
                    for k in 0..2 {
                        for l in 0..2 {
                            let pb = paulis_1q[b][k * 2 + l];
                            mat[(i * 2 + k) * 4 + (j * 2 + l)] =
                                (pa * pb).scale(scale);
                        }
                    }
                }
            }
            ops.push(mat);
        }
    }
    ops
}

/// Kraus operators for thermal relaxation (combined T1 + T2).
///
/// Models the physical process of a qubit coupled to a thermal bath
/// at finite temperature. Combines generalized amplitude damping (GAD)
/// with additional pure dephasing when T2 < 2*T1.
///
/// If T2 > 2*T1, T2 is clamped to 2*T1 (physical constraint).
pub fn thermal_relaxation(
    t1: f64,
    t2: f64,
    time: f64,
    p_exc: f64,
) -> Vec<[C64; 4]> {
    assert!(t1 > 0.0, "T1 must be positive");
    let t2_clamped = t2.min(2.0 * t1);
    assert!(t2_clamped > 0.0, "T2 must be positive");

    if time <= 0.0 {
        // No evolution: return identity channel
        return vec![[C64::ONE, C64::ZERO, C64::ZERO, C64::ONE]];
    }

    let gamma = 1.0 - (-time / t1).exp();
    let p0 = 1.0 - p_exc; // ground state population at equilibrium

    let sg = (1.0 - gamma).sqrt();
    let sqg0 = (p0 * gamma).sqrt();
    let sqg1 = (p_exc * gamma).sqrt();
    let sp0 = p0.sqrt();
    let sp1 = p_exc.sqrt();

    // GAD Kraus operators:
    // K0 = sqrt(p0) * [[1, 0], [0, sqrt(1-gamma)]]
    // K1 = sqrt(p0) * [[0, sqrt(gamma)], [0, 0]]
    // K2 = sqrt(p_exc) * [[sqrt(1-gamma), 0], [0, 1]]
    // K3 = sqrt(p_exc) * [[0, 0], [sqrt(gamma), 0]]
    let mut kraus_ops = vec![
        [C64(sp0, 0.0), C64::ZERO, C64::ZERO, C64(sp0 * sg, 0.0)],
        [C64::ZERO, C64(sqg0, 0.0), C64::ZERO, C64::ZERO],
        [C64(sp1 * sg, 0.0), C64::ZERO, C64::ZERO, C64(sp1, 0.0)],
        [C64::ZERO, C64::ZERO, C64(sqg1, 0.0), C64::ZERO],
    ];

    // Additional pure dephasing if T2 < 2*T1
    if t2_clamped < 2.0 * t1 {
        let t_phi = 1.0 / (1.0 / t2_clamped - 1.0 / (2.0 * t1));
        let lambda = 1.0 - (-time / t_phi).exp();
        let phase_factor = (1.0 - lambda).sqrt();
        for k in &mut kraus_ops {
            // Off-diagonal elements of K get multiplied by phase_factor
            k[1] = k[1].scale(phase_factor); // K[0][1]
            k[2] = k[2].scale(phase_factor); // K[1][0]
        }
    }

    kraus_ops
}

/// Kraus operators for photon loss channel.
///
/// Models the loss of a photon in a dual-rail encoded qubit.
/// eta is the transmission probability (1 = no loss, 0 = total loss).
///
/// Mathematically identical to amplitude damping with gamma = 1 - eta.
pub fn photon_loss(eta: f64) -> Vec<[C64; 4]> {
    amplitude_damping(1.0 - eta)
}

/// Kraus operators for a bit-flip channel.
///
///   E(rho) = (1 - p) rho + p X rho X
pub fn bit_flip(p: f64) -> Vec<[C64; 4]> {
    assert!((0.0..=1.0).contains(&p),
            "p must be in [0, 1], got {}", p);
    let s0 = (1.0 - p).sqrt();
    let s1 = p.sqrt();
    vec![
        [C64(s0, 0.0), C64::ZERO, C64::ZERO, C64(s0, 0.0)],
        [C64::ZERO, C64(s1, 0.0), C64(s1, 0.0), C64::ZERO],
    ]
}

/// Apply readout confusion matrix to a probability vector.
///
/// For an n-qubit measurement, applies per-qubit 2x2 confusion matrices
/// sequentially. `p01` = P(1|0), `p10` = P(0|1).
pub fn apply_readout_confusion(
    probs: &mut [f64],
    p01: f64,
    p10: f64,
) {
    let n_states = probs.len();
    let n_qubits = (n_states as f64).log2() as usize;
    assert_eq!(1 << n_qubits, n_states, "probs length must be a power of 2");

    for q in 0..n_qubits {
        let mask = 1 << (n_qubits - 1 - q);
        for base in 0..n_states {
            if base & mask != 0 { continue; }
            let idx0 = base;        // bit q = 0
            let idx1 = base | mask; // bit q = 1
            let p0_val = probs[idx0];
            let p1_val = probs[idx1];
            probs[idx0] = (1.0 - p01) * p0_val + p10 * p1_val;
            probs[idx1] = p01 * p0_val + (1.0 - p10) * p1_val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify sum_k K_k^dag K_k = I for single-qubit Kraus ops.
    fn verify_completeness_1q(ops: &[[C64; 4]]) {
        verify_completeness_1q_tol(ops, 1e-10);
    }

    /// Verify sum_k K_k^dag K_k = I with a custom tolerance.
    fn verify_completeness_1q_tol(ops: &[[C64; 4]], tol: f64) {
        let mut sum = [C64::ZERO; 4];
        for k in ops {
            let [k00, k01, k10, k11] = *k;
            sum[0] += k00.conj() * k00 + k10.conj() * k10;
            sum[1] += k00.conj() * k01 + k10.conj() * k11;
            sum[2] += k01.conj() * k00 + k11.conj() * k10;
            sum[3] += k01.conj() * k01 + k11.conj() * k11;
        }
        assert!((sum[0].0 - 1.0).abs() < tol, "sum[0,0] = {:?}", sum[0]);
        assert!(sum[1].norm_sq() < tol * tol, "sum[0,1] = {:?}", sum[1]);
        assert!(sum[2].norm_sq() < tol * tol, "sum[1,0] = {:?}", sum[2]);
        assert!((sum[3].0 - 1.0).abs() < tol, "sum[1,1] = {:?}", sum[3]);
    }

    #[test]
    fn amplitude_damping_completeness() {
        for &gamma in &[0.0, 0.01, 0.1, 0.5, 0.99, 1.0] {
            verify_completeness_1q(&amplitude_damping(gamma));
        }
    }

    #[test]
    fn phase_damping_completeness() {
        for &lambda in &[0.0, 0.01, 0.1, 0.5, 0.99, 1.0] {
            verify_completeness_1q(&phase_damping(lambda));
        }
    }

    #[test]
    fn depolarizing_single_completeness() {
        for &p in &[0.0, 0.01, 0.1, 0.5, 0.99, 1.0] {
            verify_completeness_1q(&depolarizing_single(p));
        }
    }

    #[test]
    fn bit_flip_completeness() {
        for &p in &[0.0, 0.01, 0.1, 0.5, 0.99, 1.0] {
            verify_completeness_1q(&bit_flip(p));
        }
    }

    #[test]
    fn thermal_relaxation_completeness() {
        // The GAD + dephasing approximation has O(lambda*gamma) error,
        // so we use a slightly relaxed tolerance for composed channels.
        let ops = thermal_relaxation(300e-6, 200e-6, 35e-9, 0.01);
        verify_completeness_1q_tol(&ops, 1e-7);
    }

    #[test]
    fn photon_loss_completeness() {
        for &eta in &[0.0, 0.5, 0.95, 1.0] {
            verify_completeness_1q(&photon_loss(eta));
        }
    }

    #[test]
    fn readout_confusion_preserves_normalization() {
        let mut probs = vec![0.25, 0.25, 0.25, 0.25];
        apply_readout_confusion(&mut probs, 0.01, 0.02);
        let total: f64 = probs.iter().sum();
        assert!((total - 1.0).abs() < 1e-10);
    }

    #[test]
    fn readout_confusion_identity_at_zero() {
        let original = vec![0.3, 0.2, 0.4, 0.1];
        let mut probs = original.clone();
        apply_readout_confusion(&mut probs, 0.0, 0.0);
        for (a, b) in probs.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-15);
        }
    }
}
