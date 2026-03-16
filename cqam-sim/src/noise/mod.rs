//! Noise model framework for quantum simulation.
//!
//! Provides the `NoiseModel` trait, Kraus channel library, trajectory
//! sampling, and five modality-specific noise profiles.

pub mod channels;
pub mod trajectory;
pub mod superconducting;
pub mod trapped_ion;
pub mod neutral_atom;
pub mod photonic;
pub mod spin;

use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use rand_chacha::ChaCha8Rng;

pub use superconducting::SuperconductingNoise;
pub use trapped_ion::TrappedIonNoise;
pub use neutral_atom::NeutralAtomNoise;
pub use photonic::PhotonicNoise;
pub use spin::SpinQubitNoise;

/// Noise simulation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseMethod {
    /// Deterministic Kraus on density matrix. O(4^n).
    DensityMatrix,
    /// Stochastic trajectory on statevector. O(2^n) per shot.
    Trajectory,
}

/// Backend-agnostic noise model interface.
///
/// Implementations must be Send + Sync to support shot-mode parallelism.
/// All methods receive the state by mutable reference and modify it
/// in-place. The `gate_time` parameter is in seconds (SI units).
pub trait NoiseModel: Send + Sync {
    /// Apply noise after a single-qubit gate on `target_qubit`.
    fn post_single_gate(
        &self,
        state: &mut DensityMatrix,
        target_qubit: u8,
        gate_time: f64,
    );

    /// Apply noise after a two-qubit gate on `(qubit_a, qubit_b)`.
    fn post_two_qubit_gate(
        &self,
        state: &mut DensityMatrix,
        qubit_a: u8,
        qubit_b: u8,
        gate_time: f64,
    );

    /// Apply idle decoherence for `duration` seconds on all qubits.
    fn idle_noise(
        &self,
        state: &mut DensityMatrix,
        duration: f64,
    );

    /// Apply readout noise to measurement probabilities.
    fn readout_noise(
        &self,
        probabilities: &mut [f64],
        qubit: u8,
    );

    /// Apply state preparation errors.
    fn prep_noise(
        &self,
        state: &mut DensityMatrix,
    );

    /// Apply noise after a full-register kernel application.
    fn post_kernel(
        &self,
        state: &mut DensityMatrix,
        _kernel_gate_count: usize,
        _kernel_time: f64,
    ) {
        let _ = state;
    }

    /// Return the name of this noise model.
    fn name(&self) -> &str;

    /// Whether this noise model has any readout noise.
    fn has_readout_noise(&self) -> bool;

    /// Typical single-qubit gate time in seconds.
    fn single_gate_time(&self) -> f64;

    /// Typical two-qubit gate time in seconds.
    fn two_gate_time(&self) -> f64;

    /// Return the Kraus operators for single-qubit post-gate noise.
    /// Each inner Vec is a channel to be applied in sequence.
    fn kraus_single_gate(
        &self,
        target_qubit: u8,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>>;

    /// Return the Kraus operators for two-qubit post-gate noise.
    fn kraus_two_qubit_gate(
        &self,
        qubit_a: u8,
        qubit_b: u8,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 16]>>;

    /// Return per-qubit single-qubit Kraus channels to apply to each qubit
    /// after a two-qubit gate (e.g., thermal relaxation, atom loss).
    /// Each inner Vec is one channel; all channels are applied sequentially
    /// to **both** qubits. Default returns nothing.
    fn kraus_two_qubit_per_qubit(
        &self,
        _gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        vec![]
    }

    /// Return the Kraus operators for idle noise on a single qubit.
    fn kraus_idle(&self, duration: f64) -> Vec<[C64; 4]>;

    /// Apply stochastic noise to a statevector after a single-qubit gate.
    /// Returns `true` if a quantum jump occurred.
    fn trajectory_single_gate(
        &self,
        amplitudes: &mut [C64],
        num_qubits: u8,
        target_qubit: u8,
        gate_time: f64,
        rng: &mut ChaCha8Rng,
    ) -> bool {
        let channel_sets = self.kraus_single_gate(target_qubit, gate_time);
        let mut jumped = false;
        for channel in &channel_sets {
            if trajectory::apply_kraus_to_statevector_single(
                amplitudes, num_qubits, target_qubit, channel, rng,
            ) {
                jumped = true;
            }
        }
        jumped
    }

    /// Apply stochastic noise to a statevector after a two-qubit gate.
    /// Returns `true` if a quantum jump occurred.
    fn trajectory_two_qubit_gate(
        &self,
        amplitudes: &mut [C64],
        num_qubits: u8,
        qubit_a: u8,
        qubit_b: u8,
        gate_time: f64,
        rng: &mut ChaCha8Rng,
    ) -> bool {
        let mut jumped = false;

        // Per-qubit channels (thermal relaxation, atom loss, etc.)
        let per_qubit = self.kraus_two_qubit_per_qubit(gate_time);
        for channel in &per_qubit {
            for &q in &[qubit_a, qubit_b] {
                if trajectory::apply_kraus_to_statevector_single(
                    amplitudes, num_qubits, q, channel, rng,
                ) {
                    jumped = true;
                }
            }
        }

        // Two-qubit channels (depolarizing, etc.)
        let channel_sets = self.kraus_two_qubit_gate(qubit_a, qubit_b, gate_time);
        for channel in &channel_sets {
            if trajectory::apply_kraus_to_statevector_two(
                amplitudes, num_qubits, qubit_a, qubit_b, channel, rng,
            ) {
                jumped = true;
            }
        }
        jumped
    }

    /// Apply stochastic idle noise to all qubits in a statevector.
    fn trajectory_idle(
        &self,
        amplitudes: &mut [C64],
        num_qubits: u8,
        duration: f64,
        rng: &mut ChaCha8Rng,
    ) {
        let kraus_ops = self.kraus_idle(duration);
        for q in 0..num_qubits {
            trajectory::apply_kraus_to_statevector_single(
                amplitudes, num_qubits, q, &kraus_ops, rng,
            );
        }
    }
}
