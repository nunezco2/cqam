# CQAM IBM QPU Backend Reference

This document describes how to build, configure, and use the CQAM IBM Quantum
Platform integration. The integration submits compiled CQAM programs to real
IBM quantum hardware via the IBM Quantum Platform v2 REST API. It is gated
behind an optional Cargo feature flag and requires a linked native Qiskit C
library for circuit transpilation.

---

## 1. Prerequisites

### 1.1 Qiskit C API

The transpiler path calls into the Qiskit C API (`libqiskit`). You must build
it before compiling the `ibm` feature.

**Repository:** https://github.com/Qiskit/qiskit (the `c-api` branch or the
directory that produces C header exports).

**Build command:**

```sh
cd /path/to/qiskit
make c
```

This produces:

- `dist/c/lib/libqiskit.dylib` (macOS) or `libqiskit.so` (Linux)
- `dist/c/include/qiskit/types.h`, `funcs.h`, and related headers

**Environment variable:**

```sh
export QISKIT_C_DIR=/path/to/qiskit/dist/c
```

The `cqam-qpu-ibm` build script (`build.rs`) reads this variable and passes
the correct library search path to the linker:

```
cargo:rustc-link-search=native=$QISKIT_C_DIR/lib
cargo:rustc-link-lib=dylib=qiskit
```

If `QISKIT_C_DIR` is not set, the build script defaults to `/tmp/qiskit/dist/c`.
The build will succeed but the binary will fail to launch at runtime if the
library is not present at that fallback path.

### 1.2 IBM Quantum Account and API Token

An IBM Quantum account with access to at least one real backend (or the IBM
Quantum Platform simulator) is required. Obtain your API token from
https://quantum.ibm.com/account.

---

## 2. Building with IBM Support

The IBM backend is an optional feature of `cqam-run`. It is not compiled by
default so that users without the Qiskit C dependency are not affected.

**Build with IBM support:**

```sh
QISKIT_C_DIR=/path/to/qiskit/dist/c cargo build --features ibm -p cqam-run
```

**Build only the IBM crate (for development):**

```sh
QISKIT_C_DIR=/path/to/qiskit/dist/c cargo build -p cqam-qpu-ibm
```

**Run tests (including the FFI layer):**

```sh
QISKIT_C_DIR=/path/to/qiskit/dist/c cargo test --features ibm --workspace
```

**What happens without the feature flag:**

If you pass `--backend ibm` to a binary built without `--features ibm`, the
runner returns an error at runtime with a clear instruction:

```
Error: IBM backend not available. Rebuild with: cargo build --features ibm
```

This check is enforced by a `#[cfg(not(feature = "ibm"))]` branch in
`cqam-run/src/runner.rs`.

**Cargo feature definition (`cqam-run/Cargo.toml`):**

```toml
[features]
default = []
ibm = ["cqam-qpu-ibm"]

[dependencies]
cqam-qpu-ibm = { path = "../cqam-qpu-ibm", optional = true }
```

The `cqam-qpu-ibm` crate is always compiled as a workspace member but is only
linked into `cqam-run` when the `ibm` feature is active.

**Link-time note:** The `libqiskit` dynamic library must be discoverable at
runtime via `DYLD_LIBRARY_PATH` (macOS) or `LD_LIBRARY_PATH` (Linux):

```sh
export DYLD_LIBRARY_PATH=$QISKIT_C_DIR/lib:$DYLD_LIBRARY_PATH
```

---

## 3. Authentication

### 3.1 Three-Level Token Resolution

The runner resolves the IBM API token through the following precedence chain,
stopping at the first successful source (implemented in
`cqam-run/src/runner.rs`, function `resolve_ibm_token`):

1. **CLI flag** — `--ibm-token <TOKEN>` (highest priority)
2. **Environment variable** — `IBM_QUANTUM_TOKEN`
3. **Token file** — `~/.qiskit/ibm_quantum_token` (contents trimmed of whitespace)

If no token is found at any level, execution halts with:

```
Error: IBM Quantum token not found. Provide one of:
  --ibm-token <TOKEN>
  IBM_QUANTUM_TOKEN environment variable
  ~/.qiskit/ibm_quantum_token file
```

