//! Neutral atom noise model (QuEra/Harvard-MIT Rydberg-class).

use serde::Deserialize;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use super::{NoiseModel, channels};

/// Noise parameters for neutral atom (Rydberg) qubits.
#[derive(Deserialize)]
#[serde(default)]
pub struct NeutralAtomNoise {
    /// T1 (trap lifetime) in seconds.
    pub t1: f64,
    /// T2 dephasing time in seconds.
    pub t2: f64,
    /// Rydberg two-qubit gate error probability.
    pub rydberg_error: f64,
    /// Rydberg gate duration in seconds.
    pub rydberg_gate_time: f64,
    /// Single-qubit gate error probability.
    pub single_gate_error: f64,
    /// Single-qubit gate duration in seconds.
    pub single_gate_time: f64,
    /// Atom loss probability per gate cycle.
    pub atom_loss_rate: f64,
    /// Position disorder: standard deviation of gate fidelity variation.
    pub position_disorder: f64,
    /// Readout error probability.
    pub readout_error: f64,
    /// Readout duration in seconds.
    pub readout_time: f64,
}

impl Default for NeutralAtomNoise {
    fn default() -> Self {
        Self {
            t1: 4.0,
            t2: 10e-3,
            rydberg_error: 5e-3,
            rydberg_gate_time: 0.5e-6,
            single_gate_error: 3e-3,
            single_gate_time: 5e-6,
            atom_loss_rate: 3e-3,
            position_disorder: 1e-3,
            readout_error: 0.01,
            readout_time: 5e-3,
        }
    }
}

impl NoiseModel for NeutralAtomNoise {
    fn post_single_gate(
        &self,
        state: &mut DensityMatrix,
        target_qubit: u8,
        gate_time: f64,
    ) {
        // Thermal relaxation (dominated by short T2 from light shifts)
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, gate_time, 0.0
        );
        state.apply_single_qubit_channel(target_qubit, &kraus);

        // Gate error + position disorder (mean error in DM mode)
        let total_error = (self.single_gate_error + self.position_disorder).min(1.0);
        if total_error > 0.0 {
            let kraus = channels::depolarizing_single(total_error);
            state.apply_single_qubit_channel(target_qubit, &kraus);
        }

        // Atom loss modeled as amplitude damping
        if self.atom_loss_rate > 0.0 {
            let kraus = channels::amplitude_damping(self.atom_loss_rate);
            state.apply_single_qubit_channel(target_qubit, &kraus);
        }
    }

    fn post_two_qubit_gate(
        &self,
        state: &mut DensityMatrix,
        qubit_a: u8,
        qubit_b: u8,
        gate_time: f64,
    ) {
        // Thermal relaxation on both qubits
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, gate_time, 0.0
        );
        state.apply_single_qubit_channel(qubit_a, &kraus);
        state.apply_single_qubit_channel(qubit_b, &kraus);

        // Rydberg gate depolarizing
        if self.rydberg_error > 0.0 {
            let kraus = channels::depolarizing_two_qubit(self.rydberg_error);
            state.apply_two_qubit_channel(qubit_a, qubit_b, &kraus);
        }

        // Atom loss on both qubits
        if self.atom_loss_rate > 0.0 {
            let kraus = channels::amplitude_damping(self.atom_loss_rate);
            state.apply_single_qubit_channel(qubit_a, &kraus);
            state.apply_single_qubit_channel(qubit_b, &kraus);
        }
    }

    fn idle_noise(
        &self,
        state: &mut DensityMatrix,
        duration: f64,
    ) {
        // Short T2 from differential light shifts -> significant idle dephasing
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, duration, 0.0
        );
        for q in 0..state.num_qubits() {
            state.apply_single_qubit_channel(q, &kraus);
        }
    }

    fn readout_noise(
        &self,
        probabilities: &mut [f64],
        _qubit: u8,
    ) {
        channels::apply_readout_confusion(
            probabilities,
            self.readout_error,
            self.readout_error,
        );
    }

    fn prep_noise(&self, _state: &mut DensityMatrix) {
        // No thermal population for neutral atoms (optical pumping)
    }

    fn name(&self) -> &str { "neutral-atom" }

    fn has_readout_noise(&self) -> bool {
        self.readout_error > 0.0
    }

    fn single_gate_time(&self) -> f64 { self.single_gate_time }
    fn two_gate_time(&self) -> f64 { self.rydberg_gate_time }

    fn kraus_single_gate(
        &self,
        _target_qubit: u8,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        let mut result = vec![
            channels::thermal_relaxation(self.t1, self.t2, gate_time, 0.0),
        ];
        let total_error = (self.single_gate_error + self.position_disorder).min(1.0);
        if total_error > 0.0 {
            result.push(channels::depolarizing_single(total_error));
        }
        if self.atom_loss_rate > 0.0 {
            result.push(channels::amplitude_damping(self.atom_loss_rate));
        }
        result
    }

    fn kraus_two_qubit_gate(
        &self,
        _qubit_a: u8,
        _qubit_b: u8,
        _gate_time: f64,
    ) -> Vec<Vec<[C64; 16]>> {
        if self.rydberg_error > 0.0 {
            vec![channels::depolarizing_two_qubit(self.rydberg_error)]
        } else {
            vec![]
        }
    }

    fn kraus_two_qubit_per_qubit(
        &self,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        let mut result = vec![
            channels::thermal_relaxation(self.t1, self.t2, gate_time, 0.0),
        ];
        if self.atom_loss_rate > 0.0 {
            result.push(channels::amplitude_damping(self.atom_loss_rate));
        }
        result
    }

    fn kraus_idle(&self, duration: f64) -> Vec<[C64; 4]> {
        channels::thermal_relaxation(self.t1, self.t2, duration, 0.0)
    }
}
