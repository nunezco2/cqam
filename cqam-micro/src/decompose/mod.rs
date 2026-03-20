//! Kernel decomposition: transforms high-level kernel ops into standard gates.
//!
//! Each CQAM kernel (Init, Entangle, Fourier, etc.) is decomposed into a
//! sequence of standard gates (H, X, Z, S, Sdg, T, Tdg, Rx, Ry, Rz, CX, CZ,
//! SWAP) that can subsequently be mapped to hardware-native gate sets.
//!
//! Qubit ordering follows the cqam-sim big-endian convention: qubit 0 is the
//! most significant bit.

mod fourier;
mod grover;
mod rotation;
mod diagonal;
mod permutation;

use cqam_core::circuit_ir::{self, ApplyGate1q, ApplyGate2q, Gate1q, Gate2q, Op, Param, QWire};
use cqam_core::complex::C64;
use cqam_core::instruction::{DistId, KernelId};
use cqam_core::quantum_backend::KernelParams;
use crate::error::MicroError;

// =============================================================================
// ZYZ Euler decomposition (shared utility for CustomUnitary handling)
// =============================================================================

/// ZYZ Euler decomposition of a 2x2 unitary matrix.
///
/// Returns `Some((theta, phi, lambda))` such that
///   U = e^{ig} * U3(theta, phi, lambda)
/// where U3(θ, φ, λ) = Rz(φ) * Ry(θ) * Rz(λ)
///                    = | cos(θ/2)           -e^{iλ}*sin(θ/2) |
///                      | e^{iφ}*sin(θ/2)     e^{i(φ+λ)}*cos(θ/2) |
///
/// Matrix layout (row-major): [a, b, c, d] where U = |a b|
///                                                     |c d|
///   phi    = arg(c) - arg(a)
///   lambda = arg(b) - arg(a) - π
fn decompose_zyz_2x2(mat: &[C64]) -> Option<(f64, f64, f64)> {
    if mat.len() < 4 {
        return None;
    }
    let a = mat[0]; // U[0,0]
    let b = mat[1]; // U[0,1]
    let c = mat[2]; // U[1,0]
    let d = mat[3]; // U[1,1]

    let a_norm = a.norm();
    let theta = 2.0 * a_norm.clamp(0.0, 1.0).acos();

    if theta.abs() < 1e-9 {
        // theta ≈ 0: identity-like (U ≈ e^{ig} * diag(1, e^{i(φ+λ)}))
        // Only φ+λ is observable; fix φ=arg(d)-arg(a), λ=0.
        let arg_a = if a_norm < 1e-12 { 0.0 } else { a.1.atan2(a.0) };
        let d_norm = d.norm();
        let arg_d = if d_norm < 1e-12 { 0.0 } else { d.1.atan2(d.0) };
        return Some((0.0, arg_d - arg_a, 0.0));
    }

    if (theta - std::f64::consts::PI).abs() < 1e-9 {
        // theta ≈ pi: bit-flip-like; derive from b and c, fix lambda=0.
        let b_norm = b.norm();
        let c_norm = c.norm();
        let arg_b = if b_norm < 1e-12 { 0.0 } else { b.1.atan2(b.0) };
        let arg_c = if c_norm < 1e-12 { 0.0 } else { c.1.atan2(c.0) };
        let phi = arg_c - arg_b - std::f64::consts::PI;
        return Some((std::f64::consts::PI, phi, 0.0));
    }

    // General case: phi = arg(c) - arg(a), lambda = arg(b) - arg(a) - pi
    let arg_a = if a_norm < 1e-12 { 0.0 } else { a.1.atan2(a.0) };
    let b_norm = b.norm();
    let c_norm = c.norm();
    let arg_b = if b_norm < 1e-12 { 0.0 } else { b.1.atan2(b.0) };
    let arg_c = if c_norm < 1e-12 { 0.0 } else { c.1.atan2(c.0) };

    Some((theta, arg_c - arg_a, arg_b - arg_a - std::f64::consts::PI))
}

// =============================================================================
// Top-level decomposition
// =============================================================================

