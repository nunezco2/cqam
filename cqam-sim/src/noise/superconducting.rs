//! Superconducting qubit noise model (IBM Eagle/Heron-class).

use serde::Deserialize;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use super::{NoiseModel, channels};

/// Noise parameters for superconducting transmon qubits.
#[derive(Deserialize)]
#[serde(default)]
pub struct SuperconductingNoise {
    /// T1 relaxation time in seconds.
    pub t1: f64,
    /// T2 dephasing time in seconds (must satisfy T2 <= 2*T1).
    pub t2: f64,
    /// Single-qubit gate error probability (depolarizing).
    pub single_gate_error: f64,
    /// Single-qubit gate duration in seconds.
    pub single_gate_time: f64,
    /// Two-qubit gate error probability (depolarizing).
    pub two_gate_error: f64,
    /// Two-qubit gate duration in seconds.
    pub two_gate_time: f64,
    /// Readout error probabilities: (P(1|0), P(0|1)).
    pub readout_error: (f64, f64),
    /// Readout duration in seconds.
    pub readout_time: f64,
    /// ZZ crosstalk strength in radians per second.
    pub crosstalk_zz: f64,
    /// Leakage probability per gate.
    pub leakage_rate: f64,
    /// Thermal excited-state population at equilibrium.
    pub thermal_population: f64,
}

impl Default for SuperconductingNoise {
    fn default() -> Self {
        Self {
            t1: 300e-6,
            t2: 200e-6,
            single_gate_error: 3e-4,
            single_gate_time: 35e-9,
            two_gate_error: 7e-3,
            two_gate_time: 400e-9,
            readout_error: (0.01, 0.02),
            readout_time: 700e-9,
            crosstalk_zz: 2.0 * std::f64::consts::PI * 50e3,
            leakage_rate: 5e-4,
            thermal_population: 0.01,
        }
    }
}

impl NoiseModel for SuperconductingNoise {
    fn post_single_gate(
        &self,
        state: &mut DensityMatrix,
        target_qubit: u8,
        gate_time: f64,
    ) {
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, gate_time, self.thermal_population
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
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, gate_time, self.thermal_population
        );
        state.apply_single_qubit_channel(qubit_a, &kraus);
        state.apply_single_qubit_channel(qubit_b, &kraus);

        if self.two_gate_error > 0.0 {
            let kraus = channels::depolarizing_two_qubit(self.two_gate_error);
            state.apply_two_qubit_channel(qubit_a, qubit_b, &kraus);
        }
    }

    fn idle_noise(
        &self,
        state: &mut DensityMatrix,
        duration: f64,
    ) {
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, duration, self.thermal_population
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
            self.readout_error.0,
            self.readout_error.1,
        );
    }

    fn prep_noise(&self, state: &mut DensityMatrix) {
        if self.thermal_population > 0.0 {
            let kraus = channels::bit_flip(self.thermal_population);
            for q in 0..state.num_qubits() {
                state.apply_single_qubit_channel(q, &kraus);
            }
        }
    }

    fn name(&self) -> &str { "superconducting" }

    fn has_readout_noise(&self) -> bool {
        self.readout_error.0 > 0.0 || self.readout_error.1 > 0.0
    }

    fn single_gate_time(&self) -> f64 { self.single_gate_time }
    fn two_gate_time(&self) -> f64 { self.two_gate_time }

    fn kraus_single_gate(
        &self,
        _target_qubit: u8,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        let mut result = vec![
            channels::thermal_relaxation(self.t1, self.t2, gate_time, self.thermal_population),
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
        if self.two_gate_error > 0.0 {
            vec![channels::depolarizing_two_qubit(self.two_gate_error)]
        } else {
            vec![]
        }
    }

    fn kraus_two_qubit_per_qubit(
        &self,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        vec![
            channels::thermal_relaxation(self.t1, self.t2, gate_time, self.thermal_population),
        ]
    }

    fn kraus_idle(&self, duration: f64) -> Vec<[C64; 4]> {
        channels::thermal_relaxation(self.t1, self.t2, duration, self.thermal_population)
    }
}