An empty string at any level (empty environment variable, empty file) is
treated as absent and the search continues to the next level.

### 3.2 Security

The token field in `SimConfig` is marked `#[serde(skip)]`, so it is never
written to or read from TOML configuration files. Tokens in config files would
be a security risk; the design intentionally disallows it.

When `--verbose` is passed, the runner redacts the token before printing the
config struct:

```
Config: SimConfig { ..., ibm_token: Some("***"), ... }
```

The redaction is temporary: the original token is restored to `SimConfig`
immediately after the log line, before execution begins.

---

## 4. CLI Usage

All IBM-specific flags are accepted only when `--backend ibm` is also given.
Passing `--ibm-token` with `--backend simulation` is silently accepted but
has no effect.

### 4.1 Flag Reference

| Flag | Type | Default | Description |
|---|---|---|---|
| `--backend ibm` | string | `simulation` | Select the IBM QPU backend |
| `--ibm-token <TOKEN>` | string | (see Section 3) | IBM Quantum API token |
| `--qpu-device <NAME>` | string | `ibm_brisbane` | Target device name |
| `--ibm-optimization-level <N>` | u8, 0-3 | `1` | Qiskit transpiler optimization level |
| `--qpu-shots <N>` | u32 | `4096` | Total shot budget for the execution |
| `--qpu-confidence <F>` | f64, 0.0-1.0 | `0.95` | Bayesian convergence confidence level |

Values passed to `--ibm-optimization-level` greater than 3 are clamped to 3
with a warning; they do not cause an error.

### 4.2 Flags Incompatible with QPU Backends

The following simulation-only flags produce errors or warnings when combined
with `--backend ibm`:

- `--noise <model>` or `--noise-method <m>`: error — noise injection is
  simulation-only.
- `--density-matrix`: warning only — has no effect on QPU backends.
- `--shots <N>`: warning — redundant with QPU backends, which produce
  shot-based histograms internally; use `--qpu-shots` instead.

### 4.3 Example Commands

**Minimal invocation using environment variable token:**

```sh
export IBM_QUANTUM_TOKEN=your_token_here
cqam-run program.cqam --backend ibm
```

**Specifying all relevant options:**

```sh
cqam-run program.cqam \
  --backend ibm \
  --ibm-token your_token_here \
  --qpu-device ibm_sherbrooke \
  --ibm-optimization-level 2 \
  --qpu-shots 8192 \
  --qpu-confidence 0.99 \
  --verbose \
  --print-final-state
```

**Using a token file:**

```sh
echo "your_token_here" > ~/.qiskit/ibm_quantum_token
chmod 600 ~/.qiskit/ibm_quantum_token
cqam-run program.cqam --backend ibm --qpu-device ibm_brisbane
```

**Live integration test (requires `IBM_QUANTUM_TOKEN` in environment):**

```sh
QISKIT_C_DIR=/path/to/qiskit/dist/c \
IBM_QUANTUM_TOKEN=your_token_here \
cargo test --features ibm -- --ignored test_ibm_backend_gate_set
```

---

## 5. Execution Pipeline

When a CQAM program runs with `--backend ibm`, the following sequence executes
on each `QOBSERVE` instruction.

### 5.1 Step-by-Step Flow

**Step 1 — VM execution of classical and quantum instructions.**

The CQAM VM (`cqam-vm`) executes instructions normally. Classical registers
(R, F, Z, H) and CMEM operate identically to simulation mode. Quantum
instructions (`QPREP`, `QHADM`, `QCNOT`, `QROT`, etc.) are intercepted by the
`CircuitBackend` rather than being executed against a statevector or density
matrix.

**Step 2 — CircuitBackend buffers quantum operations.**

`CircuitBackend` (`cqam-sim/src/circuit_backend.rs`) accumulates quantum gate
operations into an intermediate `native_ir::Circuit`. No computation is
performed yet: gates are stored as typed IR nodes (`Op::Gate1q`, `Op::Gate2q`,
`Op::Measure`, etc.).

**Step 3 — On `QOBSERVE`: flush the circuit buffer.**