/// Decompose all high-level ops in a MicroProgram to the standard gate set.
///
/// Walks the ops list. For each op:
/// - Gate1q/Gate2q already in standard set: pass through unchanged.
/// - Kernel ops: dispatch to kernel-specific decomposer.
/// - Prep with non-Zero distribution: decompose into gate sequence after a
///   Zero-state prep (hardware assumes |0> reset; gates create the target state).
/// - Measure, Barrier, Reset, MeasQubit: pass through unchanged.
/// - CustomUnitary: return MicroError::UnsupportedGate.
pub fn decompose_to_standard(
    program: &circuit_ir::MicroProgram,
) -> Result<circuit_ir::MicroProgram, MicroError> {
    let mut out = circuit_ir::MicroProgram::new(program.num_wires);
    out.wire_map = program.wire_map.clone();
    for op in &program.ops {
        match op {
            Op::Prep(p) => {
                // Always emit a Zero prep (hardware reset to |0...0>).
                out.push(Op::Prep(circuit_ir::Prepare {
                    wires: p.wires.clone(),
                    dist: DistId::Zero,
                }));
                // Then emit gates to create the target distribution.
                let gates = decompose_prep_dist(&p.wires, p.dist);
                for g in gates {
                    out.push(g);
                }
            }
            Op::Kernel(k) => {
                let gates = decompose_kernel(&k.wires, &k.kernel, &k.params)?;
                for g in gates {
                    out.push(g);
                }
            }
            Op::CustomUnitary { wires, matrix } => {
                if wires.len() == 1 {
                    // Single-qubit custom unitary: ZYZ decompose to U3
                    let (theta, phi, lambda) = decompose_zyz_2x2(matrix)
                        .ok_or_else(|| MicroError::UnsupportedGate {
                            gate: "CustomUnitary: degenerate 1q matrix".to_string(),
                        })?;
                    out.push(Op::Gate1q(ApplyGate1q {
                        wire: wires[0],
                        gate: Gate1q::U3(
                            Param::Resolved(theta),
                            Param::Resolved(phi),
                            Param::Resolved(lambda),
                        ),
                    }));
                } else {
                    // Multi-qubit custom unitaries require KAK decomposition (future work)
                    return Err(MicroError::UnsupportedGate {
                        gate: format!("CustomUnitary ({}-qubit KAK decomposition not implemented)", wires.len()),
                    });
                }
            }
            Op::PrepProduct(pp) => {
                // Decompose into one U3 gate per qubit
                for (wire, (alpha, beta)) in pp.wires.iter().zip(&pp.amplitudes) {
                    let eps = 1e-12;
                    let alpha_abs = (alpha.0 * alpha.0 + alpha.1 * alpha.1).sqrt();

                    let (theta, phi, lambda) = if alpha_abs < eps {
                        (std::f64::consts::PI, beta.1.atan2(beta.0), 0.0)
                    } else {
                        let theta = 2.0 * alpha_abs.acos();
                        let phi = beta.1.atan2(beta.0);
                        let lambda = -(alpha.1.atan2(alpha.0));
                        (theta, phi, lambda)
                    };

                    out.push(Op::Gate1q(ApplyGate1q {
                        wire: *wire,
                        gate: Gate1q::U3(
                            circuit_ir::Param::Resolved(theta),
                            circuit_ir::Param::Resolved(phi),
                            circuit_ir::Param::Resolved(lambda),
                        ),
                    }));
                }
            }
            other => out.push(other.clone()),
        }
    }
    Ok(out)
}

/// Decompose a distribution prep into standard gates (applied after |0...0> reset).
///
/// - Zero: no gates needed (already |0...0>).
/// - Uniform: H on every wire → equal superposition.
/// - Bell: H on wire[0], CX(wire[0], wire[1]). Requires >= 2 wires.
/// - Ghz: H on wire[0], CX(wire[0], wire[k]) for k in 1..n.
fn decompose_prep_dist(wires: &[QWire], dist: DistId) -> Vec<Op> {
    match dist {
        DistId::Zero => vec![],
        DistId::Uniform => {
            wires.iter().map(|&w| Op::Gate1q(ApplyGate1q { wire: w, gate: Gate1q::H })).collect()
        }
        DistId::Bell => {
            if wires.len() < 2 {
                return vec![];
            }
            vec![
                Op::Gate1q(ApplyGate1q { wire: wires[0], gate: Gate1q::H }),
                Op::Gate2q(ApplyGate2q { wire_a: wires[0], wire_b: wires[1], gate: Gate2q::Cx }),
            ]
        }
        DistId::Ghz => {
            if wires.len() < 2 {
                return wires.iter().map(|&w| Op::Gate1q(ApplyGate1q { wire: w, gate: Gate1q::H })).collect();
            }
            let mut ops = Vec::with_capacity(wires.len());
            ops.push(Op::Gate1q(ApplyGate1q { wire: wires[0], gate: Gate1q::H }));
            for &w in &wires[1..] {
                ops.push(Op::Gate2q(ApplyGate2q { wire_a: wires[0], wire_b: w, gate: Gate2q::Cx }));
            }
            ops
        }
    }
}

/// Dispatch to the appropriate kernel decomposer.
fn decompose_kernel(
    wires: &[QWire],
    kernel: &KernelId,
    params: &KernelParams,
) -> Result<Vec<Op>, MicroError> {
    match kernel {
        KernelId::Init        => decompose_init(wires, params),
        KernelId::Entangle    => decompose_entangle(wires, params),
        KernelId::Fourier     => fourier::decompose_fourier(wires, params),
        KernelId::FourierInv  => fourier::decompose_fourier_inv(wires, params),
        KernelId::Diffuse     => grover::decompose_diffuse(wires, params),
        KernelId::GroverIter  => grover::decompose_grover(wires, params),
        KernelId::Rotate      => rotation::decompose_rotate(wires, params),
        KernelId::PhaseShift  => rotation::decompose_phase_shift(wires, params),
        KernelId::ControlledU => rotation::decompose_controlled_u(wires, params),
        KernelId::DiagonalUnitary => diagonal::decompose_diagonal_unitary(wires, params),
        KernelId::Permutation => permutation::decompose_permutation(wires, params),
    }
}

// =============================================================================
// Helper constructors (shared across sub-modules)
// =============================================================================

pub(super) mod helpers {
    use cqam_core::circuit_ir::{Op, QWire, Param, ApplyGate1q, ApplyGate2q,
        Gate1q, Gate2q};

