//! Trapped ion noise model (Quantinuum H2-class).

use serde::Deserialize;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use super::{NoiseModel, channels};

/// Noise parameters for trapped ion qubits.
#[derive(Deserialize)]
#[serde(default)]
pub struct TrappedIonNoise {
    /// T1 relaxation time in seconds.
    pub t1: f64,
    /// T2 dephasing time in seconds.
    pub t2: f64,
    /// Single-qubit gate error probability (depolarizing).
    pub single_gate_error: f64,
    /// Single-qubit gate duration in seconds.
    pub single_gate_time: f64,
    /// Molmer-Sorensen two-qubit gate error probability.
    pub ms_gate_error: f64,
    /// Molmer-Sorensen gate duration in seconds.
    pub ms_gate_time: f64,
    /// Motional heating rate (quanta per second).
    pub heating_rate: f64,
    /// Readout error probability (symmetric).
    pub readout_error: f64,
    /// Readout duration in seconds.
    pub readout_time: f64,
    /// Shuttling error per operation (motional quanta added).
    pub shuttle_heating: f64,
    /// Crosstalk ratio.
    pub crosstalk_ratio: f64,
}

impl Default for TrappedIonNoise {
    fn default() -> Self {
        Self {
            t1: 10.0,
            t2: 2.0,
            single_gate_error: 2e-5,
            single_gate_time: 10e-6,
            ms_gate_error: 2e-3,
            ms_gate_time: 300e-6,
            heating_rate: 10.0,
            readout_error: 3e-3,
            readout_time: 200e-6,
            shuttle_heating: 0.5,
            crosstalk_ratio: 1e-3,
        }
    }
}

impl NoiseModel for TrappedIonNoise {
    fn post_single_gate(
        &self,
        state: &mut DensityMatrix,
        target_qubit: u8,
        gate_time: f64,
    ) {
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, gate_time, 0.0
        );
        state.apply_single_qubit_channel(target_qubit, &kraus);

        if self.single_gate_error > 0.0 {
            let kraus = channels::depolarizing_single(self.single_gate_error);
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

        // MS gate depolarizing
        if self.ms_gate_error > 0.0 {
            let kraus = channels::depolarizing_two_qubit(self.ms_gate_error);
            state.apply_two_qubit_channel(qubit_a, qubit_b, &kraus);
        }

        // Motional heating: additional dephasing proportional to heating_rate * gate_time
        let heating_dephasing = (self.heating_rate * gate_time).min(1.0);
        if heating_dephasing > 0.0 {
            let kraus = channels::phase_damping(heating_dephasing);
            state.apply_single_qubit_channel(qubit_a, &kraus);
            state.apply_single_qubit_channel(qubit_b, &kraus);
        }
    }

    fn idle_noise(
        &self,
        state: &mut DensityMatrix,
        duration: f64,
    ) {
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
        // Hyperfine qubits: T1 >> circuit time, no thermal population
    }

    fn bell_pair_fidelity(&self) -> f64 { 1.0 - self.ms_gate_error }

    fn name(&self) -> &str { "trapped-ion" }

    fn has_readout_noise(&self) -> bool {
        self.readout_error > 0.0
    }

    fn single_gate_time(&self) -> f64 { self.single_gate_time }
    fn two_gate_time(&self) -> f64 { self.ms_gate_time }

    fn kraus_single_gate(
        &self,
        _target_qubit: u8,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        let mut result = vec![
            channels::thermal_relaxation(self.t1, self.t2, gate_time, 0.0),
        ];
        if self.single_gate_error > 0.0 {
            result.push(channels::depolarizing_single(self.single_gate_error));
        }
        result
    }

    fn kraus_two_qubit_gate(
        &self,
        _qubit_a: u8,
        _qubit_b: u8,
        _gate_time: f64,
    ) -> Vec<Vec<[C64; 16]>> {
        if self.ms_gate_error > 0.0 {
            vec![channels::depolarizing_two_qubit(self.ms_gate_error)]
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
        let heating_dephasing = (self.heating_rate * gate_time).min(1.0);
        if heating_dephasing > 0.0 {
            result.push(channels::phase_damping(heating_dephasing));
        }
        result
    }

    fn kraus_idle(&self, duration: f64) -> Vec<[C64; 4]> {
        channels::thermal_relaxation(self.t1, self.t2, duration, 0.0)
    }
}
