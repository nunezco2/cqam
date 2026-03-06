//! Controlled-U kernel (kernel_id = 8).
//!
//! Applies a sub-kernel conditioned on a control qubit within the register.
//! When the control qubit is |1⟩, the sub-kernel U is applied to the target
//! qubits (bottom `target_qubits` of the register, excluding the control);
//! when |0⟩, the state is unchanged.
//!
//! Supports power application: U^{2^k}. For ROTATE and PHASE_SHIFT kernels,
//! the power is folded into the parameter (theta * 2^k) in O(1). For other
//! kernels, the sub-kernel is applied 2^k times in a loop.
//!
//! When `target_qubits = 0`, the sub-kernel acts on ALL (n-1) non-control
//! qubits (original behavior). When `target_qubits > 0`, only the bottom t
//! qubits of the register are targeted; the remaining spectator qubits pass
//! through as identity. This is essential for QPE where counting qubits must
//! not be affected by the sub-kernel.

use cqam_core::error::CqamError;
use cqam_core::instruction::kernel_id;
use crate::complex;
#[cfg(test)]
use crate::complex::C64;
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

/// Controlled-U kernel parameterized by control qubit, sub-kernel, power,
/// and optional target qubit count.
pub struct ControlledU {
    pub control_qubit: u8,
    pub sub_kernel_id: u8,
    pub power: u32,
    pub param_re: f64,
    pub param_im: f64,
    /// Number of bottom qubits in the register to target with the sub-kernel.
    /// 0 means all (n-1) non-control qubits (backward compatible).
    pub target_qubits: u8,
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
            _ => Box::new(Init),
        }
    }

    /// Whether the power can be folded into the parameter.
    fn is_power_foldable(&self) -> bool {
        matches!(self.sub_kernel_id, kernel_id::ROTATE | kernel_id::PHASE_SHIFT)
    }

    /// Effective number of target qubits for the sub-kernel, given register size n.
    fn effective_target_qubits(&self, n: u8) -> u8 {
        if self.target_qubits == 0 {
            n - 1 // all non-control qubits
        } else {
            self.target_qubits
        }
    }
}

