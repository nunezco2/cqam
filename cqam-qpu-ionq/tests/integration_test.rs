//! Live integration tests against the IonQ Cloud API v0.4.
//!
//! These tests require a valid `IONQ_API_KEY` environment variable and make
//! real network calls to `https://api.ionq.co/v0.4`. They are marked
//! `#[ignore]` so they do not run in CI; invoke them explicitly with:
//!
//! ```sh
//! IONQ_API_KEY=<key> cargo test -p cqam-qpu-ionq -- --ignored
//! ```

use cqam_core::native_ir::{
    ApplyGate1q, Circuit, NativeGate1, NativeGate2, Observe, Op, PhysicalQubit,
};
use cqam_qpu_ionq::IonQQpuBackend;
use cqam_qpu_ionq::rest::IonQRestClient;

fn api_key() -> String {
    std::env::var("IONQ_API_KEY")
        .expect("IONQ_API_KEY must be set to run integration tests")
}

// ---------------------------------------------------------------------------
// REST client tests
// ---------------------------------------------------------------------------

/// Verify that the /backends endpoint returns a non-empty list and includes
/// the simulator backend.
#[test]
#[ignore]
fn test_live_list_backends() {
    let client = IonQRestClient::new(api_key(), "simulator");
    let backends = client.list_backends().expect("list_backends should succeed");

    assert!(!backends.is_empty(), "backend list should not be empty");

    let simulator = backends.iter().find(|b| b.backend == "simulator");
    assert!(simulator.is_some(), "simulator backend must be present");

    let sim = simulator.unwrap();
    assert_eq!(sim.qubits, Some(29));
    // Simulator has no characterization_id.
    assert!(
        sim.characterization_id.is_none(),
        "simulator should not have a characterization_id"
    );
}

/// Verify that the forte-1 characterization endpoint returns calibration data.
#[test]
#[ignore]
fn test_live_get_forte1_characterization() {
    let client = IonQRestClient::new(api_key(), "qpu.forte-1");
    let backends = client.list_backends().expect("list_backends should succeed");

    let forte1 = backends.iter().find(|b| b.backend == "qpu.forte-1")
        .expect("qpu.forte-1 must be in backend list");

    let char_id = forte1.characterization_id.as_deref()
        .expect("qpu.forte-1 must have a characterization_id");

    let char_resp = client
        .get_characterization("qpu.forte-1", char_id)
        .expect("get_characterization should succeed");

    assert_eq!(char_resp.backend, "qpu.forte-1");
    assert_eq!(char_resp.qubits, 36);

    // Verify fidelity fields are present and plausible.
    let fidelity = char_resp.fidelity.expect("forte-1 should have fidelity data");
    let spam = fidelity.spam.expect("forte-1 should have SPAM fidelity");
    let spam_median = spam.median.expect("SPAM median should be present");
    assert!(
        spam_median > 0.9 && spam_median <= 1.0,
        "SPAM fidelity should be >90%: got {spam_median}"
    );

    // 1q and 2q fidelity.
    let sq_fidelity = fidelity.single_qubit
        .and_then(|g| g.median)
        .expect("single-qubit gate fidelity should be present");
    assert!(sq_fidelity > 0.99, "1Q fidelity should be >99%: got {sq_fidelity}");

    let tq_fidelity = fidelity.two_qubit
        .and_then(|g| g.median)
        .expect("two-qubit gate fidelity should be present");
    assert!(tq_fidelity > 0.9, "2Q fidelity should be >90%: got {tq_fidelity}");

    // Timing fields.
    let timing = char_resp.timing.expect("forte-1 should have timing data");
    let t1 = timing.t1.expect("T1 should be present");
    assert!(t1 > 0.0, "T1 should be positive: got {t1}");
    let t2 = timing.t2.expect("T2 should be present");
    assert!(t2 > 0.0, "T2 should be positive: got {t2}");
}

// ---------------------------------------------------------------------------
// Job submission tests
// ---------------------------------------------------------------------------

