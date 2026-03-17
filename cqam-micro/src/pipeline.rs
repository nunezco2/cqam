//! Compilation pipeline: decompose -> route -> native_map -> optimize -> cache.
//!
//! `CompilationPipeline::synthesize()` is the main entry point, transforming
//! a `circuit_ir::MicroProgram` into a hardware-ready `native_ir::Circuit`.

use cqam_core::circuit_ir;
use cqam_core::native_ir::{self, NativeGateSet, Circuit};
use cqam_qpu::traits::{ConnectivityGraph, CalibrationData, QpuMetrics};
use crate::cache::{CircuitCache, CachedCircuit, rebind};
use crate::decompose::decompose_to_standard;
use crate::native_map::map_to_native;
use crate::routing::route;
use crate::optimize::optimize;
use crate::error::MicroError;

pub struct CompilationPipeline {
    gate_set: NativeGateSet,
    connectivity: ConnectivityGraph,
    cache: CircuitCache,
    metrics: QpuMetrics,
}

impl CompilationPipeline {
    pub fn new(
        gate_set: NativeGateSet,
        connectivity: ConnectivityGraph,
        cache_capacity: usize,
    ) -> Self {
        Self {
            gate_set,
            connectivity,
            cache: CircuitCache::new(cache_capacity),
            metrics: QpuMetrics::default(),
        }
    }

    pub fn synthesize(
        &mut self,
        program: &circuit_ir::MicroProgram,
        calibration: Option<&dyn CalibrationData>,
    ) -> Result<Circuit, MicroError> {
        // 1. Compute structure key
        let mut program_clone = program.clone();
        let key = program_clone.compute_structure_key();

        // 2. Check cache
        if let Some(cached) = self.cache.lookup(key) {
            let new_params = extract_resolved_params(program);
            let circuit = rebind(cached, &new_params);
            self.metrics.cache_hits += 1;
            self.metrics.compilations += 1;
            return Ok(circuit);
        }

        // 3. Full compilation pipeline
        // 3a. Decompose kernels to standard gate set
        let decomposed = decompose_to_standard(program)?;

        // 3b. Route on connectivity graph
        let (routed, routing_result) = route(&decomposed, &self.connectivity, calibration)?;

        // 3c. Map to native gate set
        let mut native = map_to_native(&routed, &self.gate_set)?;

        // 3d. Optimize
        optimize(&mut native);

        // 4. Update metrics
        native.swap_count = routing_result.swaps_inserted;
        native.qubit_map = routing_result.virtual_to_physical
            .iter()
            .map(|&p| native_ir::PhysicalQubit(p))
            .collect();
        self.metrics.swap_count += routing_result.swaps_inserted;
        self.metrics.circuit_depth = native.depth;
        self.metrics.physical_qubits_used = native.num_physical_qubits;
        self.metrics.compilations += 1;

        // 5. Cache the result
        let param_slots = identify_param_slots(&native);
        self.cache.insert(key, CachedCircuit {
            template: native.clone(),
            param_slots,
        });

        Ok(native)
    }

    pub fn metrics(&self) -> &QpuMetrics {
        &self.metrics
    }
}

impl Clone for CompilationPipeline {
    /// Clone the pipeline, but start with a fresh empty cache.
    ///
    /// Fork threads run short-lived quantum sections that will not benefit from
    /// the parent's cached compiled circuits, so the cache is not copied.
    fn clone(&self) -> Self {
        Self {
            gate_set: self.gate_set.clone(),
            connectivity: self.connectivity.clone(),
            cache: CircuitCache::new(self.cache.capacity()),
            metrics: self.metrics.clone(),
        }
    }
}

/// Extract all Resolved parameter values from a MicroProgram, in op order.
fn extract_resolved_params(program: &circuit_ir::MicroProgram) -> Vec<f64> {
    let mut params = Vec::new();
    for op in &program.ops {
        if let circuit_ir::Op::Gate1q(g) = op {
            match &g.gate {
                circuit_ir::Gate1q::Rx(p) | circuit_ir::Gate1q::Ry(p)
                | circuit_ir::Gate1q::Rz(p) => {
                    if let Some(v) = p.value() { params.push(v); }
                }
                circuit_ir::Gate1q::U3(a, b, c) => {
                    if let Some(v) = a.value() { params.push(v); }
                    if let Some(v) = b.value() { params.push(v); }
                    if let Some(v) = c.value() { params.push(v); }
                }
                _ => {}
            }
        }
    }
    params
}