    pub fn h(wire: QWire) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::H })
    }
    pub fn x(wire: QWire) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::X })
    }
    #[allow(dead_code)]
    pub fn z(wire: QWire) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::Z })
    }
    pub fn t_gate(wire: QWire) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::T })
    }
    pub fn tdg(wire: QWire) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::Tdg })
    }
    pub fn rz(wire: QWire, theta: f64) -> Op {
        Op::Gate1q(ApplyGate1q { wire, gate: Gate1q::Rz(Param::Resolved(theta)) })
    }
    pub fn cx(control: QWire, target: QWire) -> Op {
        Op::Gate2q(ApplyGate2q { wire_a: control, wire_b: target, gate: Gate2q::Cx })
    }
    #[allow(dead_code)]
    pub fn cz(a: QWire, b: QWire) -> Op {
        Op::Gate2q(ApplyGate2q { wire_a: a, wire_b: b, gate: Gate2q::Cz })
    }
    pub fn swap(a: QWire, b: QWire) -> Op {
        Op::Gate2q(ApplyGate2q { wire_a: a, wire_b: b, gate: Gate2q::Swap })
    }
}

// =============================================================================
// Parameter extraction helpers (shared across sub-modules)
// =============================================================================

pub(super) mod params {
    use cqam_core::quantum_backend::KernelParams;
    use crate::error::MicroError;

    /// Extract f64 theta from KernelParams::Float { param0, .. }.
    pub fn extract_float_param0(params: &KernelParams, kernel_name: &str) -> Result<f64, MicroError> {
        match params {
            KernelParams::Float { param0, .. } => Ok(*param0),
            _ => Err(MicroError::DecompositionFailed {
                kernel: kernel_name.to_string(),
                detail: "expected Float params".to_string(),
            }),
        }
    }

    /// Extract C64 from KernelParams::Complex { param0, .. }.
    pub fn extract_complex_param0(params: &KernelParams, kernel_name: &str)
        -> Result<cqam_core::complex::C64, MicroError>
    {
        match params {
            KernelParams::Complex { param0, .. } => Ok(*param0),
            _ => Err(MicroError::DecompositionFailed {
                kernel: kernel_name.to_string(),
                detail: "expected Complex params".to_string(),
            }),
        }
    }

    /// Extract Int params (param0, param1, cmem_data).
    pub fn extract_int_params<'a>(params: &'a KernelParams, kernel_name: &str)
        -> Result<(i64, i64, &'a Vec<i64>), MicroError>
    {
        match params {
            KernelParams::Int { param0, param1, cmem_data } => Ok((*param0, *param1, cmem_data)),
            _ => Err(MicroError::DecompositionFailed {
                kernel: kernel_name.to_string(),
                detail: "expected Int params".to_string(),
            }),
        }
    }
}

// =============================================================================
// Kernel: Init
// =============================================================================

/// Decompose Init kernel: H on each wire.
///
/// The Init kernel produces H^n|0> (uniform superposition). The gate
/// decomposition H^n only matches when the input is |0...0>, which is
/// always the case because Init follows QPREP.
fn decompose_init(wires: &[QWire], _params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    Ok(wires.iter().map(|&w| helpers::h(w)).collect())
}

// =============================================================================
// Kernel: Entangle
// =============================================================================

