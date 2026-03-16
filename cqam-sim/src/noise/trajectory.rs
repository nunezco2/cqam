//! Stochastic Kraus sampling on statevectors (quantum trajectory method).
//!
//! These functions apply a Kraus channel to a statevector by sampling
//! one operator based on outcome probabilities, then renormalizing.
//! This produces statistically correct results over many shots while
//! using O(2^n) memory per trajectory instead of O(4^n) for density matrices.

use crate::complex::C64;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Apply a single-qubit Kraus channel stochastically to a statevector.
///
/// Given Kraus operators {K_0, K_1, ..., K_m}, this function:
/// 1. Computes p_k = <psi| K_k^dag K_k |psi> for each k.
/// 2. Samples one operator k* according to these probabilities.
/// 3. Applies K_{k*} to the statevector on the target qubit.
/// 4. Renormalizes the statevector.
///
/// Returns `true` if k* > 0 (a quantum jump occurred).
pub fn apply_kraus_to_statevector_single(
    amplitudes: &mut [C64],
    num_qubits: u8,
    target: u8,
    kraus_ops: &[[C64; 4]],
    rng: &mut ChaCha8Rng,
) -> bool {
    let n = num_qubits as usize;
    let dim = 1usize << n;
    let bit = n - 1 - target as usize;
    let mask = 1usize << bit;

    // Step 1: Compute jump probabilities p_k = <psi| K_k^dag K_k |psi>
    let mut probs: Vec<f64> = Vec::with_capacity(kraus_ops.len());

    for kraus in kraus_ops {
        let [k00, k01, k10, k11] = *kraus;
        let mut prob = 0.0_f64;

        for base in 0..dim {
            if base & mask != 0 { continue; }
            let i0 = base;
            let i1 = base | mask;
            let a0 = amplitudes[i0];
            let a1 = amplitudes[i1];

            let b0 = k00 * a0 + k01 * a1;
            let b1 = k10 * a0 + k11 * a1;
            prob += b0.norm_sq() + b1.norm_sq();
        }
        probs.push(prob);
    }

    // Step 2: Sample one operator according to probabilities.
    let r: f64 = rng.r#gen();
    let mut cumulative = 0.0;
    let mut selected = probs.len() - 1;
    for (k, &p) in probs.iter().enumerate() {
        cumulative += p;
        if r < cumulative {
            selected = k;
            break;
        }
    }

    // Step 3: Apply the selected Kraus operator to the statevector.
    let [k00, k01, k10, k11] = kraus_ops[selected];
    for base in 0..dim {
        if base & mask != 0 { continue; }
        let i0 = base;
        let i1 = base | mask;
        let a0 = amplitudes[i0];
        let a1 = amplitudes[i1];
        amplitudes[i0] = k00 * a0 + k01 * a1;
        amplitudes[i1] = k10 * a0 + k11 * a1;
    }

    // Step 4: Renormalize.
    let norm_sq: f64 = amplitudes.iter().map(|a| a.norm_sq()).sum();
    if norm_sq > 0.0 {
        let inv_norm = 1.0 / norm_sq.sqrt();
        for a in amplitudes.iter_mut() {
            *a = a.scale(inv_norm);
        }
    }

    selected > 0
}