When the VM executes `QOBSERVE`, the `CircuitBackend` calls
`IbmQpuBackend::submit`. At this point the accumulated `native_ir::Circuit` is
passed through the full compilation and submission pipeline described below.

**Step 4 — Qubit allocation check.**

`submit` first verifies that the circuit's `num_physical_qubits` does not
exceed `IbmQpuBackend::max_qubits`. If it does, a
`CqamError::QpuQubitAllocationFailed` is returned immediately:

```
Runtime error: QPU qubit allocation failed: required 200, available 127
```

**Step 5 — Convert `native_ir::Circuit` to `QkCircuit`.**

The function `native_to_qk` (`cqam-qpu-ibm/src/convert.rs`) walks the native
IR and builds a `QkCircuit` by calling into the Qiskit C API:

- `qk_circuit_new(num_qubits, num_clbits)` allocates the opaque handle.
- `qk_circuit_gate(...)` appends each single- and two-qubit gate using the
  `QkGate` constants defined in `ffi.rs` (e.g., `QK_GATE_SX`, `QK_GATE_CX`,
  `QK_GATE_RZ`).
- `qk_circuit_measure(...)` appends measurement operations.

**Step 6 — Transpile via the Qiskit C API.**

`transpile_for_ibm` (`cqam-qpu-ibm/src/transpile.rs`) builds a `QkTarget`
representing the IBM superconducting native gate set
`{SX, X, Rz, Id, CX, Measure, Reset}` and calls `qk_transpile`:

```
qk_transpile(QkCircuit*, QkTarget*, QkTranspileOptions*, QkTranspileResult*, error**)
```

The `QkTranspileOptions` struct carries:

- `optimization_level`: 0–3 (forwarded from `--ibm-optimization-level`).
- `seed`: -1 (system entropy) unless overridden programmatically.
- `approximation_degree`: 1.0 (no approximation).

When the backend has live calibration data, the target is enriched with
per-qubit error rates and gate durations, enabling the transpiler to make
calibration-aware routing and optimization decisions. This path is available
via `transpile_for_ibm_calibrated` but the current `submit` path uses the
global target (`transpile_for_ibm`) for safety and reproducibility.

If transpilation fails, the transpiler writes a C string to the `error**`
output parameter. The Rust layer captures and frees this string with
`qk_str_free` (not the system `free`) and returns `IbmError::TranspileError`.

**Step 7 — Emit OpenQASM 3.**

`circuit_to_qasm3` (`cqam-qpu-ibm/src/qasm.rs`) walks the transpiled
`QkCircuit` using the instruction enumeration API (`qk_circuit_num_instructions`,
`qk_circuit_get_instruction`) and emits a pure-Rust OpenQASM 3 string:

```openqasm
OPENQASM 3;
include "stdgates.inc";
qubit[5] q;
bit[5] c;

sx q[0];
cx q[0], q[1];
rz(1.5707963267948966) q[2];
c[0] = measure q[0];
```

Parameterized gates emit floating-point angles with 15 significant digits to
preserve full IEEE 754 round-trip fidelity. Trailing zeros are trimmed for
readability, but at least one digit after the decimal point is always retained.
`delay` instructions are silently dropped because the IBM REST API does not
accept them.

**Step 8 — Submit to IBM Quantum Platform v2.**

`IbmRestClient::submit_job` (`cqam-qpu-ibm/src/rest.rs`) posts the QASM string
to `https://api.quantum.ibm.com/api/v1/jobs`:

```json
{
  "program": {
    "qasm": "OPENQASM 3;\n...",
    "shots": 1024
  },
  "backend": "ibm_brisbane"
}
```

The API token is sent as a `Bearer` token in the `Authorization` header.
The response contains a job ID.

**Step 9 — Poll for completion with exponential backoff.**

`IbmRestClient::poll_until_done` polls `GET /api/v1/jobs/{id}` until the
status is `COMPLETED` or `DONE`. Between polls, it sleeps with an initial
interval of 2 seconds, growing by a factor of 1.5 per poll, capped at 30
seconds. The default timeout is 600 seconds.

