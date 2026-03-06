//! Controlled-U kernel (kernel_id = 8).
//!
//! Applies a sub-kernel conditioned on a control qubit within the register.
//! When the control qubit is |1⟩, the sub-kernel U is applied to the remaining
//! (n-1) qubits; when |0⟩, the state is unchanged.
//!
//! Supports power application: U^{2^k}. For ROTATE and PHASE_SHIFT kernels,
//! the power is folded into the parameter (theta * 2^k) in O(1). For other
//! kernels, the sub-kernel is applied 2^k times in a loop.

use cqam_core::error::CqamError;
use cqam_core::instruction::kernel_id;
use crate::complex::{self, C64, cx_norm_sq};
use crate::density_matrix::DensityMatrix;
use crate::statevector::Statevector;
use crate::kernel::Kernel;
use crate::kernels::init::Init;
use crate::kernels::entangle::Entangle;
use crate::kernels::fourier::Fourier;
use crate::kernels::diffuse::Diffuse;
use crate::kernels::grover::GroverIter;
use crate::kernels::rotate::Rotate;
use crate::kernels::phase::PhaseShift;
use crate::kernels::fourier_inv::FourierInv;

/// Controlled-U kernel parameterized by control qubit, sub-kernel, and power.
pub struct ControlledU {
    pub control_qubit: u8,
    pub sub_kernel_id: u8,
    pub power: u32,
    pub param_re: f64,
    pub param_im: f64,
}

impl ControlledU {
    /// Build the sub-kernel with power-folded parameters where possible.
    fn build_sub_kernel(&self) -> Box<dyn Kernel> {
        let scale = if self.power == 0 { 1.0 } else { (1u64 << self.power) as f64 };
        match self.sub_kernel_id {
            kernel_id::INIT => Box::new(Init),
            kernel_id::ENTANGLE => Box::new(Entangle),
            kernel_id::FOURIER => Box::new(Fourier),
            kernel_id::DIFFUSE => Box::new(Diffuse),
            kernel_id::GROVER_ITER => Box::new(GroverIter::single(self.param_re as u16)),
            kernel_id::ROTATE => Box::new(Rotate { theta: self.param_re * scale }),
            kernel_id::PHASE_SHIFT => Box::new(PhaseShift {
                amplitude: (self.param_re * scale, self.param_im * scale),
            }),
            kernel_id::FOURIER_INV => Box::new(FourierInv),
            // Default: treat as identity-like (will error if applied to wrong dim)
            _ => Box::new(Init),
        }
    }

    /// Whether the power can be folded into the parameter.
    fn is_power_foldable(&self) -> bool {
        matches!(self.sub_kernel_id, kernel_id::ROTATE | kernel_id::PHASE_SHIFT)
    }
}

/// Remove bit at position `bit_pos` from index `k`, collapsing to (n-1)-bit index.
///
/// Example: remove_bit(0b11010, 2) with n=5 bits:
///   bits above pos 2: 0b110__ → shifted right by 1 → 0b11___
///   bits below pos 2: 0b___10
///   result: 0b1110 (4-bit value)
#[inline]
fn remove_bit(k: usize, bit_pos: usize) -> usize {
    let above = (k >> (bit_pos + 1)) << bit_pos;
    let below = k & ((1 << bit_pos) - 1);
    above | below
}

/// Insert a bit value at position `bit_pos`, expanding from (n-1)-bit to n-bit index.
#[inline]
#[cfg(test)]
fn insert_bit(sub_k: usize, bit_pos: usize, bit_val: usize) -> usize {
    let above = (sub_k >> bit_pos) << (bit_pos + 1);
    let below = sub_k & ((1 << bit_pos) - 1);
    above | (bit_val << bit_pos) | below
}

