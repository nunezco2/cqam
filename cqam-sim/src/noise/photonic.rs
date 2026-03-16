//! Photonic qubit noise model (PsiQuantum/Xanadu-class).

use serde::Deserialize;
use crate::complex::C64;
use crate::density_matrix::DensityMatrix;
use super::{NoiseModel, channels};

/// Noise parameters for photonic qubits.
#[derive(Deserialize)]
#[serde(default)]
pub struct PhotonicNoise {
    /// Photon loss per optical component in linear scale.
    pub loss_per_component: f64,
    /// Number of components per single-qubit gate.
    pub components_per_single_gate: u32,
    /// Number of components per two-qubit gate.
    pub components_per_two_gate: u32,
    /// Detector efficiency (0 to 1).
    pub detector_efficiency: f64,
    /// Dark count probability per detector per gate cycle.
    pub dark_count_prob: f64,
    /// Hong-Ou-Mandel visibility (0 to 1).
    pub hom_visibility: f64,
    /// Phase error standard deviation in radians.
    pub phase_error_std: f64,
    /// Fusion success probability (for FBQC).
    pub fusion_success_prob: f64,
}

impl Default for PhotonicNoise {
    fn default() -> Self {
        Self {
            loss_per_component: 0.023,
            components_per_single_gate: 3,
            components_per_two_gate: 6,
            detector_efficiency: 0.95,
            dark_count_prob: 1e-8,
            hom_visibility: 0.98,
            phase_error_std: 5e-3,
            fusion_success_prob: 0.5,
        }
    }
}

impl PhotonicNoise {
    /// Compute total transmission for a given number of components.
    fn transmission(&self, num_components: u32) -> f64 {
        (1.0 - self.loss_per_component).powi(num_components as i32)
    }
}

impl NoiseModel for PhotonicNoise {
    fn post_single_gate(
        &self,
        state: &mut DensityMatrix,
        target_qubit: u8,
        _gate_time: f64,
    ) {
        // Photon loss through optical components
        let eta = self.transmission(self.components_per_single_gate);
        if eta < 1.0 {
            let kraus = channels::photon_loss(eta);
            state.apply_single_qubit_channel(target_qubit, &kraus);
        }

        // Phase error modeled as phase damping
        let lambda = self.phase_error_std * self.phase_error_std;
        if lambda > 0.0 {
            let kraus = channels::phase_damping(lambda.min(1.0));
            state.apply_single_qubit_channel(target_qubit, &kraus);
        }
    }

    fn post_two_qubit_gate(
        &self,
        state: &mut DensityMatrix,
        qubit_a: u8,
        qubit_b: u8,
        _gate_time: f64,
    ) {
        // Photon loss on both qubits
        let eta = self.transmission(self.components_per_two_gate);
        if eta < 1.0 {
            let kraus = channels::photon_loss(eta);
            state.apply_single_qubit_channel(qubit_a, &kraus);
            state.apply_single_qubit_channel(qubit_b, &kraus);
        }

        // HOM visibility imperfection -> two-qubit depolarizing
        let hom_error = 1.0 - self.hom_visibility;
        if hom_error > 0.0 {
            let kraus = channels::depolarizing_two_qubit(hom_error);
            state.apply_two_qubit_channel(qubit_a, qubit_b, &kraus);
        }

        // Phase error on both qubits
        let lambda = self.phase_error_std * self.phase_error_std;
        if lambda > 0.0 {
            let kraus = channels::phase_damping(lambda.min(1.0));
            state.apply_single_qubit_channel(qubit_a, &kraus);
            state.apply_single_qubit_channel(qubit_b, &kraus);
        }
    }

    fn idle_noise(
        &self,
        _state: &mut DensityMatrix,
        _duration: f64,
    ) {
        // Photons don't decohere in free propagation -- no idle noise
    }

    fn readout_noise(
        &self,
        probabilities: &mut [f64],
        _qubit: u8,
    ) {
        // Detector inefficiency modeled as readout confusion
        let p_miss = 1.0 - self.detector_efficiency;
        if p_miss > 0.0 || self.dark_count_prob > 0.0 {
            channels::apply_readout_confusion(
                probabilities,
                self.dark_count_prob,
                p_miss,
            );
        }
    }

    fn prep_noise(&self, _state: &mut DensityMatrix) {
        // No thermal population for photonic qubits
    }

    fn name(&self) -> &str { "photonic" }

    fn has_readout_noise(&self) -> bool {
        self.detector_efficiency < 1.0 || self.dark_count_prob > 0.0
    }

    fn single_gate_time(&self) -> f64 {
        // Photonic gates are effectively instantaneous (speed of light)
        0.0
    }

    fn two_gate_time(&self) -> f64 {
        0.0
    }

    fn kraus_single_gate(
        &self,
        _target_qubit: u8,
        _gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        let mut result = Vec::new();
        let eta = self.transmission(self.components_per_single_gate);
        if eta < 1.0 {
            result.push(channels::photon_loss(eta));
        }
        let lambda = self.phase_error_std * self.phase_error_std;
        if lambda > 0.0 {
            result.push(channels::phase_damping(lambda.min(1.0)));
        }
        result
    }

    fn kraus_two_qubit_gate(
        &self,
        _qubit_a: u8,
        _qubit_b: u8,
        _gate_time: f64,
    ) -> Vec<Vec<[C64; 16]>> {
        let hom_error = 1.0 - self.hom_visibility;
        if hom_error > 0.0 {
            vec![channels::depolarizing_two_qubit(hom_error)]
        } else {
            vec![]
        }
    }

    fn kraus_two_qubit_per_qubit(
        &self,
        _gate_time: f64,
    ) -> Vec<Vec<[C64; 4]>> {
        let mut result = Vec::new();
        let eta = self.transmission(self.components_per_two_gate);
        if eta < 1.0 {
            result.push(channels::photon_loss(eta));
        }
        let lambda = self.phase_error_std * self.phase_error_std;
        if lambda > 0.0 {
            result.push(channels::phase_damping(lambda.min(1.0)));
        }
        result
    }

    fn kraus_idle(&self, _duration: f64) -> Vec<[C64; 4]> {
        // No idle noise: return identity channel
        vec![[C64::ONE, C64::ZERO, C64::ZERO, C64::ONE]]
    }
}