/// Identify which ops in the native circuit correspond to parameterized
/// Rz gates (for cache rebinding).
fn identify_param_slots(circuit: &Circuit) -> Vec<usize> {
    circuit.ops.iter().enumerate()
        .filter_map(|(i, op)| match op {
            native_ir::Op::Gate1q(g) => match g.gate {
                native_ir::NativeGate1::Rz(_) => Some(i),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cqam_core::circuit_ir::{MicroProgram, Op, QWire, ApplyGate1q as CApplyGate1q,
        ApplyGate2q as CApplyGate2q, Gate1q, Gate2q, ApplyKernel, Param};
    use cqam_core::instruction::KernelId;
    use cqam_core::quantum_backend::KernelParams;

    #[test]
    fn test_synthesize_bell_circuit() {
        let mut mp = MicroProgram::new(2);
        mp.push(Op::Gate1q(CApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));
        mp.push(Op::Gate2q(CApplyGate2q {
            wire_a: QWire(0),
            wire_b: QWire(1),
            gate: Gate2q::Cx,
        }));

        let conn = ConnectivityGraph::all_to_all(2);
        let mut pipeline = CompilationPipeline::new(
            NativeGateSet::Superconducting, conn, 16,
        );
        let circuit = pipeline.synthesize(&mp, None).unwrap();
        assert!(!circuit.is_empty());
        assert!(circuit.gate2q_count() >= 1);
    }

    #[test]
    fn test_synthesize_cache_hit() {
        let mut mp = MicroProgram::new(1);
        mp.push(Op::Gate1q(CApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::Rz(Param::Resolved(1.0)),
        }));

        let conn = ConnectivityGraph::all_to_all(1);
        let mut pipeline = CompilationPipeline::new(
            NativeGateSet::Superconducting, conn, 16,
        );

        let _c1 = pipeline.synthesize(&mp, None).unwrap();
        assert_eq!(pipeline.metrics().compilations, 1);

        // Same structure, different parameter
        let mut mp2 = MicroProgram::new(1);
        mp2.push(Op::Gate1q(CApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::Rz(Param::Resolved(2.0)),
        }));
        let _c2 = pipeline.synthesize(&mp2, None).unwrap();
        assert_eq!(pipeline.metrics().compilations, 2);
        assert_eq!(pipeline.metrics().cache_hits, 1);
    }

    #[test]
    fn test_synthesize_linear_connectivity_swaps() {
        let mut mp = MicroProgram::new(3);
        mp.push(Op::Gate2q(CApplyGate2q {
            wire_a: QWire(0),
            wire_b: QWire(2),
            gate: Gate2q::Cx,
        }));

        let conn = ConnectivityGraph::linear(3);
        let mut pipeline = CompilationPipeline::new(
            NativeGateSet::Superconducting, conn, 16,
        );
        let circuit = pipeline.synthesize(&mp, None).unwrap();
        assert!(circuit.swap_count >= 1,
            "Expected SWAP insertion on linear topology");
    }

    #[test]
    fn test_synthesize_kernel_fourier() {
        let mut mp = MicroProgram::new(2);
        mp.push(Op::Kernel(ApplyKernel {
            wires: vec![QWire(0), QWire(1)],
            kernel: KernelId::Fourier,
            params: KernelParams::Int { param0: 0, param1: 0, cmem_data: vec![] },
        }));

        let conn = ConnectivityGraph::all_to_all(2);
        let mut pipeline = CompilationPipeline::new(
            NativeGateSet::Superconducting, conn, 16,
        );
        let circuit = pipeline.synthesize(&mp, None).unwrap();
        assert!(!circuit.is_empty());
    }

    #[test]
    fn test_metrics_accumulation() {
        let mut mp = MicroProgram::new(2);
        mp.push(Op::Gate1q(CApplyGate1q {
            wire: QWire(0),
            gate: Gate1q::H,
        }));

        let conn = ConnectivityGraph::all_to_all(2);
        let mut pipeline = CompilationPipeline::new(
            NativeGateSet::Superconducting, conn, 16,
        );

        let _ = pipeline.synthesize(&mp, None).unwrap();
        let _ = pipeline.synthesize(&mp, None).unwrap();
        assert_eq!(pipeline.metrics().compilations, 2);
    }
}