/// Submit a Bell circuit to the simulator, poll for completion, and verify
/// that the result is approximately a 50/50 distribution over |00⟩ and |11⟩.
#[test]
#[ignore]
fn test_live_bell_circuit_simulator() {
    let client = IonQRestClient::new(api_key(), "simulator");

    // Build: V(0) → CNOT(0→1) → Bell state
    let circuit_json = serde_json::json!({
        "gateset": "qis",
        "qubits": 2,
        "circuit": [
            {"gate": "v", "target": 0},
            {"gate": "cnot", "control": 0, "target": 1}
        ]
    });

    let job_id = client
        .submit_job(circuit_json, 1024)
        .expect("submit_job should succeed");

    assert!(!job_id.is_empty(), "job ID should not be empty");
    println!("Submitted job: {job_id}");

    // Poll with 60-second timeout.
    let result = client
        .poll_until_done(&job_id, None, Some(std::time::Duration::from_secs(60)))
        .expect("poll_until_done should succeed");

    assert_eq!(result.status, "completed");

    // Fetch probabilities.
    let probs_url = result
        .results.expect("completed job must have results")
        .probabilities.expect("completed job must have probabilities URL");

    let (counts, total_shots) = client
        .get_probabilities_with_shots(&probs_url.url, 1024)
        .expect("get_probabilities_with_shots should succeed");

    println!("Counts: {counts:?}, total_shots: {total_shots}");

    // Bell state: only |00⟩ (key=0) and |11⟩ (key=3) should be populated.
    assert_eq!(counts.len(), 2, "Bell state should only have 2 outcomes");
    assert!(counts.contains_key(&0), "Bell state must include |00⟩");
    assert!(counts.contains_key(&3), "Bell state must include |11⟩");

    // With the ideal simulator both probabilities should be exactly 0.5.
    let p00 = counts[&0] as f64 / total_shots as f64;
    let p11 = counts[&3] as f64 / total_shots as f64;
    assert!(
        (p00 - 0.5).abs() < 0.05,
        "|00⟩ probability should be ~0.5, got {p00}"
    );
    assert!(
        (p11 - 0.5).abs() < 0.05,
        "|11⟩ probability should be ~0.5, got {p11}"
    );
}

/// Full backend workflow: `IonQQpuBackend::from_device` pulls live calibration,
/// then `compile` validates a simple circuit.
#[test]
#[ignore]
fn test_live_from_device_calibration() {
    let backend = IonQQpuBackend::from_device(api_key(), "qpu.forte-1")
        .expect("from_device should succeed for qpu.forte-1");

    assert_eq!(backend.max_qubits(), 36);

    // Calibration data should reflect real hardware values.
    let cal = backend.calibration().expect("calibration should succeed");
    let t1 = cal.t1(0);
    assert!(t1 > 1.0, "T1 for forte-1 should be >1 second, got {t1}");

    // Single-qubit gate fidelity should be better than 99%.
    let sq_err = cal.single_gate_error(0);
    assert!(
        sq_err < 0.01,
        "1Q gate error should be <1%, got {sq_err}"
    );

    // compile() should accept a small Bell circuit.
    let mut c = Circuit::new(2);
    c.ops.push(Op::Gate1q(ApplyGate1q {
        qubit: PhysicalQubit(0),
        gate: NativeGate1::Sx,
    }));
    c.ops.push(Op::Gate2q(cqam_core::native_ir::ApplyGate2q {
        qubit_a: PhysicalQubit(0),
        qubit_b: PhysicalQubit(1),
        gate: NativeGate2::Cx,
    }));
    c.ops.push(Op::Measure(Observe { qubit: PhysicalQubit(0), clbit: 0 }));
    c.ops.push(Op::Measure(Observe { qubit: PhysicalQubit(1), clbit: 1 }));

    use cqam_qpu::traits::QpuBackend;
    backend.compile(&c).expect("compile should accept a valid Bell circuit");
}
