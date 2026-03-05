//! Tests for all quantum kernel implementations (Init, Entangle, Fourier,
//! Diffuse, GroverIter) operating on `DensityMatrix`.

use cqam_sim::density_matrix::DensityMatrix;
use cqam_sim::kernels::init::Init;
use cqam_sim::kernels::entangle::Entangle;
use cqam_sim::kernels::fourier::Fourier;
use cqam_sim::kernels::diffuse::Diffuse;
use cqam_sim::kernels::grover::GroverIter;
use cqam_sim::kernels::rotate::Rotate;
use cqam_sim::kernels::phase::PhaseShift;
use cqam_sim::kernel::Kernel;

// =============================================================================
// Init kernel tests
// =============================================================================

#[test]
fn test_init_returns_uniform() {
    let init = Init;
    let input = DensityMatrix::new_zero_state(2);
    let output = init.apply(&input);

    // All diagonal entries should be 0.25
    let probs = output.diagonal_probabilities();
    for &p in &probs {
        assert!((p - 0.25).abs() < 1e-10, "Init should produce uniform, got p={}", p);
    }
    assert!((output.purity() - 1.0).abs() < 1e-10, "Init should produce pure state");
}

// =============================================================================
// Fourier kernel tests
// =============================================================================

#[test]
fn test_fourier_zero_to_uniform() {
    // QFT on |0><0| should produce uniform superposition
    let input = DensityMatrix::new_zero_state(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    let probs = output.diagonal_probabilities();
    for &p in &probs {
        assert!(
            (p - 0.25).abs() < 1e-10,
            "QFT of |0> should be uniform, got p={}",
            p
        );
    }
}

#[test]
fn test_fourier_uniform_to_zero() {
    // QFT on uniform pure state should produce |0><0|
    let input = DensityMatrix::new_uniform(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    let probs = output.diagonal_probabilities();
    assert!(
        probs[0] > 0.99,
        "QFT of uniform should concentrate on state 0, got p[0]={}",
        probs[0]
    );
}

#[test]
fn test_fourier_preserves_purity() {
    let input = DensityMatrix::new_zero_state(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    assert!(
        (output.purity() - 1.0).abs() < 1e-10,
        "QFT should preserve purity, got {}",
        output.purity()
    );
}

#[test]
fn test_fourier_preserves_trace() {
    let input = DensityMatrix::new_zero_state(2);
    let fourier = Fourier;
    let output = fourier.apply(&input);

    let tr = output.trace();
    assert!(
        (tr.0 - 1.0).abs() < 1e-10,
        "QFT should preserve trace, got ({}, {})",
        tr.0, tr.1
    );
}

// =============================================================================
// Diffuse kernel tests
// =============================================================================

#[test]
fn test_diffuse_on_uniform_is_identity() {
    // Diffusion on uniform superposition should keep it unchanged
    let input = DensityMatrix::new_uniform(2);
    let diffuse = Diffuse;
    let output = diffuse.apply(&input);

    let probs = output.diagonal_probabilities();
    for &p in &probs {
        assert!(
            (p - 0.25).abs() < 1e-10,
            "Diffuse on uniform should stay uniform, got p={}",
            p
        );
    }
}

#[test]
fn test_diffuse_preserves_purity() {
    let input = DensityMatrix::new_zero_state(2);
    let diffuse = Diffuse;
    let output = diffuse.apply(&input);

    assert!(
        (output.purity() - 1.0).abs() < 1e-10,
        "Diffuse should preserve purity, got {}",
        output.purity()
    );
}

// =============================================================================
// GroverIter kernel tests
// =============================================================================

#[test]
fn test_grover_2q_target3_exact() {
    // Key verification: 1 iteration on N=4, target=3 -> probability 1.0
    let input = DensityMatrix::new_uniform(2);
    let grover = GroverIter { target: 3 };
    let output = grover.apply(&input);

    let probs = output.diagonal_probabilities();
    assert!(
        (probs[3] - 1.0).abs() < 1e-10,
        "Grover 2q target=3: expected p[3]=1.0, got {}",
        probs[3]
    );
}

#[test]
fn test_grover_amplifies_target() {
    let input = DensityMatrix::new_uniform(2);
    let grover = GroverIter { target: 2 };
    let output = grover.apply(&input);

    let probs = output.diagonal_probabilities();
    assert!(
        probs[2] > probs[0],
        "Grover should amplify target. target_p={}, other_p={}",
        probs[2], probs[0]
    );
}

#[test]
fn test_grover_preserves_normalization() {
    let input = DensityMatrix::new_uniform(2);
    let grover = GroverIter { target: 1 };
    let output = grover.apply(&input);

    let tr = output.trace();
    assert!(
        (tr.0 - 1.0).abs() < 1e-10,
        "Grover output should have trace 1, got ({}, {})",
        tr.0, tr.1
    );
}

#[test]
fn test_grover_4q_3_iterations() {
    // 3 iterations on 16 states should give high probability for the target
    let mut dm = DensityMatrix::new_uniform(4);
    let grover = GroverIter { target: 7 };

    for _ in 0..3 {
        dm = grover.apply(&dm);
    }

    let probs = dm.diagonal_probabilities();
    assert!(
        probs[7] > 0.9,
        "After 3 Grover iterations on 4-qubit, p[7]={} should be > 0.9",
        probs[7]
    );
}

// =============================================================================
// Entangle kernel tests
// =============================================================================

#[test]
fn test_entangle_creates_bell() {
    // Start with |+>|0> = H|0> tensor |0>
    // Apply CNOT -> Bell state
    // First create |+> tensor |0> as a statevector
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    // |+>|0> = (1/sqrt(2))(|00> + |10>)
    let psi = vec![
        (inv_sqrt2, 0.0), // |00>
        (0.0, 0.0),       // |01>
        (inv_sqrt2, 0.0), // |10>
        (0.0, 0.0),       // |11>
    ];
    let input = DensityMatrix::from_statevector(&psi).unwrap();

    let entangle = Entangle;
    let output = entangle.apply(&input);

    // Should produce Bell state: rho[0][0] = rho[0][3] = rho[3][0] = rho[3][3] = 0.5
    assert!(
        (output.get(0, 0).0 - 0.5).abs() < 1e-10,
        "rho[0][0] should be 0.5, got {}",
        output.get(0, 0).0
    );
    assert!(
        (output.get(0, 3).0 - 0.5).abs() < 1e-10,
        "rho[0][3] should be 0.5, got {}",
        output.get(0, 3).0
    );
    assert!(
        (output.get(3, 0).0 - 0.5).abs() < 1e-10,
        "rho[3][0] should be 0.5, got {}",
        output.get(3, 0).0
    );
    assert!(
        (output.get(3, 3).0 - 0.5).abs() < 1e-10,
        "rho[3][3] should be 0.5, got {}",
        output.get(3, 3).0
    );
}

// =============================================================================
// Rotate kernel tests
// =============================================================================

#[test]
fn test_rotate_kernel_identity() {
    // theta=0 -> U=I, so output should equal input
    let input = DensityMatrix::new_uniform(2);
    let rotate = Rotate { theta: 0.0 };
    let output = rotate.apply(&input);

    let probs_in = input.diagonal_probabilities();
    let probs_out = output.diagonal_probabilities();
    for (i, (&pi, &po)) in probs_in.iter().zip(probs_out.iter()).enumerate() {
        assert!(
            (pi - po).abs() < 1e-10,
            "Rotate(0) should be identity: p_in[{}]={}, p_out[{}]={}",
            i, pi, i, po
        );
    }
    assert!(
        (output.purity() - input.purity()).abs() < 1e-10,
        "Rotate(0) should preserve purity"
    );
}

#[test]
fn test_rotate_kernel_pi() {
    // theta=PI -> phases flip; diagonal probabilities preserved (diagonal unitary)
    let input = DensityMatrix::new_uniform(2);
    let rotate = Rotate { theta: std::f64::consts::PI };
    let output = rotate.apply(&input);

    let probs_in = input.diagonal_probabilities();
    let probs_out = output.diagonal_probabilities();
    for (i, (&pi, &po)) in probs_in.iter().zip(probs_out.iter()).enumerate() {
        assert!(
            (pi - po).abs() < 1e-10,
            "Rotate(PI) should preserve diagonal probs: p_in[{}]={}, p_out[{}]={}",
            i, pi, i, po
        );
    }
    assert!(
        (output.purity() - 1.0).abs() < 1e-10,
        "Rotate(PI) should preserve purity of pure state"
    );
}

#[test]
fn test_rotate_kernel_preserves_trace() {
    let input = DensityMatrix::new_zero_state(2);
    let rotate = Rotate { theta: 1.234 };
    let output = rotate.apply(&input);

    let tr = output.trace();
    assert!(
        (tr.0 - 1.0).abs() < 1e-10,
        "Rotate should preserve trace, got ({}, {})",
        tr.0, tr.1
    );
}

// =============================================================================
// PhaseShift kernel tests
// =============================================================================

#[test]
fn test_phase_shift_kernel_zero() {
    // amplitude=(0,0) -> |z|=0 -> U=I
    let input = DensityMatrix::new_uniform(2);
    let ps = PhaseShift { amplitude: (0.0, 0.0) };
    let output = ps.apply(&input);

    let probs_in = input.diagonal_probabilities();
    let probs_out = output.diagonal_probabilities();
    for (i, (&pi, &po)) in probs_in.iter().zip(probs_out.iter()).enumerate() {
        assert!(
            (pi - po).abs() < 1e-10,
            "PhaseShift(0,0) should be identity: p[{}] in={}, out={}",
            i, pi, po
        );
    }
}

#[test]
fn test_phase_shift_kernel_real() {
    // amplitude=(1.0, 0.0) -> |z|=1.0 -> same as Rotate(1.0)
    let input = DensityMatrix::new_uniform(2);
    let ps = PhaseShift { amplitude: (1.0, 0.0) };
    let rotate = Rotate { theta: 1.0 };

    let output_ps = ps.apply(&input);
    let output_rot = rotate.apply(&input);

    let dim = input.dimension();
    for i in 0..dim {
        for j in 0..dim {
            let (re_ps, im_ps) = output_ps.get(i, j);
            let (re_rot, im_rot) = output_rot.get(i, j);
            assert!(
                (re_ps - re_rot).abs() < 1e-10 && (im_ps - im_rot).abs() < 1e-10,
                "PhaseShift(1.0, 0.0) should equal Rotate(1.0) at [{},{}]: ({},{}) vs ({},{})",
                i, j, re_ps, im_ps, re_rot, im_rot
            );
        }
    }
}

#[test]
fn test_phase_shift_kernel_preserves_unitarity() {
    // Purity should be preserved for any amplitude
    let input = DensityMatrix::new_uniform(2);
    let ps = PhaseShift { amplitude: (0.7, 0.3) };
    let output = ps.apply(&input);

    assert!(
        (output.purity() - 1.0).abs() < 1e-10,
        "PhaseShift should preserve purity of pure state, got {}",
        output.purity()
    );
}

// =============================================================================
// Rotate kernel: negative theta
// =============================================================================

#[test]
fn test_rotate_kernel_negative_theta() {
    // Negative theta should produce the conjugate of the positive-theta unitary.
    // Diagonal probabilities must still be preserved.
    let input = DensityMatrix::new_uniform(2);
    let rotate_pos = Rotate { theta: 1.5 };
    let rotate_neg = Rotate { theta: -1.5 };

    let out_pos = rotate_pos.apply(&input);
    let out_neg = rotate_neg.apply(&input);

    // Both should preserve diagonal probabilities identically
    let probs_pos = out_pos.diagonal_probabilities();
    let probs_neg = out_neg.diagonal_probabilities();
    for (i, (&pp, &pn)) in probs_pos.iter().zip(probs_neg.iter()).enumerate() {
        assert!(
            (pp - pn).abs() < 1e-10,
            "Rotate(+theta) and Rotate(-theta) should give same diagonal probs: p[{}] pos={}, neg={}",
            i, pp, pn
        );
    }

    // Purity preserved
    assert!(
        (out_neg.purity() - 1.0).abs() < 1e-10,
        "Rotate(-theta) should preserve purity, got {}", out_neg.purity()
    );

    // Trace preserved
    let tr = out_neg.trace();
    assert!(
        (tr.0 - 1.0).abs() < 1e-10,
        "Rotate(-theta) should preserve trace, got ({}, {})", tr.0, tr.1
    );
}

// =============================================================================
// Rotate kernel: zero state diagonal preservation
// =============================================================================

#[test]
fn test_rotate_preserves_zero_state_diagonal() {
    // A diagonal unitary applied to |0><0| should leave diagonal unchanged:
    // rho' = U|0><0|U^dagger, and since |0><0| has all weight on (0,0),
    // rho'[0][0] = |U[0][0]|^2 * 1.0 = 1.0 (since |exp(i*0)|^2 = 1).
    let input = DensityMatrix::new_zero_state(2);
    let rotate = Rotate { theta: 2.718 }; // arbitrary angle
    let output = rotate.apply(&input);

    let probs = output.diagonal_probabilities();
    assert!(
        (probs[0] - 1.0).abs() < 1e-10,
        "Rotate on |0><0| should keep p[0]=1.0, got {}", probs[0]
    );
    for k in 1..probs.len() {
        assert!(
            probs[k].abs() < 1e-10,
            "Rotate on |0><0| should keep p[{}]=0.0, got {}", k, probs[k]
        );
    }
}

// =============================================================================
// Rotate kernel: very large theta (wraps around 2*PI)
// =============================================================================

#[test]
fn test_rotate_large_theta_wraps() {
    // theta and theta + 2*PI*N should produce identical density matrices
    let input = DensityMatrix::new_uniform(2);
    let theta = 1.234;
    let rotate_small = Rotate { theta };
    let rotate_large = Rotate { theta: theta + 2.0 * std::f64::consts::PI * 100.0 };

    let out_small = rotate_small.apply(&input);
    let out_large = rotate_large.apply(&input);

    // Note: for the diagonal, both agree exactly. For off-diagonal, the phases
    // are exp(i*(theta_large - theta_small)*k) for different k, which should be
    // close to 1 for all k since U[k]=exp(i*theta*k) and exp wraps at 2*PI.
    // However, because k ranges 0..dim, the additional 2*PI*N*k wraps exactly.
    let dim = input.dimension();
    for i in 0..dim {
        for j in 0..dim {
            let (re_s, im_s) = out_small.get(i, j);
            let (re_l, im_l) = out_large.get(i, j);
            assert!(
                (re_s - re_l).abs() < 1e-6 && (im_s - im_l).abs() < 1e-6,
                "Rotate({}) and Rotate({}) should agree at [{},{}]: ({},{}) vs ({},{})",
                theta, theta + 200.0 * std::f64::consts::PI,
                i, j, re_s, im_s, re_l, im_l
            );
        }
    }
}

// =============================================================================
// PhaseShift kernel: purely imaginary amplitude
// =============================================================================

#[test]
fn test_phase_shift_purely_imaginary() {
    // amplitude = (0, 3.0) -> |z| = 3.0 -> same as Rotate(3.0)
    let input = DensityMatrix::new_uniform(2);
    let ps = PhaseShift { amplitude: (0.0, 3.0) };
    let rotate = Rotate { theta: 3.0 };

    let out_ps = ps.apply(&input);
    let out_rot = rotate.apply(&input);

    let dim = input.dimension();
    for i in 0..dim {
        for j in 0..dim {
            let (re_ps, im_ps) = out_ps.get(i, j);
            let (re_rot, im_rot) = out_rot.get(i, j);
            assert!(
                (re_ps - re_rot).abs() < 1e-10 && (im_ps - im_rot).abs() < 1e-10,
                "PhaseShift(0, 3.0) should equal Rotate(3.0) at [{},{}]: ({},{}) vs ({},{})",
                i, j, re_ps, im_ps, re_rot, im_rot
            );
        }
    }

    // Purity preserved
    assert!(
        (out_ps.purity() - 1.0).abs() < 1e-10,
        "PhaseShift with purely imaginary amplitude should preserve purity"
    );
}

// =============================================================================
// PhaseShift kernel: trace preservation with complex amplitude
// =============================================================================

#[test]
fn test_phase_shift_preserves_trace() {
    let input = DensityMatrix::new_zero_state(2);
    let ps = PhaseShift { amplitude: (2.0, -1.5) };
    let output = ps.apply(&input);

    let tr = output.trace();
    assert!(
        (tr.0 - 1.0).abs() < 1e-10,
        "PhaseShift should preserve trace, got ({}, {})", tr.0, tr.1
    );
}