/// Apply a two-qubit Kraus channel stochastically to a statevector.
///
/// Each Kraus operator is a 4x4 matrix in row-major order as [C64; 16],
/// acting on the (qubit_a, qubit_b) subspace.
///
/// Returns `true` if a quantum jump occurred (selected operator k > 0).
pub fn apply_kraus_to_statevector_two(
    amplitudes: &mut [C64],
    num_qubits: u8,
    qubit_a: u8,
    qubit_b: u8,
    kraus_ops: &[[C64; 16]],
    rng: &mut ChaCha8Rng,
) -> bool {
    let n = num_qubits as usize;
    let dim = 1usize << n;
    let bit_a = n - 1 - qubit_a as usize;
    let bit_b = n - 1 - qubit_b as usize;
    let mask_a = 1usize << bit_a;
    let mask_b = 1usize << bit_b;

    let bases: Vec<usize> = (0..dim)
        .filter(|&b| b & (mask_a | mask_b) == 0)
        .collect();

    // Step 1: Compute jump probabilities.
    let mut probs: Vec<f64> = Vec::with_capacity(kraus_ops.len());

    for kraus in kraus_ops {
        let mut prob = 0.0_f64;
        for &base in &bases {
            let idxs = [
                base,
                base | mask_b,
                base | mask_a,
                base | mask_a | mask_b,
            ];
            let a: [C64; 4] = [
                amplitudes[idxs[0]],
                amplitudes[idxs[1]],
                amplitudes[idxs[2]],
                amplitudes[idxs[3]],
            ];
            for row in 0..4 {
                let mut b = C64::ZERO;
                for col in 0..4 {
                    b += kraus[row * 4 + col] * a[col];
                }
                prob += b.norm_sq();
            }
        }
        probs.push(prob);
    }

    // Step 2: Sample.
    let r: f64 = rng.r#gen();
    let mut cumulative = 0.0;
    let mut selected = probs.len() - 1;
    for (k, &p) in probs.iter().enumerate() {
        cumulative += p;
        if r < cumulative {
            selected = k;
            break;
        }
    }

    // Step 3: Apply selected operator.
    let kraus = &kraus_ops[selected];
    for &base in &bases {
        let idxs = [
            base,
            base | mask_b,
            base | mask_a,
            base | mask_a | mask_b,
        ];
        let a: [C64; 4] = [
            amplitudes[idxs[0]],
            amplitudes[idxs[1]],
            amplitudes[idxs[2]],
            amplitudes[idxs[3]],
        ];
        for row in 0..4 {
            let mut b = C64::ZERO;
            for col in 0..4 {
                b += kraus[row * 4 + col] * a[col];
            }
            amplitudes[idxs[row]] = b;
        }
    }

    // Step 4: Renormalize.
    let norm_sq: f64 = amplitudes.iter().map(|a| a.norm_sq()).sum();
    if norm_sq > 0.0 {
        let inv_norm = 1.0 / norm_sq.sqrt();
        for a in amplitudes.iter_mut() {
            *a = a.scale(inv_norm);
        }
    }

    selected > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn identity_channel_preserves_state() {
        let identity = [C64::ONE, C64::ZERO, C64::ZERO, C64::ONE];
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let mut amps = vec![C64(inv_sqrt2, 0.0), C64(inv_sqrt2, 0.0)];
        let original = amps.clone();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let jumped = apply_kraus_to_statevector_single(
            &mut amps, 1, 0, &[identity], &mut rng,
        );
        assert!(!jumped);
        for (a, b) in amps.iter().zip(original.iter()) {
            assert!((*a - *b).norm_sq() < 1e-20);
        }
    }

    #[test]
    fn normalization_preserved() {
        let gamma = 0.3_f64;
        let sg = (1.0 - gamma).sqrt();
        let sqg = gamma.sqrt();
        let k0 = [C64::ONE, C64::ZERO, C64::ZERO, C64(sg, 0.0)];
        let k1 = [C64::ZERO, C64(sqg, 0.0), C64::ZERO, C64::ZERO];

        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let mut amps = vec![C64(inv_sqrt2, 0.0), C64(inv_sqrt2, 0.0)];
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        apply_kraus_to_statevector_single(
            &mut amps, 1, 0, &[k0, k1], &mut rng,
        );

        let norm: f64 = amps.iter().map(|a| a.norm_sq()).sum();
        assert!((norm - 1.0).abs() < 1e-10, "norm = {}", norm);
    }

    #[test]
    fn reproducible_with_same_seed() {
        let gamma = 0.5_f64;
        let sg = (1.0 - gamma).sqrt();
        let sqg = gamma.sqrt();
        let k0 = [C64::ONE, C64::ZERO, C64::ZERO, C64(sg, 0.0)];
        let k1 = [C64::ZERO, C64(sqg, 0.0), C64::ZERO, C64::ZERO];

        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();

        let mut amps1 = vec![C64(inv_sqrt2, 0.0), C64(inv_sqrt2, 0.0)];
        let mut rng1 = ChaCha8Rng::seed_from_u64(123);
        apply_kraus_to_statevector_single(&mut amps1, 1, 0, &[k0, k1], &mut rng1);

        let mut amps2 = vec![C64(inv_sqrt2, 0.0), C64(inv_sqrt2, 0.0)];
        let mut rng2 = ChaCha8Rng::seed_from_u64(123);
        apply_kraus_to_statevector_single(&mut amps2, 1, 0, &[k0, k1], &mut rng2);

        for (a, b) in amps1.iter().zip(amps2.iter()) {
            assert!((*a - *b).norm_sq() < 1e-30);
        }
    }
}