/// Decompose Entangle kernel: CX(wires[0], wires[1]).
fn decompose_entangle(wires: &[QWire], _params: &KernelParams) -> Result<Vec<Op>, MicroError> {
    if wires.len() < 2 {
        return Err(MicroError::DecompositionFailed {
            kernel: "Entangle".to_string(),
            detail: format!("requires >= 2 wires, got {}", wires.len()),
        });
    }
    Ok(vec![helpers::cx(wires[0], wires[1])])
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::circuit_ir::{ApplyGate1q, ApplyGate2q, Gate1q, Gate2q};
    use cqam_core::complex::C64;
    use cqam_sim::statevector::Statevector;
    use cqam_sim::kernel::Kernel as SimKernel;
    use cqam_sim::kernels::init::Init;
    use cqam_sim::kernels::entangle::Entangle;
    use cqam_sim::kernels::fourier::Fourier;
    use cqam_sim::kernels::fourier_inv::FourierInv;
    use cqam_sim::kernels::diffuse::Diffuse;
    use cqam_sim::kernels::grover::GroverIter;
    use cqam_sim::kernels::rotate::Rotate;
    use cqam_sim::kernels::phase::PhaseShift;
    use cqam_core::quantum_backend::KernelParams;

    // =========================================================================
    // Mini statevector simulator for test verification
    // =========================================================================

    /// Apply a sequence of circuit_ir Ops to a statevector (big-endian convention).
    pub(super) fn apply_ops_to_sv(amps: &[C64], ops: &[Op], n: u8) -> Vec<C64> {
        let mut state = amps.to_vec();
        for op in ops {
            match op {
                Op::Gate1q(g) => {
                    let mat = gate1q_matrix(&g.gate);
                    apply_1q_gate(&mut state, g.wire.0 as usize, n as usize, &mat);
                }
                Op::Gate2q(g) => {
                    let mat = gate2q_matrix(&g.gate);
                    apply_2q_gate(&mut state, g.wire_a.0 as usize, g.wire_b.0 as usize, n as usize, &mat);
                }
                _ => {} // Prep, Measure, etc. don't affect unitary
            }
        }
        state
    }

    /// Get the 2x2 unitary matrix for a single-qubit gate.
    pub(super) fn gate1q_matrix(gate: &Gate1q) -> [C64; 4] {
        let h_val = std::f64::consts::FRAC_1_SQRT_2;
        match gate {
            Gate1q::H => [
                C64(h_val, 0.0), C64(h_val, 0.0),
                C64(h_val, 0.0), C64(-h_val, 0.0),
            ],
            Gate1q::X => [
                C64::ZERO, C64::ONE,
                C64::ONE, C64::ZERO,
            ],
            Gate1q::Y => [
                C64::ZERO, C64(0.0, -1.0),
                C64(0.0, 1.0), C64::ZERO,
            ],
            Gate1q::Z => [
                C64::ONE, C64::ZERO,
                C64::ZERO, C64(-1.0, 0.0),
            ],
            Gate1q::S => [
                C64::ONE, C64::ZERO,
                C64::ZERO, C64::I,
            ],
            Gate1q::Sdg => [
                C64::ONE, C64::ZERO,
                C64::ZERO, C64(0.0, -1.0),
            ],
            Gate1q::T => {
                let v = std::f64::consts::FRAC_1_SQRT_2;
                [
                    C64::ONE, C64::ZERO,
                    C64::ZERO, C64(v, v),
                ]
            }
            Gate1q::Tdg => {
                let v = std::f64::consts::FRAC_1_SQRT_2;
                [
                    C64::ONE, C64::ZERO,
                    C64::ZERO, C64(v, -v),
                ]
            }
            Gate1q::Rx(p) => {
                let t = p.value().unwrap();
                let c = (t / 2.0).cos();
                let s = (t / 2.0).sin();
                [
                    C64(c, 0.0), C64(0.0, -s),
                    C64(0.0, -s), C64(c, 0.0),
                ]
            }
            Gate1q::Ry(p) => {
                let t = p.value().unwrap();
                let c = (t / 2.0).cos();
                let s = (t / 2.0).sin();
                [
                    C64(c, 0.0), C64(-s, 0.0),
                    C64(s, 0.0), C64(c, 0.0),
                ]
            }
            Gate1q::Rz(p) => {
                let t = p.value().unwrap();
                [
                    C64::exp_i(-t / 2.0), C64::ZERO,
                    C64::ZERO, C64::exp_i(t / 2.0),
                ]
            }
            Gate1q::U3(_, _, _) => {
                panic!("U3 not expected in decomposition output");
            }
            Gate1q::Custom(_) => {
                panic!("Custom gate not expected in decomposition output");
            }
        }
    }

    /// Get the 4x4 unitary matrix for a two-qubit gate (big-endian ordering).
    pub(super) fn gate2q_matrix(gate: &Gate2q) -> [C64; 16] {
        match gate {
            Gate2q::Cx => [
                C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
                C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
                C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE,
                C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
            ],
            Gate2q::Cz => [
                C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
                C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
                C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
                C64::ZERO, C64::ZERO, C64::ZERO, C64(-1.0, 0.0),
            ],
            Gate2q::Swap => [
                C64::ONE,  C64::ZERO, C64::ZERO, C64::ZERO,
                C64::ZERO, C64::ZERO, C64::ONE,  C64::ZERO,
                C64::ZERO, C64::ONE,  C64::ZERO, C64::ZERO,
                C64::ZERO, C64::ZERO, C64::ZERO, C64::ONE,
            ],
            Gate2q::EchoCrossResonance => {
                panic!("ECR not expected in decomposition output");
            }
            Gate2q::Custom(_) => {
                panic!("Custom gate not expected in decomposition output");
            }
        }
    }

    /// Apply a single-qubit gate to the statevector (big-endian convention).
    /// qubit_idx: the qubit number (0 = MSB).
    pub(super) fn apply_1q_gate(state: &mut [C64], qubit_idx: usize, n: usize, mat: &[C64; 4]) {
        let dim = 1 << n;
        let bit = 1 << (n - 1 - qubit_idx);
        for s in 0..dim {
            if s & bit == 0 {
                let partner = s | bit;
                let a = state[s];
                let b = state[partner];
                state[s] = mat[0] * a + mat[1] * b;
                state[partner] = mat[2] * a + mat[3] * b;
            }
        }
    }

    /// Apply a two-qubit gate to the statevector (big-endian convention).
    /// wire_a, wire_b: qubit indices (0 = MSB).
    pub(super) fn apply_2q_gate(state: &mut [C64], wire_a: usize, wire_b: usize, n: usize, mat: &[C64; 16]) {
        let dim = 1 << n;
        let bit_a = 1 << (n - 1 - wire_a);
        let bit_b = 1 << (n - 1 - wire_b);

        let mut processed = vec![false; dim];
        for s in 0..dim {
            if processed[s] {
                continue;
            }
            // The four basis states for the two-qubit subspace
            let s00 = s & !(bit_a | bit_b);
            let s01 = s00 | bit_b;
            let s10 = s00 | bit_a;
            let s11 = s00 | bit_a | bit_b;

            let v = [state[s00], state[s01], state[s10], state[s11]];
            for (i, &idx) in [s00, s01, s10, s11].iter().enumerate() {
                let mut sum = C64::ZERO;
                for (j, &_vidx) in [s00, s01, s10, s11].iter().enumerate() {
                    sum = sum + mat[i * 4 + j] * v[j];
                }
                state[idx] = sum;
            }
            processed[s00] = true;
            processed[s01] = true;
            processed[s10] = true;
            processed[s11] = true;
        }
    }

    /// Compute the unitary matrix of a gate sequence by probing each basis state.
    pub(super) fn gate_sequence_unitary(ops: &[Op], n: u8) -> Vec<C64> {
        let dim = 1usize << n;
        let mut unitary = vec![C64::ZERO; dim * dim];
        for col in 0..dim {
            let mut amps = vec![C64::ZERO; dim];
            amps[col] = C64::ONE;
            let result = apply_ops_to_sv(&amps, ops, n);
            for row in 0..dim {
                unitary[row * dim + col] = result[row];
            }
        }
        unitary
    }

    /// Get the reference unitary by probing a sim kernel with basis states.
    pub(super) fn kernel_unitary(kernel: &dyn SimKernel, n: u8) -> Vec<C64> {
        let dim = 1usize << n;
        let mut unitary = vec![C64::ZERO; dim * dim];
        for col in 0..dim {
            let mut amps = vec![C64::ZERO; dim];
            amps[col] = C64::ONE;
            let sv = Statevector::from_amplitudes(amps).unwrap();
            let result = kernel.apply_sv(&sv).unwrap();
            let result_amps = result.amplitudes();
            for row in 0..dim {
                unitary[row * dim + col] = result_amps[row];
            }
        }
        unitary
    }

    /// Compare two unitary matrices allowing global phase difference.
    /// Returns true if they are equal up to a global phase factor.
    pub(super) fn unitaries_equal_up_to_phase(a: &[C64], b: &[C64], tol: f64) -> bool {
        assert_eq!(a.len(), b.len());
        // Find global phase by comparing the first nonzero pair
        let mut phase = C64::ONE;
        let mut found = false;
        for i in 0..a.len() {
            if a[i].norm() > 1e-12 && b[i].norm() > 1e-12 {
                // phase = b[i] / a[i]
                let a_conj = a[i].conj();
                let num = C64(
                    b[i].0 * a_conj.0 - b[i].1 * a_conj.1,
                    b[i].0 * a_conj.1 + b[i].1 * a_conj.0,
                );
                let denom = a[i].norm_sq();
                phase = C64(num.0 / denom, num.1 / denom);
                found = true;
                break;
            }
        }
        if !found {
            // Both matrices are all-zero (shouldn't happen for unitaries)
            return true;
        }

        // Compute Frobenius norm of (phase * a - b)
        let mut frob_sq = 0.0;
        for i in 0..a.len() {
            let pa = phase * a[i];
            let diff = C64(pa.0 - b[i].0, pa.1 - b[i].1);
            frob_sq += diff.norm_sq();
        }
        frob_sq.sqrt() < tol
    }

    pub(super) fn make_wires(n: usize) -> Vec<QWire> {
        (0..n).map(|i| QWire(i as u32)).collect()
    }

    // =========================================================================
    // Init tests
    // =========================================================================

    #[test]
    fn test_decompose_init_1q() {
        let wires = make_wires(1);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = decompose_init(&wires, &params).unwrap();
        assert_eq!(ops.len(), 1);

        // Apply to |0> and compare with Init kernel
        let zero = vec![C64::ONE, C64::ZERO];
        let decomp_result = apply_ops_to_sv(&zero, &ops, 1);
        let sv = Statevector::new_zero_state(1);
        let ref_result = Init.apply_sv(&sv).unwrap();
        for i in 0..2 {
            assert!((decomp_result[i].0 - ref_result.amplitudes()[i].0).abs() < 1e-10);
            assert!((decomp_result[i].1 - ref_result.amplitudes()[i].1).abs() < 1e-10);
        }
    }

    #[test]
    fn test_decompose_init_2q() {
        let wires = make_wires(2);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = decompose_init(&wires, &params).unwrap();
        assert_eq!(ops.len(), 2);

        let zero = vec![C64::ONE, C64::ZERO, C64::ZERO, C64::ZERO];
        let decomp_result = apply_ops_to_sv(&zero, &ops, 2);
        let sv = Statevector::new_zero_state(2);
        let ref_result = Init.apply_sv(&sv).unwrap();
        for i in 0..4 {
            assert!((decomp_result[i].0 - ref_result.amplitudes()[i].0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_decompose_init_3q() {
        let wires = make_wires(3);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = decompose_init(&wires, &params).unwrap();
        assert_eq!(ops.len(), 3);

        let zero = vec![C64::ONE, C64::ZERO, C64::ZERO, C64::ZERO,
                        C64::ZERO, C64::ZERO, C64::ZERO, C64::ZERO];
        let decomp_result = apply_ops_to_sv(&zero, &ops, 3);
        let sv = Statevector::new_zero_state(3);
        let ref_result = Init.apply_sv(&sv).unwrap();
        for i in 0..8 {
            assert!((decomp_result[i].0 - ref_result.amplitudes()[i].0).abs() < 1e-10);
        }
    }

    // =========================================================================
    // Entangle tests
    // =========================================================================

    #[test]
    fn test_decompose_entangle_2q() {
        let wires = make_wires(2);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = decompose_entangle(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&Entangle, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10));
    }

    #[test]
    fn test_decompose_entangle_3q() {
        let wires = make_wires(3);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = decompose_entangle(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 3);
        let ref_u = kernel_unitary(&Entangle, 3);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10));
    }

    #[test]
    fn test_decompose_entangle_1q_error() {
        let wires = make_wires(1);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        assert!(decompose_entangle(&wires, &params).is_err());
    }

    // =========================================================================
    // Fourier tests
    // =========================================================================

    #[test]
    fn test_decompose_fourier_1q() {
        let wires = make_wires(1);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = fourier::decompose_fourier(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 1);
        let ref_u = kernel_unitary(&Fourier, 1);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10));
    }

    #[test]
    fn test_decompose_fourier_2q() {
        let wires = make_wires(2);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = fourier::decompose_fourier(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&Fourier, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "QFT 2-qubit unitary mismatch");
    }

    #[test]
    fn test_decompose_fourier_3q() {
        let wires = make_wires(3);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = fourier::decompose_fourier(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 3);
        let ref_u = kernel_unitary(&Fourier, 3);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "QFT 3-qubit unitary mismatch");
    }

    #[test]
    fn test_decompose_fourier_4q() {
        let wires = make_wires(4);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = fourier::decompose_fourier(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 4);
        let ref_u = kernel_unitary(&Fourier, 4);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "QFT 4-qubit unitary mismatch");
    }

    // =========================================================================
    // FourierInv tests
    // =========================================================================

    #[test]
    fn test_decompose_fourier_inv_2q() {
        let wires = make_wires(2);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = fourier::decompose_fourier_inv(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&FourierInv, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "IQFT 2-qubit unitary mismatch");
    }

    #[test]
    fn test_decompose_fourier_inv_3q() {
        let wires = make_wires(3);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = fourier::decompose_fourier_inv(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 3);
        let ref_u = kernel_unitary(&FourierInv, 3);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "IQFT 3-qubit unitary mismatch");
    }

    // =========================================================================
    // Diffuse tests
    // =========================================================================

    #[test]
    fn test_decompose_diffuse_2q() {
        let wires = make_wires(2);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = grover::decompose_diffuse(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&Diffuse, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "Diffuse 2-qubit unitary mismatch");
    }

    #[test]
    fn test_decompose_diffuse_3q() {
        let wires = make_wires(3);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = grover::decompose_diffuse(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 3);
        let ref_u = kernel_unitary(&Diffuse, 3);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "Diffuse 3-qubit unitary mismatch");
    }

    // =========================================================================
    // Grover tests
    // =========================================================================

    #[test]
    fn test_decompose_grover_2q_target0() {
        let wires = make_wires(2);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] };
        let ops = grover::decompose_grover(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&GroverIter::single(0), 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "GroverIter 2q target=0 unitary mismatch");
    }

    #[test]
    fn test_decompose_grover_2q_target3() {
        let wires = make_wires(2);
        let params = KernelParams::Int { param0: 3, param1: 0, cmem_data: vec![] };
        let ops = grover::decompose_grover(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&GroverIter::single(3), 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "GroverIter 2q target=3 unitary mismatch");
    }

    #[test]
    fn test_decompose_grover_3q_multi() {
        let wires = make_wires(3);
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![2, 5] };
        let ops = grover::decompose_grover(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 3);
        let ref_u = kernel_unitary(&GroverIter::multi(vec![2, 5]), 3);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "GroverIter 3q multi-target unitary mismatch");
    }

    // =========================================================================
    // Rotate tests
    // =========================================================================

    #[test]
    fn test_decompose_rotate_2q() {
        let wires = make_wires(2);
        let params = KernelParams::Float { param0: 1.0, param1: 0.0 };
        let ops = rotation::decompose_rotate(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&Rotate { theta: 1.0 }, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "Rotate 2q theta=1.0 unitary mismatch");
    }

    #[test]
    fn test_decompose_rotate_3q() {
        use std::f64::consts::PI;
        let wires = make_wires(3);
        let params = KernelParams::Float { param0: PI, param1: 0.0 };
        let ops = rotation::decompose_rotate(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 3);
        let ref_u = kernel_unitary(&Rotate { theta: PI }, 3);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "Rotate 3q theta=pi unitary mismatch");
    }

    // =========================================================================
    // PhaseShift tests
    // =========================================================================

    #[test]
    fn test_decompose_phase_shift_2q() {
        let wires = make_wires(2);
        let params = KernelParams::Complex {
            param0: C64(1.0, 0.0),
            param1: C64::ZERO,
        };
        let ops = rotation::decompose_phase_shift(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&PhaseShift { amplitude: C64(1.0, 0.0) }, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "PhaseShift 2q amp=(1,0) unitary mismatch");
    }

    // =========================================================================
    // DiagonalUnitary tests
    // =========================================================================

    #[test]
    fn test_decompose_diagonal_2q() {
        use std::f64::consts::PI;
        use cqam_sim::kernels::diagonal::DiagonalUnitary;

        let wires = make_wires(2);
        // Diagonal with phases [0, pi/4, pi/2, 3pi/4]
        let diag = vec![
            C64::exp_i(0.0),
            C64::exp_i(PI / 4.0),
            C64::exp_i(PI / 2.0),
            C64::exp_i(3.0 * PI / 4.0),
        ];
        // Encode as cmem_data: pairs of f64-as-i64
        let mut cmem_data = Vec::new();
        for &d in &diag {
            cmem_data.push(d.0.to_bits() as i64);
            cmem_data.push(d.1.to_bits() as i64);
        }
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data };
        let ops = diagonal::decompose_diagonal_unitary(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let ref_u = kernel_unitary(&DiagonalUnitary { diagonal: diag }, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "DiagonalUnitary 2q unitary mismatch");
    }

    // =========================================================================
    // Permutation tests
    // =========================================================================

    #[test]
    fn test_decompose_permutation_1q() {
        use cqam_sim::kernels::permutation::Permutation;

        let wires = make_wires(1);
        let table = vec![1i64, 0];
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: table };
        let ops = permutation::decompose_permutation(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 1);
        let perm = Permutation::new(vec![1, 0]).unwrap();
        let ref_u = kernel_unitary(&perm, 1);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "Permutation 1q [1,0] unitary mismatch");
    }

    #[test]
    fn test_decompose_permutation_identity() {
        let wires = make_wires(2);
        let table = vec![0i64, 1, 2, 3];
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: table };
        let ops = permutation::decompose_permutation(&wires, &params).unwrap();
        assert!(ops.is_empty(), "Identity permutation should produce no gates");
    }

    #[test]
    fn test_decompose_permutation_2q() {
        use cqam_sim::kernels::permutation::Permutation;

        let wires = make_wires(2);
        // Swap states |01> and |10>: table = [0, 2, 1, 3]
        let table = vec![0i64, 2, 1, 3];
        let params = KernelParams::Int { param0: 0, param1: 0, cmem_data: table };
        let ops = permutation::decompose_permutation(&wires, &params).unwrap();

        let decomp_u = gate_sequence_unitary(&ops, 2);
        let perm = Permutation::new(vec![0, 2, 1, 3]).unwrap();
        let ref_u = kernel_unitary(&perm, 2);
        assert!(unitaries_equal_up_to_phase(&decomp_u, &ref_u, 1e-10),
            "Permutation 2q [0,2,1,3] unitary mismatch");
    }

    // =========================================================================
    // Passthrough tests
    // =========================================================================

    #[test]
    fn test_decompose_passthrough_h() {
        let mut mp = circuit_ir::MicroProgram::new(1);
        mp.push(Op::Gate1q(ApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));
        let result = decompose_to_standard(&mp).unwrap();
        assert_eq!(result.ops.len(), 1);
    }

    #[test]
    fn test_decompose_passthrough_cx() {
        let mut mp = circuit_ir::MicroProgram::new(2);
        mp.push(Op::Gate2q(ApplyGate2q {
            wire_a: QWire(0),
            wire_b: QWire(1),
            gate: Gate2q::Cx,
        }));
        let result = decompose_to_standard(&mp).unwrap();
        assert_eq!(result.ops.len(), 1);
    }

    #[test]
    fn test_decompose_custom_unitary_1q_succeeds() {
        // A 1-qubit CustomUnitary is now ZYZ-decomposed to a U3 gate.
        let mut mp = circuit_ir::MicroProgram::new(1);
        mp.push(Op::CustomUnitary {
            wires: vec![QWire(0)],
            matrix: vec![C64::ONE, C64::ZERO, C64::ZERO, C64::ONE],
        });
        let result = decompose_to_standard(&mp).unwrap();
        assert_eq!(result.ops.len(), 1);
        assert!(matches!(&result.ops[0], Op::Gate1q(g) if matches!(g.gate, Gate1q::U3(_, _, _))));
    }

    #[test]
    fn test_decompose_custom_unitary_2q_error() {
        // Multi-qubit CustomUnitary is still unsupported (no KAK decomposition yet).
        let mut mp = circuit_ir::MicroProgram::new(2);
        mp.push(Op::CustomUnitary {
            wires: vec![QWire(0), QWire(1)],
            matrix: vec![C64::ONE; 16],
        });
        assert!(decompose_to_standard(&mp).is_err());
    }

    #[test]
    fn test_decompose_custom_unitary_1q_rx_roundtrip() {
        use std::f64::consts::PI;
        // Rx(1.0) as a CustomUnitary should decompose to U3 with correct parameters.
        let angle = 1.0_f64;
        let c = (angle / 2.0).cos();
        let s = (angle / 2.0).sin();
        let rx_mat = vec![C64(c, 0.0), C64(0.0, -s), C64(0.0, -s), C64(c, 0.0)];

        let mut mp = circuit_ir::MicroProgram::new(1);
        mp.push(Op::CustomUnitary {
            wires: vec![QWire(0)],
            matrix: rx_mat.clone(),
        });
        let result = decompose_to_standard(&mp).unwrap();
        assert_eq!(result.ops.len(), 1);

        // Extract U3 parameters and verify the reconstructed matrix matches Rx(1.0)
        if let Op::Gate1q(g) = &result.ops[0] {
            if let Gate1q::U3(theta_p, phi_p, lambda_p) = &g.gate {
                let t = theta_p.value().unwrap();
                let p = phi_p.value().unwrap();
                let l = lambda_p.value().unwrap();
                // Reconstruct U3 matrix
                let ct = (t / 2.0).cos();
                let st = (t / 2.0).sin();
                let el = C64::exp_i(l);
                let ep = C64::exp_i(p);
                let epl = C64::exp_i(p + l);
                let u3 = vec![
                    C64(ct, 0.0),
                    C64(-el.0 * st, -el.1 * st),
                    C64(ep.0 * st, ep.1 * st),
                    C64(epl.0 * ct, epl.1 * ct),
                ];
                // Compare up to global phase: find phase from first nonzero pair
                let mut phase = C64::ONE;
                let mut found = false;
                for i in 0..4 {
                    let rx_norm = (rx_mat[i].0 * rx_mat[i].0 + rx_mat[i].1 * rx_mat[i].1).sqrt();
                    let u3_norm = (u3[i].0 * u3[i].0 + u3[i].1 * u3[i].1).sqrt();
                    if rx_norm > 1e-10 && u3_norm > 1e-10 {
                        let conj = C64(rx_mat[i].0, -rx_mat[i].1);
                        let num = C64(
                            u3[i].0 * conj.0 - u3[i].1 * conj.1,
                            u3[i].0 * conj.1 + u3[i].1 * conj.0,
                        );
                        let d = rx_mat[i].0 * rx_mat[i].0 + rx_mat[i].1 * rx_mat[i].1;
                        phase = C64(num.0 / d, num.1 / d);
                        found = true;
                        break;
                    }
                }
                assert!(found);
                let mut frob_sq = 0.0;
                for i in 0..4 {
                    let pa = C64(phase.0 * rx_mat[i].0 - phase.1 * rx_mat[i].1,
                                  phase.0 * rx_mat[i].1 + phase.1 * rx_mat[i].0);
                    let d = C64(pa.0 - u3[i].0, pa.1 - u3[i].1);
                    frob_sq += d.0 * d.0 + d.1 * d.1;
                }
                assert!(
                    frob_sq.sqrt() < 1e-9,
                    "Rx(1.0) CustomUnitary -> U3 reconstruction error too large: {}",
                    frob_sq.sqrt()
                );
            } else {
                panic!("Expected U3 gate, got {:?}", g.gate);
            }
        } else {
            panic!("Expected Gate1q op");
        }
        let _ = PI; // suppress unused warning
    }

    // =========================================================================
    // Prep distribution decomposition tests
    // =========================================================================

    #[test]
    fn test_prep_zero_no_gates() {
        let mut mp = circuit_ir::MicroProgram::new(2);
        mp.push(Op::Prep(circuit_ir::Prepare {
            wires: make_wires(2),
            dist: DistId::Zero,
        }));
        let result = decompose_to_standard(&mp).unwrap();
        // Should only have the Prep op, no extra gates
        assert_eq!(result.ops.len(), 1);
        assert!(matches!(&result.ops[0], Op::Prep(_)));
    }

    #[test]
    fn test_prep_uniform_produces_hadamards() {
        let n = 3;
        let mut mp = circuit_ir::MicroProgram::new(n);
        mp.push(Op::Prep(circuit_ir::Prepare {
            wires: make_wires(n as usize),
            dist: DistId::Uniform,
        }));
        let result = decompose_to_standard(&mp).unwrap();
        // Prep + n Hadamard gates
        assert_eq!(result.ops.len(), 1 + n as usize);
        // Verify the statevector is uniform
        let dim = 1 << n;
        let zero = vec![C64::ZERO; dim];
        let mut sv = zero.clone();
        sv[0] = C64::ONE;
        let sv = apply_ops_to_sv(&sv, &result.ops, n as u8);
        let expected = 1.0 / (dim as f64).sqrt();
        for amp in &sv {
            assert!((amp.0 - expected).abs() < 1e-10, "expected uniform amplitude {expected}, got {}", amp.0);
            assert!(amp.1.abs() < 1e-10);
        }
    }

    #[test]
    fn test_prep_bell_produces_bell_state() {
        let mut mp = circuit_ir::MicroProgram::new(2);
        mp.push(Op::Prep(circuit_ir::Prepare {
            wires: make_wires(2),
            dist: DistId::Bell,
        }));
        let result = decompose_to_standard(&mp).unwrap();
        // Prep + H + CX = 3 ops
        assert_eq!(result.ops.len(), 3);
        let sv = apply_ops_to_sv(&[C64::ONE, C64::ZERO, C64::ZERO, C64::ZERO], &result.ops, 2);
        let h = std::f64::consts::FRAC_1_SQRT_2;
        // Bell state: (|00> + |11>) / sqrt(2)
        assert!((sv[0].0 - h).abs() < 1e-10, "|00> amplitude wrong");
        assert!(sv[1].norm_sq() < 1e-10, "|01> should be zero");
        assert!(sv[2].norm_sq() < 1e-10, "|10> should be zero");
        assert!((sv[3].0 - h).abs() < 1e-10, "|11> amplitude wrong");
    }

    #[test]
    fn test_prep_ghz_3q() {
        let mut mp = circuit_ir::MicroProgram::new(3);
        mp.push(Op::Prep(circuit_ir::Prepare {
            wires: make_wires(3),
            dist: DistId::Ghz,
        }));
        let result = decompose_to_standard(&mp).unwrap();
        // Prep + H + 2 CX = 4 ops
        assert_eq!(result.ops.len(), 4);
        let mut sv = vec![C64::ZERO; 8];
        sv[0] = C64::ONE;
        let sv = apply_ops_to_sv(&sv, &result.ops, 3);
        let h = std::f64::consts::FRAC_1_SQRT_2;
        // GHZ: (|000> + |111>) / sqrt(2)
        assert!((sv[0].0 - h).abs() < 1e-10, "|000> amplitude wrong");
        for i in 1..7 {
            assert!(sv[i].norm_sq() < 1e-10, "|{i:03b}> should be zero");
        }
        assert!((sv[7].0 - h).abs() < 1e-10, "|111> amplitude wrong");
    }
}