/// Remove bit at position `bit_pos` from index `k`, collapsing to (n-1)-bit index.
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

        let t = self.effective_target_qubits(n);
        if t >= n {
            return Err(CqamError::TypeMismatch {
                instruction: "QKERNEL/CONTROLLED_U".to_string(),
                detail: format!("target_qubits {} >= register size {}", t, n),
            });
        }

        let dim = input.dimension();
        let target_dim = 1usize << t;
        let ctrl_bit_pos = (n - 1 - self.control_qubit) as usize;

        // Build sub-unitary for the target qubits only (t-qubit system).
        let sub_kernel = self.build_sub_kernel();
        let need_loop = !self.is_power_foldable() && self.power > 0;
        let loop_count = if need_loop { 1u64 << self.power } else { 1 };

        let mut target_u = vec![complex::ZERO; target_dim * target_dim];
        for col in 0..target_dim {
            let mut basis = vec![complex::ZERO; target_dim];
            basis[col] = complex::ONE;
            let sv = Statevector::from_amplitudes(basis)
                .map_err(|e| CqamError::TypeMismatch {
                    instruction: "QKERNEL/CONTROLLED_U".to_string(),
                    detail: e,
                })?;

            let mut result_sv = sv;
            for _ in 0..loop_count {
                result_sv = sub_kernel.apply_sv(&result_sv)
                    .map_err(|_| CqamError::TypeMismatch {
                        instruction: "QKERNEL/CONTROLLED_U".to_string(),
                        detail: "sub-kernel does not support statevector probing".to_string(),
                    })?;
            }

            let result_amps = result_sv.amplitudes();
            for row in 0..target_dim {
                target_u[row * target_dim + col] = result_amps[row];
            }
        }

        // Build full C_U matrix.
        // For |ctrl=0⟩: identity.
        // For |ctrl=1⟩: sub_u on target qubits (bottom t bits of (n-1)-bit index),
        //               identity on spectator qubits (upper bits).
        let target_mask = target_dim - 1; // mask for bottom t bits of sub-index
        let mut cu = vec![complex::ZERO; dim * dim];
        for row in 0..dim {
            for col in 0..dim {
                let row_ctrl = (row >> ctrl_bit_pos) & 1;
                let col_ctrl = (col >> ctrl_bit_pos) & 1;

                if row_ctrl != col_ctrl {
                    continue;
                }

                if row_ctrl == 0 {
                    if row == col {
                        cu[row * dim + col] = complex::ONE;
                    }
                } else {
                    let sub_row = remove_bit(row, ctrl_bit_pos);
                    let sub_col = remove_bit(col, ctrl_bit_pos);
                    // Spectator bits must match (identity on spectators)
                    let spec_row = sub_row >> t;
                    let spec_col = sub_col >> t;
                    if spec_row != spec_col {
                        continue;
                    }
                    // Target bits: apply sub-unitary
                    let tgt_row = sub_row & target_mask;
                    let tgt_col = sub_col & target_mask;
                    cu[row * dim + col] = target_u[tgt_row * target_dim + tgt_col];
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

        let t = self.effective_target_qubits(n);
        if t >= n {
            return Err(format!("target_qubits {} >= register size {}", t, n));
        }

        let target_dim = 1usize << t;
        let ctrl_bit_pos = (n - 1 - self.control_qubit) as usize;
        let ctrl_mask = 1usize << ctrl_bit_pos;

        let amps = input.amplitudes();

        // Build sub-kernel unitary on target qubits
        let sub_kernel = self.build_sub_kernel();
        let need_loop = !self.is_power_foldable() && self.power > 0;
        let loop_count = if need_loop { 1u64 << self.power } else { 1 };

        let mut target_u = vec![complex::ZERO; target_dim * target_dim];
        for col in 0..target_dim {
            let mut basis = vec![complex::ZERO; target_dim];
            basis[col] = complex::ONE;
            let sv = Statevector::from_amplitudes(basis)
                .map_err(|e| format!("ControlledU sub-state error: {}", e))?;
            let mut result_sv = sv;
            for _ in 0..loop_count {
                result_sv = sub_kernel.apply_sv(&result_sv)?;
            }
            let result_amps = result_sv.amplitudes();
            for row in 0..target_dim {
                target_u[row * target_dim + col] = result_amps[row];
            }
        }

        // Apply: for each |ctrl=1⟩ amplitude, transform target bits using target_u,
        // keeping spectator bits fixed.
        let mut out_amps = amps.to_vec();

        // Group amplitudes by (ctrl=1, spectator bits) and apply target_u
        let sub_n = (n - 1) as usize;
        let num_spectator_configs = 1usize << (sub_n - t as usize);

        for spec in 0..num_spectator_configs {
            // Collect target amplitudes for this spectator configuration
            let mut target_amps = vec![complex::ZERO; target_dim];
            let mut full_indices = vec![0usize; target_dim];

            for tgt in 0..target_dim {
                // Reconstruct sub-index: spectator bits in upper, target in lower
                let sub_idx = (spec << t) | tgt;
                // Reconstruct full index by inserting control bit = 1
                let above = (sub_idx >> ctrl_bit_pos) << (ctrl_bit_pos + 1);
                let below = sub_idx & ((1 << ctrl_bit_pos) - 1);
                let full_idx = above | ctrl_mask | below;
                full_indices[tgt] = full_idx;
                target_amps[tgt] = amps[full_idx];
            }

            // Apply target_u to these amplitudes
            let mut new_target = vec![complex::ZERO; target_dim];
            for row in 0..target_dim {
                let mut sum = complex::ZERO;
                for col in 0..target_dim {
                    let u_elem = target_u[row * target_dim + col];
                    let a = target_amps[col];
                    sum.0 += u_elem.0 * a.0 - u_elem.1 * a.1;
                    sum.1 += u_elem.0 * a.1 + u_elem.1 * a.0;
                }
                new_target[row] = sum;
            }

            // Write back
            for tgt in 0..target_dim {
                out_amps[full_indices[tgt]] = new_target[tgt];
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
        assert_eq!(remove_bit(0b11010, 2), 0b1110);
        assert_eq!(insert_bit(0b1110, 2, 0), 0b11010);
        assert_eq!(insert_bit(0b1110, 2, 1), 0b11110);
        let k = 0b10110;
        let bit_pos = 1;
        let bit_val = (k >> bit_pos) & 1;
        let sub = remove_bit(k, bit_pos);
        assert_eq!(insert_bit(sub, bit_pos, bit_val), k);
    }

    #[test]
    fn test_controlled_rotate_ctrl0_identity() {
        let sv = Statevector::new_zero_state(2);
        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: 1.0,
            param_im: 0.0,
            target_qubits: 0,
        };
        let result = cu.apply_sv(&sv).unwrap();
        let amps = result.amplitudes();
        assert!((amps[0].0 - 1.0).abs() < 1e-10);
        assert!(amps[1].0.abs() < 1e-10);
        assert!(amps[2].0.abs() < 1e-10);
        assert!(amps[3].0.abs() < 1e-10);
    }

    #[test]
    fn test_controlled_rotate_ctrl1_applies() {
        let sv = Statevector::from_amplitudes(vec![
            complex::ZERO, complex::ZERO, complex::ONE, complex::ZERO,
        ]).unwrap();
        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: std::f64::consts::PI,
            param_im: 0.0,
            target_qubits: 0,
        };
        let result = cu.apply_sv(&sv).unwrap();
        let amps = result.amplitudes();
        assert!((amps[2].0 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_controlled_u_density_matrix() {
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
            target_qubits: 0,
        };
        let result_sv = cu.apply_sv(&sv).unwrap();
        let result_dm = cu.apply(&dm).unwrap();

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

    #[test]
    fn test_target_qubits_selective() {
        // 3-qubit register: qubit 0 = control, qubit 1 = spectator, qubit 2 = target
        // State: |1⟩|0⟩|1⟩ = |101⟩ = basis state 5
        // C-ROTATE(theta=pi) on target_qubits=1 should only phase qubit 2
        let sv = Statevector::from_amplitudes(vec![
            complex::ZERO, complex::ZERO, complex::ZERO, complex::ZERO,
            complex::ZERO, complex::ONE, complex::ZERO, complex::ZERO,
        ]).unwrap(); // |101⟩

        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: std::f64::consts::PI,
            param_im: 0.0,
            target_qubits: 1, // only bottom 1 qubit
        };

        let result = cu.apply_sv(&sv).unwrap();
        let amps = result.amplitudes();
        // Target qubit (qubit 2) has value 1, so ROTATE(pi) gives exp(i*pi*1) = -1
        // State should become -|101⟩
        let prob_5 = cx_norm_sq(amps[5]);
        assert!((prob_5 - 1.0).abs() < 1e-10, "Expected all probability at |101⟩");
        // Phase should be exp(i*pi) = -1
        assert!((amps[5].0 - (-1.0)).abs() < 1e-10, "Expected phase -1");
    }

    #[test]
    fn test_target_qubits_spectator_unchanged() {
        // 3-qubit register: qubit 0 = control, qubit 1 = spectator, qubit 2 = target
        // Superposition of |100⟩ and |110⟩ (different spectator values)
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let sv = Statevector::from_amplitudes(vec![
            complex::ZERO, complex::ZERO, complex::ZERO, complex::ZERO,
            (h, 0.0), complex::ZERO, (h, 0.0), complex::ZERO,
        ]).unwrap(); // (|100⟩ + |110⟩)/sqrt(2)

        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: std::f64::consts::PI,
            param_im: 0.0,
            target_qubits: 1,
        };

        let result = cu.apply_sv(&sv).unwrap();
        let amps = result.amplitudes();
        // Target qubit = 0 in both cases, ROTATE(pi)*|0⟩ = exp(i*pi*0)|0⟩ = |0⟩
        // So state should be unchanged
        assert!((cx_norm_sq(amps[4]) - 0.5).abs() < 1e-10);
        assert!((cx_norm_sq(amps[6]) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_target_qubits_sv_dm_consistency() {
        // Verify SV and DM paths agree with target_qubits
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let sv = Statevector::from_amplitudes(vec![
            complex::ZERO, complex::ZERO, complex::ZERO, complex::ZERO,
            (h, 0.0), (h, 0.0), complex::ZERO, complex::ZERO,
        ]).unwrap(); // (|100⟩ + |101⟩)/sqrt(2)

        let dm = sv.to_density_matrix();
        let cu = ControlledU {
            control_qubit: 0,
            sub_kernel_id: kernel_id::ROTATE,
            power: 0,
            param_re: 1.0,
            param_im: 0.0,
            target_qubits: 1,
        };

        let result_sv = cu.apply_sv(&sv).unwrap();
        let result_dm = cu.apply(&dm).unwrap();

        let sv_probs: Vec<f64> = result_sv.amplitudes().iter()
            .map(|a| cx_norm_sq(*a))
            .collect();
        let dm_probs: Vec<f64> = (0..result_dm.dimension())
            .map(|i| result_dm.get(i, i).0)
            .collect();
        for i in 0..sv_probs.len() {
            assert!((sv_probs[i] - dm_probs[i]).abs() < 1e-9,
                "Mismatch at {}: sv={}, dm={}", i, sv_probs[i], dm_probs[i]);
        }
    }
}