Terminal failure statuses (`FAILED`, `CANCELLED`, `ERROR`) return immediately
with `IbmError::UnexpectedStatus`. A timeout returns `IbmError::Timeout`.

**Step 10 — Parse result counts.**

The job result response contains a `counts` map of hex bitstrings to shot
counts, e.g. `{"0x0": 512, "0x3": 512}`. `parse_counts` strips the `0x`
prefix and converts each key to `u64`, producing a `BTreeMap<u64, u32>`.

**Step 11 — Adaptive shot loop and convergence.**

The shot loop in `IbmQpuBackend::submit` does not necessarily submit all
`shot_budget` shots at once. Instead, it iterates:

1. Submit a batch of `convergence.min_batch_size` shots (or the remaining
   budget, whichever is smaller).
2. Pass the batch counts to `BayesianEstimator::update`.
3. If `BayesianEstimator::is_converged` returns true, stop early.
4. Otherwise, if budget remains, submit another batch.

The QASM string is generated once before the loop; only the shot count per
batch varies. Transpilation is expensive and is not repeated across batches.

**Step 12 — Write result to VM registers.**

The finalized count distribution is returned as `RawResults` to the
`CircuitBackend`, which converts it to a `HybridValue::Dist` (or `Hist`) and
stores it in the appropriate H register. The CQAM program resumes execution
from the instruction following `QOBSERVE`.

---

## 6. Configuration Details

### 6.1 Transpiler Optimization Levels

The Qiskit transpiler accepts levels 0 through 3:

| Level | Effect |
|---|---|
| 0 | Basis translation and connectivity mapping only. No optimization passes. Fastest compilation, lowest circuit quality. |
| 1 | Layout and routing with basic gate cancellation. Default. Balances compile time and circuit depth. |
| 2 | Full optimization including commutation-based cancellation. Uses gate durations from calibration if available. |
| 3 | Aggressive noise-adaptive optimization. Uses per-qubit error rates to minimize expected infidelity. Slowest compilation. |

Values greater than 3 are clamped to 3 with a warning printed to stderr.

### 6.2 HTTP Retry Policy

All REST operations use exponential backoff via `request_with_retry`. The
default `RetryPolicy` is:

| Parameter | Value |
|---|---|
| `max_retries` | 5 |
| `initial_backoff` | 1 second |
| `max_backoff` | 60 seconds |
| `backoff_multiplier` | 2.0x |

Retries trigger on:
- HTTP 429 (Too Many Requests): respects `Retry-After` header if present and
  within 300 seconds; otherwise uses the exponential backoff value.
- HTTP 502, 503, 504: server-side transient errors.
- TCP connection failures and read/write timeouts (via `reqwest::Error::is_connect`
  and `is_timeout`).

All other HTTP status codes and errors are returned immediately without retry.

### 6.3 Polling Backoff

Job status polling uses a separate, gentler backoff from the retry policy:

| Parameter | Value |
|---|---|
| Initial interval | 2 seconds |
| Backoff multiplier | 1.5x per poll |
| Maximum interval | 30 seconds |
| Total timeout | 600 seconds |

The interval reaches the 30-second cap after approximately 8 polls
(2 → 3 → 4.5 → 6.75 → ... → 30).

### 6.4 Device Auto-Discovery

`IbmQpuBackend::from_device` calls `GET /api/v1/backends/{name}/configuration`
to fetch the device's coupling map and qubit count. The coupling map is a list
of directed `[control, target]` pairs; `ConnectivityGraph::from_edges`
normalizes these to undirected edges with deduplication.

To enumerate all available devices:

```rust
let client = IbmRestClient::new(token, "ibm_brisbane");
let backends: Vec<BackendInfo> = client.list_backends()?;
```

Each `BackendInfo` contains `name`, `num_qubits`, `status` (e.g. `"online"`,
`"offline"`, `"maintenance"`), and `simulator` (bool).

### 6.5 Calibration

Calibration data is fetched on startup by `build_ibm_qpu` in `runner.rs`:

```rust
if let Err(e) = backend.refresh_calibration() {
    eprintln!("warning: could not fetch IBM calibration for '{}': {}. \
               Using synthetic defaults.", device_name, e);
}
```