impl Kernel for ControlledU {
    fn apply(&self, input: &DensityMatrix) -> Result<DensityMatrix, CqamError> {
        let n = input.num_qubits();
        if n < 2 {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/CONTROLLED_U".to_string(),
                detail: format!("Controlled-U requires >= 2 qubits, got {}", n),
            });
        }
        if self.control_qubit >= n {
            return Err(CqamError::AddressOutOfRange {
                instruction: "QKERNEL/CONTROLLED_U".to_string(),
                address: self.control_qubit as i64,
            });
        }

        let dim = input.dimension();
        let sub_n = n - 1;
        let sub_dim = 1usize << sub_n;
        let ctrl_bit_pos = (n - 1 - self.control_qubit) as usize;

        // Build the sub-unitary by probing the sub-kernel with each basis vector.
        let sub_kernel = self.build_sub_kernel();
        let need_loop = !self.is_power_foldable() && self.power > 0;
        let loop_count = if need_loop { 1u64 << self.power } else { 1 };

        // Build sub-unitary matrix U_sub by applying the kernel to each basis vector
        let mut sub_u = vec![complex::ZERO; sub_dim * sub_dim];
        for col in 0..sub_dim {
            // Create basis vector |col⟩
            let mut basis = vec![complex::ZERO; sub_dim];
            basis[col] = complex::ONE;
            let sv = Statevector::from_amplitudes(basis)
                .map_err(|e| CqamError::TypeMismatch {
                    instruction: "QKERNEL/CONTROLLED_U".to_string(),
                    detail: e,
                })?;

            // Apply sub-kernel (potentially multiple times for power)
            let mut result_sv = sv;
            for _ in 0..loop_count {
                result_sv = sub_kernel.apply_sv(&result_sv)
                    .map_err(|_| {
                        // Fallback: use density matrix path for sub-kernel
                        CqamError::TypeMismatch {
                            instruction: "QKERNEL/CONTROLLED_U".to_string(),
                            detail: "sub-kernel does not support statevector probing".to_string(),
                        }
                    })?;
            }

            let result_amps = result_sv.amplitudes();
            for row in 0..sub_dim {
                sub_u[row * sub_dim + col] = result_amps[row];
            }
        }

        // Build full C_U matrix: identity on |ctrl=0⟩, sub_u on |ctrl=1⟩
        let mut cu = vec![complex::ZERO; dim * dim];
        for row in 0..dim {
            for col in 0..dim {
                let row_ctrl = (row >> ctrl_bit_pos) & 1;
                let col_ctrl = (col >> ctrl_bit_pos) & 1;

                if row_ctrl != col_ctrl {
                    // Cross-block: zero
                    continue;
                }

                if row_ctrl == 0 {
                    // Identity block
                    if row == col {
                        cu[row * dim + col] = complex::ONE;
                    }
                } else {
                    // Sub-unitary block
                    let sub_row = remove_bit(row, ctrl_bit_pos);
                    let sub_col = remove_bit(col, ctrl_bit_pos);
                    cu[row * dim + col] = sub_u[sub_row * sub_dim + sub_col];
                }
            }
        }

        let mut result = input.clone();
        result.apply_unitary(&cu);
        Ok(result)
    }

    fn apply_sv(&self, input: &Statevector) -> Result<Statevector, String> {
        let n = input.num_qubits();
        if n < 2 {
            return Err(format!("Controlled-U requires >= 2 qubits, got {}", n));
        }
        if self.control_qubit >= n {
            return Err(format!(
                "Control qubit {} out of range for {}-qubit register",
                self.control_qubit, n
            ));
        }

        let sub_n = n - 1;
        let sub_dim = 1usize << sub_n;
        let ctrl_bit_pos = (n - 1 - self.control_qubit) as usize;
        let ctrl_mask = 1usize << ctrl_bit_pos;

        let amps = input.amplitudes();

        // Extract sub-amplitudes for |ctrl=1⟩ subset
        let mut sub_amps = vec![complex::ZERO; sub_dim];
        let mut norm_sq_ctrl1 = 0.0f64;

        for (k, &amp) in amps.iter().enumerate() {
            if k & ctrl_mask != 0 {
                let sub_k = remove_bit(k, ctrl_bit_pos);
                sub_amps[sub_k] = amp;
                norm_sq_ctrl1 += cx_norm_sq(amp);
            }
        }

        // If ctrl=1 subspace is empty, nothing to do
        if norm_sq_ctrl1 < 1e-30 {
            return Ok(Statevector::from_amplitudes(amps.to_vec())
                .expect("ControlledU: pass-through failed"));
        }

        // Normalize sub-amplitudes
        let norm_ctrl1 = norm_sq_ctrl1.sqrt();
        let inv_norm = 1.0 / norm_ctrl1;
        let normalized: Vec<C64> = sub_amps.iter()
            .map(|&a| (a.0 * inv_norm, a.1 * inv_norm))
            .collect();

        // Apply sub-kernel to normalized sub-state
        let sub_sv = Statevector::from_amplitudes(normalized)
            .map_err(|e| format!("ControlledU sub-state error: {}", e))?;

        let sub_kernel = self.build_sub_kernel();
        let need_loop = !self.is_power_foldable() && self.power > 0;
        let loop_count = if need_loop { 1u64 << self.power } else { 1 };

        let mut result_sv = sub_sv;
        for _ in 0..loop_count {
            result_sv = sub_kernel.apply_sv(&result_sv)?;
        }

        // Rescale back and reinsert
        let result_sub = result_sv.amplitudes();
        let mut out_amps = amps.to_vec();

        for (k, out_amp) in out_amps.iter_mut().enumerate() {
            if k & ctrl_mask != 0 {
                let sub_k = remove_bit(k, ctrl_bit_pos);
                *out_amp = (
                    result_sub[sub_k].0 * norm_ctrl1,
                    result_sub[sub_k].1 * norm_ctrl1,
                );
            }
        }

        Ok(Statevector::from_amplitudes(out_amps)
            .expect("ControlledU apply_sv produced invalid amplitudes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_insert_bit() {
        // remove_bit(0b11010, 2) removes bit at pos 2 (which is 0) → 0b1110
        assert_eq!(remove_bit(0b11010, 2), 0b1110);
        // insert_bit(0b1110, 2, 0) re-inserts 0 at pos 2 → 0b11010
        assert_eq!(insert_bit(0b1110, 2, 0), 0b11010);
        // insert_bit(0b1110, 2, 1) inserts 1 at pos 2 → 0b11110
        assert_eq!(insert_bit(0b1110, 2, 1), 0b11110);
        // Round-trip: remove then re-insert with original bit value
        let k = 0b10110;
        let bit_pos = 1;
        let bit_val = (k >> bit_pos) & 1;
        let sub = remove_bit(k, bit_pos);
        assert_eq!(insert_bit(sub, bit_pos, bit_val), k);
    }

    #[test]
    fn test_controlled_rotate_ctrl0_identity() {
        // If control qubit is |0⟩, state should be unchanged
        // 2-qubit state: |00⟩ (ctrl=qubit 0 is |0⟩)
        let sv = Statevector::new_zero_state(2); // |00⟩
        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: 1.0,
            param_im: 0.0,
        };
        let result = cu.apply_sv(&sv).unwrap();
        // |00⟩ should remain |00⟩ since control is 0
        let amps = result.amplitudes();
        assert!((amps[0].0 - 1.0).abs() < 1e-10);
        assert!(amps[1].0.abs() < 1e-10);
        assert!(amps[2].0.abs() < 1e-10);
        assert!(amps[3].0.abs() < 1e-10);
    }

    #[test]
    fn test_controlled_rotate_ctrl1_applies() {
        // 2-qubit state: |10⟩ (ctrl=qubit 0 is |1⟩)
        let sv = Statevector::from_amplitudes(vec![
            complex::ZERO, complex::ZERO, complex::ONE, complex::ZERO,
        ]).unwrap();
        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: std::f64::consts::PI,
            param_im: 0.0,
        };
        let result = cu.apply_sv(&sv).unwrap();
        // Sub-state is 1-qubit |0⟩, Rotate(pi) on |0⟩ = exp(i*pi*0)*|0⟩ = |0⟩
        // So |10⟩ → |10⟩
        let amps = result.amplitudes();
        assert!((amps[2].0 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_controlled_u_density_matrix() {
        // Verify DM path produces same result as SV path
        let sv = Statevector::from_amplitudes(vec![
            complex::ZERO, complex::ZERO, complex::ONE, complex::ZERO,
        ]).unwrap();
        let dm = sv.to_density_matrix();
        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: 1.0,
            param_im: 0.0,
        };
        let result_sv = cu.apply_sv(&sv).unwrap();
        let result_dm = cu.apply(&dm).unwrap();

        // Compare diagonal probabilities
        let sv_probs: Vec<f64> = result_sv.amplitudes().iter()
            .map(|a| cx_norm_sq(*a))
            .collect();
        let dm_dim = result_dm.dimension();
        let dm_probs: Vec<f64> = (0..dm_dim)
            .map(|i| result_dm.get(i, i).0)
            .collect();
        for i in 0..sv_probs.len() {
            assert!((sv_probs[i] - dm_probs[i]).abs() < 1e-9,
                "Mismatch at {}: sv={}, dm={}", i, sv_probs[i], dm_probs[i]);
        }
    }
}
