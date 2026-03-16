//! Spin qubit noise model (Silicon quantum dots).

use serde::Deserialize;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use super::{NoiseModel, channels};

/// Noise parameters for semiconductor spin qubits.
#[derive(Deserialize)]
#[serde(default)]
pub struct SpinQubitNoise {
    /// T1 relaxation time in seconds.
    pub t1: f64,
    /// T2 (Hahn echo) dephasing time in seconds.
    pub t2: f64,
    /// T2* (free induction decay) time in seconds.
    pub t2_star: f64,
    /// Single-qubit gate error probability (depolarizing).
    pub single_gate_error: f64,
    /// Single-qubit gate duration in seconds.
    pub single_gate_time: f64,
    /// Exchange-based two-qubit gate error probability.
    pub exchange_error: f64,
    /// Two-qubit gate duration in seconds.
    pub two_gate_time: f64,
    /// Charge noise amplitude (dimensionless).
    pub charge_noise: f64,
    /// Readout error probability.
    pub readout_error: f64,
    /// Readout duration in seconds.
    pub readout_time: f64,
    /// Nearest-neighbor capacitive crosstalk fraction.
    pub crosstalk_fraction: f64,
}

impl Default for SpinQubitNoise {
    fn default() -> Self {
        Self {
            t1: 10e-3,
            t2: 10e-3,
            t2_star: 20e-6,
            single_gate_error: 4e-4,
            single_gate_time: 1e-6,
            exchange_error: 5e-3,
            two_gate_time: 1e-6,
            charge_noise: 0.01,
            readout_error: 0.01,
            readout_time: 10e-6,
            crosstalk_fraction: 0.03,
        }
    }
}

impl NoiseModel for SpinQubitNoise {
    fn post_single_gate(
        &self,
        state: &mut DensityMatrix,
        target_qubit: u8,
        gate_time: f64,
    ) {
        // Thermal relaxation using T2 (Hahn echo, assumes dynamical decoupling during gates)
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2, gate_time, 0.0
        );
        state.apply_single_qubit_channel(target_qubit, &kraus);

        // Gate error + charge noise
        let total_error = (self.single_gate_error + self.charge_noise).min(1.0);
        if total_error > 0.0 {
            let kraus = channels::depolarizing_single(total_error);
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

        // Exchange gate depolarizing
        if self.exchange_error > 0.0 {
            let kraus = channels::depolarizing_two_qubit(self.exchange_error);
            state.apply_two_qubit_channel(qubit_a, qubit_b, &kraus);
        }

        // Charge noise on both qubits
        if self.charge_noise > 0.0 {
            let kraus = channels::depolarizing_single(self.charge_noise.min(1.0));
            state.apply_single_qubit_channel(qubit_a, &kraus);
            state.apply_single_qubit_channel(qubit_b, &kraus);
        }
    }

    fn idle_noise(
        &self,
        state: &mut DensityMatrix,
        duration: f64,
    ) {
        // Use T2* for idle (free induction decay without echo refocusing)
        let kraus = channels::thermal_relaxation(
            self.t1, self.t2_star, duration, 0.0
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
        // Spin qubits: initialization by measurement-based reset, no thermal population
    }

    fn name(&self) -> &str { "spin" }

    fn has_readout_noise(&self) -> bool {
        self.readout_error > 0.0
    }

    fn single_gate_time(&self) -> f64 { self.single_gate_time }
    fn two_gate_time(&self) -> f64 { self.two_gate_time }

    fn kraus_single_gate(
        &self,
        _target_qubit: u8,
        gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        let mut result = vec![
            channels::thermal_relaxation(self.t1, self.t2, gate_time, 0.0),
        ];
        let total_error = (self.single_gate_error + self.charge_noise).min(1.0);
        if total_error > 0.0 {
            result.push(channels::depolarizing_single(total_error));
        }
        result
    }

    fn kraus_two_qubit_gate(
        &self,
        _qubit_a: u8,
        _qubit_b: u8,
        _gate_time: f64,
    ) -> Vec<Vec<[C64; 16]>> {
        if self.exchange_error > 0.0 {
            vec![channels::depolarizing_two_qubit(self.exchange_error)]
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
        if self.charge_noise > 0.0 {
            result.push(channels::depolarizing_single(self.charge_noise.min(1.0)));
        }
        result
    }

    fn kraus_idle(&self, duration: f64) -> Vec<[C64; 4]> {
        channels::thermal_relaxation(self.t1, self.t2_star, duration, 0.0)
    }
}