`refresh_calibration` calls `GET /api/v1/backends/{name}/properties` and
parses the response into `IbmCalibrationData`. Extraction rules:

- **T1, T2**: read from the `qubits` array; units (`s`, `us`, `ms`, `ns`,
  `µs`) are normalized to seconds.
- **Single-qubit gate error**: `sx` error takes priority over `x`, which takes
  priority over `id`. This matches IBM's own recommendation for representative
  single-qubit error rates.
- **Two-qubit gate error**: read from `cx` or `ecr` gate entries (both are
  supported; newer IBM Eagle and Heron devices use ECR).
- **Gate times**: the last `sx`/`x` `gate_length` seen sets the single-qubit
  gate time; the last `cx`/`ecr` `gate_length` sets the two-qubit gate time.
- **Missing properties**: default to `f64::NAN`. If no gate times appear in
  the response, synthetic defaults are used (35 ns single-qubit, 660 ns
  two-qubit — typical IBM Falcon/Heron values).

If `refresh_calibration` fails (network error, authentication error, or parse
error), the existing calibration (initialized to synthetic defaults) is
preserved unchanged. The failure is best-effort: execution continues.

### 6.6 Convergence Criterion

The `ConvergenceCriterion` passed to `CircuitBackend` controls when the
adaptive shot loop terminates early:

| Field | Default | Description |
|---|---|---|
| `confidence` | 0.95 | Bayesian confidence level for convergence (from `--qpu-confidence`) |
| `max_relative_error` | (BayesianEstimator default) | Maximum acceptable relative error in the estimated distribution |
| `min_batch_size` | (BayesianEstimator default) | Minimum shots per REST submission |

The `confidence` field is set from `--qpu-confidence`. The remaining fields
use `ConvergenceCriterion::default()`. Convergence is checked by
`BayesianEstimator::is_converged` after each batch; the loop exits as soon as
convergence is reached or the `shot_budget` is exhausted.

---

## 7. Troubleshooting

### `error: could not find native library 'qiskit'`

The `QISKIT_C_DIR` environment variable is not set, or points to a directory
that does not contain `lib/libqiskit.dylib` (macOS) or `lib/libqiskit.so`
(Linux). Verify:

```sh
ls $QISKIT_C_DIR/lib/
# Expected: libqiskit.dylib (macOS) or libqiskit.so (Linux)
```

If the file exists but the binary still fails to launch, the dynamic library
search path is not configured:

```sh
# macOS
export DYLD_LIBRARY_PATH=$QISKIT_C_DIR/lib:$DYLD_LIBRARY_PATH

# Linux
export LD_LIBRARY_PATH=$QISKIT_C_DIR/lib:$LD_LIBRARY_PATH
```

### `IBM backend not available. Rebuild with: cargo build --features ibm`

The `cqam-run` binary was built without the `ibm` feature flag. Rebuild:

```sh
QISKIT_C_DIR=/path/to/qiskit/dist/c cargo build --features ibm -p cqam-run
```

### `IBM Quantum token not found`

No token was found at any of the three resolution levels. Provide one via:

```sh
export IBM_QUANTUM_TOKEN=your_token_here
# or
cqam-run program.cqam --backend ibm --ibm-token your_token_here
# or
echo "your_token_here" > ~/.qiskit/ibm_quantum_token
```

### `IBM backend initialization failed for 'ibm_brisbane': ...`

The `from_device` call to `GET /api/v1/backends/ibm_brisbane/configuration`
failed. Common causes:

- Invalid or expired API token (HTTP 401 or 403).
- Device name misspelled: check with `list_backends` (see Section 6.4).
- Network connectivity issue: the REST client retries 5 times before failing.
- IBM Quantum Platform outage: check https://quantum.ibm.com/services.

### `QPU qubit allocation failed: required N, available M`

The CQAM program requested more physical qubits than the target device has.
Options:

- Select a larger device with `--qpu-device <name>`.
- Reduce the qubit count in the CQAM program or use the `#! qubits N` pragma
  with a smaller value.
- Use `--backend simulation` for programs that exceed real hardware limits.

### `TranspileError: ...`

The Qiskit transpiler returned a non-zero exit code. The error message from the
C API is forwarded verbatim. Common causes:

- The input circuit contains operations not in the IBM native gate set after
  conversion. Verify that `native_to_qk` handles all gate types in the program.
- The optimization level is too aggressive for the circuit structure. Try
  `--ibm-optimization-level 0` to isolate whether the issue is in the optimizer.
- Very deep circuits may trigger internal Qiskit DAG errors (`QK_EXIT_DAG_ERROR`,
  code 500).

### Job status `FAILED`, `CANCELLED`, or `ERROR`

IBM returned a terminal failure for the submitted job. This is typically caused
by:

- The submitted QASM is invalid or uses gates not supported by the device.
- The device went offline between job submission and execution.
- The job exceeded the device's maximum circuit depth or shot count limits.

The full IBM job ID is included in `IbmError::UnexpectedStatus` for manual
inspection via the IBM Quantum dashboard at https://quantum.ibm.com/jobs.

### Job timeout after 600 seconds

`poll_until_done` timed out waiting for the job to complete. IBM queue wait
times can be long on busy devices. Options:

- Retry later when the device queue is shorter.
- Use `--qpu-device ibm_qasm_simulator` for shorter queue times (if your
  account has simulator access).
- The timeout is currently hardcoded to 600 seconds; this is not yet a CLI
  parameter.

### `--noise is not compatible with QPU backends`

Noise injection operates at the simulation layer and has no meaning for real
hardware execution. Remove `--noise` when using `--backend ibm`.

---

## 8. Architecture Overview

### 8.1 Crate Dependency Diagram

```
cqam-run (binary, optional feature: ibm)
  |
  +-- cqam-vm          (instruction executor, HFORK/HMERGE)
  |     |
  |     +-- cqam-core  (ISA, types, native_ir, error types)
  |
  +-- cqam-sim         (SimulationBackend, CircuitBackend, noise layer)
  |     |
  |     +-- cqam-core
  |
  +-- cqam-qpu         (MockQpuBackend, QpuBackend trait, BayesianEstimator,
  |     |               ConvergenceCriterion, CalibrationData trait)
  |     +-- cqam-core
  |
  +-- cqam-qpu-ibm     (IbmQpuBackend, IbmRestClient, transpile, qasm emitter)
        |               [compiled only with --features ibm]
        |
        +-- cqam-core
        +-- cqam-qpu
        +-- libqiskit   (Qiskit C API, dynamic library, external)
```

### 8.2 Crate Responsibilities

| Crate | Responsibility |
|---|---|
| `cqam-core` | ISA definition, `native_ir::Circuit` IR, error types, connectivity types |
| `cqam-vm` | Instruction fetch-execute loop, HFORK/HMERGE, ISR dispatch |
| `cqam-sim` | Statevector and density-matrix quantum simulation; `CircuitBackend` that implements `QuantumBackend` by accumulating ops |
| `cqam-qpu` | QPU backend trait (`QpuBackend`), mock backend, `BayesianEstimator`, `ConvergenceCriterion`, `CalibrationData` trait |
| `cqam-qpu-ibm` | IBM-specific: REST client, FFI bindings to `libqiskit`, transpilation, OpenQASM 3 emission, calibration parsing |
| `cqam-run` | CLI argument parsing, config loading, backend selection and wiring, token resolution |

### 8.3 Feature Flag Boundaries

The `ibm` feature in `cqam-run` gates exactly one dependency: `cqam-qpu-ibm`.
All IBM-specific code is isolated inside that crate or behind `#[cfg(feature = "ibm")]`
guards in `runner.rs`. The rest of the workspace compiles identically with or
without the feature.

This boundary means that the `libqiskit` dynamic library is only required when:

1. `cqam-qpu-ibm` is compiled (always, as a workspace member), AND
2. The resulting binary is launched (link-time search happens at runtime for
   dynamic libraries on macOS and Linux).

Build-only verification of the workspace without a Qiskit installation is not
currently possible because `cqam-qpu-ibm` always links against `libqiskit`,
even when `cqam-run` is built without `--features ibm`.
