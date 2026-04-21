# CQAM IBM QPU Backend Reference

This document describes how to build, configure, and use the CQAM IBM Quantum
Platform integration. The integration compiles CQAM programs to OpenQASM 2.0
circuits and submits them to real IBM quantum hardware via the IBM Quantum
Platform v2 REST API using the Sampler primitive. It is gated behind an
optional Cargo feature flag and requires a linked native Qiskit C library for
circuit transpilation.

---

## 1. Prerequisites

### 1.1 Qiskit C API

The transpiler path calls into the Qiskit C API (`libqiskit`). The
`cqam-qpu-ibm` build script resolves the library location using the following
precedence:

1. **`QISKIT_C_DIR` environment variable** — explicit override pointing at a
   directory that contains `lib/libqiskit.{dylib,so}` and
   `include/qiskit/*.h`.
2. **`$HOME/.local/qiskit/dist/c`** — user-local canonical location. If
   absent, `build.rs` clones the upstream Qiskit repository into
   `$HOME/.local/qiskit` and runs `make c` there as part of the cargo
   build. Subsequent builds reuse the existing tree. No sudo required —
   `~/.local` is user-owned on macOS and Linux.

**Repository:** https://github.com/Qiskit/qiskit

**Typical first build:** nothing to prepare. Just run

```sh
cargo build --features ibm -p cqam-run
```

The build script will:

1. Check `$HOME/.local/qiskit/dist/c/lib` — if present, link it and stop.
2. Otherwise clone `https://github.com/Qiskit/qiskit.git` (shallow, default
   branch `main`) into `$HOME/.local/qiskit` and run `make c`.
3. Emit `cargo:rustc-link-search=native=$HOME/.local/qiskit/dist/c/lib` and
   `cargo:rustc-link-lib=dylib=qiskit`.

Requirements on `PATH`: `git`, `make`, `python3`, and a working Rust
toolchain. The first build downloads the Qiskit sources and compiles them,
which may take several minutes. Subsequent builds are instant.

**Custom location:**

```sh
export QISKIT_C_DIR=/path/to/qiskit/dist/c
```

Takes precedence over the default — useful for shared/system installs or
when building from a branch.

**Knobs:**

- `QISKIT_GIT_REV` — upstream revision (default `main`).
- `CQAM_NO_QISKIT_BUILD=1` — disable the automatic clone+build and fail
  fast when the library is missing.

### 1.2 IBM Quantum Account and API Token

An IBM Quantum account with access to at least one real backend is required.
Obtain your API key (not a legacy token) from https://quantum.cloud.ibm.com/account.

The API key undergoes a two-step exchange before use: it is exchanged for an
IAM bearer token via IBM Cloud, and the runner auto-discovers the Service CRN
required by the IBM Quantum REST API. This process is handled automatically by
the runner at startup (see Section 3.2).

---

## 2. Building with IBM Support

The IBM backend is an optional feature of `cqam-run`. It is not compiled by
default so that users without the Qiskit C dependency are not affected.

**Build with IBM support:**

```sh
cargo build --features ibm -p cqam-run
```

**Build only the IBM crate (for development):**

```sh
cargo build -p cqam-qpu-ibm
```

**Run tests (including the FFI layer):**

```sh
cargo test --features ibm --workspace
```

Prepend `QISKIT_C_DIR=/path/to/qiskit/dist/c` to any of the above to
override the default `~/.local/qiskit/dist/c`.

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
# macOS
export DYLD_LIBRARY_PATH=$HOME/.local/qiskit/dist/c/lib:$DYLD_LIBRARY_PATH

# Linux
export LD_LIBRARY_PATH=$HOME/.local/qiskit/dist/c/lib:$LD_LIBRARY_PATH
```

If `QISKIT_C_DIR` is set, substitute `$QISKIT_C_DIR/lib` instead.

---

## 3. Authentication

### 3.1 Three-Level Token Resolution

The runner resolves the IBM API key through the following precedence chain,
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

### 3.2 Two-Step Authentication Flow

IBM Quantum Platform requires more than a bare Bearer token in API requests.
The runner performs two steps automatically at startup inside `IbmRestClient::new`:

**Step 1 — IAM token exchange.**
The API key is exchanged for a short-lived IAM bearer token:

```
POST https://iam.cloud.ibm.com/identity/token
Content-Type: application/x-www-form-urlencoded

grant_type=urn:ibm:params:oauth:grant-type:apikey&apikey=<YOUR_KEY>
```

The response contains `access_token`, which is stored as the bearer token for
all subsequent IBM Quantum API calls.

**Step 2 — Service CRN auto-discovery.**
IBM Quantum REST endpoints require a `Service-CRN` header identifying the
specific quantum service instance in your account. The runner fetches all
resource instances and filters for the one whose CRN contains
`"quantum-computing"`:

```
GET https://resource-controller.cloud.ibm.com/v2/resource_instances
Authorization: Bearer <iam_token>
```

The CRN is extracted from the first matching instance and injected into every
subsequent IBM Quantum API request as the `Service-CRN` header.

**Per-request headers** (injected by `authed_get`/`authed_post`):

```
Authorization: Bearer <iam_token>
Service-CRN: <crn>
Accept: application/json
User-Agent: cqam-qpu-ibm/0.1
```

The `Accept: application/json` header is required to avoid 403 responses from
the Cloudflare-fronted API endpoints.

### 3.3 Security

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
| `--ibm-token <TOKEN>` | string | (see Section 3) | IBM Quantum API key |
| `--qpu-device <NAME>` | string | `ibm_torino` | Target device name |
| `--ibm-optimization-level <N>` | u8, 0-3 | `1` | Qiskit transpiler optimization level |
| `--qpu-shots <N>` | u32 | `8192` | Total shot budget for the execution |
| `--qpu-confidence <F>` | f64, 0.0-1.0 | `0.95` | Bayesian convergence confidence level |
| `--qpu-timeout <secs>` | u64 | `1800` | Job polling timeout in seconds |

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
export IBM_QUANTUM_TOKEN=your_api_key_here
cqam-run program.cqam --backend ibm
```

**Specifying all relevant options:**

```sh
cqam-run program.cqam \
  --backend ibm \
  --ibm-token your_api_key_here \
  --qpu-device ibm_torino \
  --ibm-optimization-level 2 \
  --qpu-shots 8192 \
  --qpu-confidence 0.99 \
  --qpu-timeout 3600 \
  --verbose \
  --print-final-state
```

**Using a token file:**

```sh
echo "your_api_key_here" > ~/.qiskit/ibm_quantum_token
chmod 600 ~/.qiskit/ibm_quantum_token
cqam-run program.cqam --backend ibm
```

**Live integration test (requires `IBM_QUANTUM_TOKEN` in environment):**

```sh
IBM_QUANTUM_TOKEN=your_api_key_here \
cargo test --features ibm -- --ignored test_ibm_backend_gate_set
```

---

## 5. Execution Pipeline

When a CQAM program runs with `--backend ibm`, the following sequence executes
on each `QOBSERVE` instruction. Each quantum section (the instructions between
one `QPREP` and one `QOBSERVE`) constitutes one IBM job submission.

### 5.1 Step-by-Step Flow

**Step 1 — VM execution of classical and quantum instructions.**

The CQAM VM (`cqam-vm`) executes instructions normally. Classical registers
(R, F, Z, H) and CMEM operate identically to simulation mode. Quantum
instructions (`QPREP`, `QHADM`, `QCNOT`, `QROT`, `QPREPSM`, etc.) are
intercepted by the `CircuitBackend` rather than being executed against a
statevector or density matrix.

**Step 2 — CircuitBackend buffers quantum operations.**

`CircuitBackend` (`cqam-sim/src/circuit_backend.rs`) accumulates quantum gate
operations into an intermediate `MicroProgram`. No computation is performed
yet: gates are stored as typed IR nodes (`Op::Gate1q`, `Op::Gate2q`,
`Op::PrepProduct`, etc.). Arbitrary 2x2 unitary matrices from `apply_single_gate`
are recognized against known gate matrices (H, X, Y, Z, S, Sdg, T, Tdg with
tolerance 1e-8); unrecognized gates are decomposed to `Gate1q::U3` via ZYZ
Euler decomposition.

**Step 3 — On `QOBSERVE`: flush the circuit buffer.**

When the VM executes `QOBSERVE`, the `CircuitBackend` calls
`IbmQpuBackend::submit`. At this point the accumulated `MicroProgram` is
passed through the full compilation and submission pipeline described below.

**Step 4 — Qubit allocation check.**

`submit` first verifies that the circuit's `num_physical_qubits` does not
exceed `IbmQpuBackend::max_qubits`. If it does, a
`CqamError::QpuQubitAllocationFailed` is returned immediately:

```
Runtime error: QPU qubit allocation failed: required 200, available 133
```

**Step 5 — Five-stage compilation pipeline.**

The `MicroProgram` passes through the `cqam-micro` pipeline:

1. **Decompose** (`decompose_to_standard`): kernel ops and `PrepProduct` ops
   are expanded to the standard gate set {H, X, Y, Z, S, Sdg, T, Tdg, Rx, Ry,
   Rz, CX, CZ, SWAP}. Key decompositions:
   - `Gate1q::U3(theta, phi, lambda)` from QPREPS/QPREPSM and ZYZ decomposition
     passes through as-is to the native map stage.
   - MCZ (multi-controlled-Z) for n >= 4 qubits uses a relative-phase Toffoli
     V-chain (ancilla-free O(n) gate count), not the exponential diagonal/WHT
     path. MCZ(16) produces ~100 gates rather than the previous 917,508.
   - Cyclic shift permutations are recognized structurally and decomposed in
     O(p^2) gates using an increment/decrement circuit, rather than the generic
     O(2^p) transposition path. A 10-qubit quantum walk produces ~6,800 CX gates
     rather than the previous ~41 million.
2. **Route** (`route`): virtual-to-physical qubit assignment with BFS SWAP
   insertion for constrained topologies.
3. **Native map** (`map_to_native`): standard gates are translated to the IBM
   superconducting native set {SX, X, Rz, CX}. `Gate1q::U3(t, p, l)` maps to
   `Rz(l) . SX . Rz(t+pi) . SX . Rz(p+pi)` (5 native gates).
4. **Optimize**: gate cancellation (future work; currently a pass-through).
5. **Cache**: circuits sharing a `structure_key` (same topology, potentially
   different parameter values) skip recompilation on repeated execution.

**Step 6 — Convert `native_ir::Circuit` to `QkCircuit`.**

The function `native_to_qk` (`cqam-qpu-ibm/src/convert.rs`) walks the native
IR and builds a `QkCircuit` by calling into the Qiskit C API:

- `qk_circuit_new(num_qubits, num_clbits)` allocates the opaque handle.
- `qk_circuit_gate(...)` appends each gate using the `QkGate` constants
  (e.g., `QK_GATE_SX`, `QK_GATE_CX`, `QK_GATE_RZ`).
- `qk_circuit_measure(...)` appends measurement operations.

**Step 7 — Transpile via the Qiskit C API.**

`transpile_for_ibm` (`cqam-qpu-ibm/src/transpile.rs`) builds a `QkTarget`
representing the device's native gate set and connectivity. The target is
populated with per-edge CX/CZ properties and per-qubit error rates from live
calibration data (when available):

```
qk_transpile(QkCircuit*, QkTarget*, QkTranspileOptions*, QkTranspileResult*, error**)
```

The `QkTranspileOptions` struct carries:

- `optimization_level`: 0–3 (forwarded from `--ibm-optimization-level`).
- `seed`: -1 (system entropy) unless overridden programmatically.
- `approximation_degree`: 1.0 (no approximation).

Device connectivity is provided as directed edge pairs from the backend's
coupling map. NaN error values from incomplete calibration data are replaced
with a 1e-2 fallback before being passed to the transpiler, which prevents
routing failures caused by NaN propagation in the transpiler's cost model.

If transpilation fails, the transpiler writes a C string to the `error**`
output parameter. The Rust layer captures and frees this string with
`qk_str_free` and returns `IbmError::TranspileError`.

**Step 8 — Emit OpenQASM 2.0.**

`circuit_to_qasm3` (`cqam-qpu-ibm/src/qasm.rs`) walks the transpiled
`QkCircuit` using the instruction enumeration API and emits an OpenQASM 2.0
string. Despite the function name, the emitter produces OpenQASM 2.0 because
the IBM Sampler primitive accepts only 2.0 syntax:

```
OPENQASM 2.0;
include "qelib1.inc";
qreg q[5];
creg c[5];

sx q[0];
cx q[0], q[1];
rz(1.570796326794897) q[2];
measure q[0] -> c[0];
```

Parameterized gates emit floating-point angles with 15 significant digits to
preserve full IEEE 754 round-trip fidelity. Trailing zeros are trimmed for
readability, but at least one digit after the decimal point is always retained.
`delay` instructions (transpiler scheduling hints) are silently dropped because
the IBM REST API does not accept them.

Parameter values are extracted from transpiled gates via `qk_param_as_real(param)
-> f64`; the `QkParam` type is otherwise opaque in the C API.

**Step 9 — Submit to IBM Quantum Platform v2 (Sampler primitive).**

`IbmRestClient::submit_job` (`cqam-qpu-ibm/src/rest.rs`) posts the QASM string
to `https://quantum.cloud.ibm.com/api/v1/jobs` using the Sampler v2 primitive
format with PUBs (Primitive Unified Blocs):

```json
{
  "program_id": "sampler",
  "backend": "ibm_torino",
  "params": {
    "pubs": [["OPENQASM 2.0; ..."]],
    "version": 2,
    "options": { "default_shots": 8192 }
  }
}
```

The response contains a job ID.

**Step 10 — Poll for completion with exponential backoff.**

`IbmRestClient::poll_until_done` polls `GET /api/v1/jobs/{id}` until the
status is `COMPLETED` or `DONE`. Between polls, it sleeps with an initial
interval of 2 seconds, growing by a factor of 1.5 per poll, capped at 30
seconds. The default timeout is 1800 seconds (configurable via `--qpu-timeout`).

Terminal failure statuses (`FAILED`, `CANCELLED`, `ERROR`) return immediately
with `IbmError::UnexpectedStatus`. A timeout returns `IbmError::Timeout`.

**Step 11 — Adaptive shot loop and convergence.**

The shot loop in `IbmQpuBackend::submit` does not necessarily submit all
`shot_budget` shots at once. Instead, it iterates:

1. Submit a batch of `convergence.min_batch_size` shots (or the remaining
   budget, whichever is smaller).
2. Pass the batch counts to `BayesianEstimator::update`.
3. If `BayesianEstimator::is_converged` returns true, stop early.
4. Otherwise, if budget remains, submit another batch.

The QASM string is generated once before the loop and the transpilation is not
repeated across batches. Only the shot count per batch varies. In practice, the
adaptive convergence criterion (Dirichlet-Multinomial Bayesian estimator, 95%
confidence, 5% maximum relative error) achieves convergence with 40–60% fewer
shots than a fixed-count baseline on typical distributions.

**Step 12 — Parse results and write to VM registers.**

The job result response contains per-shot bitstrings; `result_to_counts()`
aggregates hex-encoded samples (`"0x0"`, `"0x3"`, ...) across all PUBs into a
`BTreeMap<u64, u32>`. The finalized count distribution is returned as
`RawResults` to the `CircuitBackend`, which converts it to a `HybridValue::Dist`
(or `Hist` for multi-shot observation) and stores it in the appropriate H
register. The CQAM program resumes execution from the instruction following
`QOBSERVE`.

---

## 6. New Instructions for Hardware Compatibility

### 6.1 QPREPS — Register-Direct Product State Preparation

```
QPREPS Qdst, Z_start, count
```

Prepares `count` qubits of register `Qdst` in an arbitrary product state. Each
qubit `i` is independently placed in state `alpha_i|0> + beta_i|1>` by reading
the complex amplitudes `(alpha_i, beta_i)` from Z-register pairs:

- Z[start + 2*i] = alpha_i
- Z[start + 2*i + 1] = beta_i

Maximum 4 qubits (consumes 8 Z-registers; Z0–Z7). Requires a prior
`QPREP Qdst, ZERO`.

**Gate cost:** O(n) — one U3(theta, phi, lambda) per qubit, where:
- `theta = 2 * arccos(|alpha|)`
- `phi = arg(beta)`
- `lambda = -arg(alpha)`

Each U3 decomposes to 5 native gates (Rz, SX, Rz, SX, Rz).

**PSW flags:** `sf` = any beta nonzero; `norm_warn` set if any amplitude pair
required normalization (auto-corrected); `trap_arith` set and execution halts
if any (alpha, beta) = (0, 0).

### 6.2 QPREPSM — CMEM-Indirect Product State Preparation

```
QPREPSM Qdst, Rbase, Rcount
```

Prepares `R[Rcount]` qubits of register `Qdst` from amplitude data stored in
classical memory. For each qubit `i`, reads four CMEM cells starting at
`R[Rbase] + 4*i`:

- CMEM[base + 4*i + 0] = re(alpha_i) as f64::to_bits() stored in i64
- CMEM[base + 4*i + 1] = im(alpha_i)
- CMEM[base + 4*i + 2] = re(beta_i)
- CMEM[base + 4*i + 3] = im(beta_i)

Supports arbitrary qubit counts (limited only by CMEM size). Same U3
decomposition and PSW logic as QPREPS.

### 6.3 The `.qstate` Assembler Directive

The `.qstate` directive provides a convenient way to declare normalized
single-qubit amplitude data in `.data` sections:

```
.data
amps:
    .qstate 0.707106781, 0.0, 0.707106781, 0.0   # |+>: alpha=(0.707,0), beta=(0.707,0)
    .qstate 1.0, 0.0, 0.0, 0.0                    # |0>: alpha=(1,0),   beta=(0,0)
    .qstate 0.0, 0.0, 1.0, 0.0                    # |1>: alpha=(0,0),   beta=(1,0)
```

Each `.qstate` line produces 4 consecutive CMEM cells in the format expected by
QPREPSM. The assembler validates normalization at assembly time and emits an
error if `|alpha|^2 + |beta|^2` deviates from 1.0 by more than 1e-10.

### 6.4 QENCODE Deprecation

`QENCODE` remains in the ISA and continues to work on `SimulationBackend`.
The `CircuitBackend` rejects it with `QpuUnsupportedOperation` because arbitrary
statevector preparation has O(2^n) gate cost and cannot be efficiently executed
on hardware. New code targeting hardware compatibility should use QPREPS or
QPREPSM instead. ISA documentation marks QENCODE as simulation-only.

### 6.5 Reference Program

```
#! qubits 3
.data
    .org 500
amps:
    .qstate 0.707106781, 0.0, 0.707106781, 0.0   # qubit 0: |+>
    .qstate 1.0, 0.0, 0.0, 0.0                    # qubit 1: |0>
    .qstate 0.0, 0.0, 1.0, 0.0                    # qubit 2: |1>

.code
    QPREP Q0, 0                # allocate 3-qubit zero state (ZERO = dist_id 0... but use QPREP with count)
    ILDI R0, 500               # CMEM base address
    ILDI R1, 3                 # qubit count
    QPREPSM Q0, R0, R1         # encode product state
    QOBSERVE H0, Q0            # measure
    HALT
```

Expected: outcomes 4 (`|100>`) and 5 (`|101>`) each at approximately 50%.

---

## 7. Decomposition Optimizations

Two decomposition improvements implemented in the `cqam-micro` pipeline
significantly reduce circuit sizes for large algorithms, enabling programs that
previously caused out-of-memory failures or IBM HTTP errors to execute
successfully.

### 7.1 Relative-Phase Toffoli V-Chain for Multi-Controlled-Z

Multi-controlled-Z (MCZ) gates on n >= 4 qubits previously used a diagonal
unitary / Walsh-Hadamard synthesis path with CX gate count:

```
(n - 2) * 2^n + 4
```

For n = 16 this is 917,508 CX gates per MCZ call. The Grover oracle and
diffusion operator each contain one MCZ(n), so a 16-qubit Grover circuit with
201 iterations required approximately 395 million CX gates and ~19 GB of memory
— causing an OOM abort at about 8.6 GB.

The replacement uses an ancilla-free relative-phase Toffoli chain
(Maslov 2016). For an n-qubit MCZ:

```
H(w[n-1])
for i in 1..(n-1):
    RelToffoli(w[i-1], w[i], w[i+1])
for i in (1..(n-1)).rev():
    RelToffoli_adj(w[i-1], w[i], w[i+1])
H(w[n-1])
```

Gate count: 2*(n-2) relative-phase Toffoli gates = 6*(n-2) + 2 CX gates.
For n = 16: approximately 86 CX gates.

| Program | Before (CX per MCZ) | After (CX per MCZ) | Reduction |
|---|---|---|---|
| MCZ(16) | 917,508 | ~86 | ~10,700x |
| grover_16q (201 iterations) | ~368 million total | ~18,000 total | ~20,000x |
| quantum_counting (15 iterations) | ~27 million total | ~1,300 total | ~20,000x |

### 7.2 Cyclic Shift Recognition for Permutation Circuits

The permutation decomposition previously treated every permutation as a generic
table of transpositions, decomposing each transposition into multi-controlled-X
gates via the diagonal path. For a shift permutation on p position qubits,
this produced O(p * 4^p) gates.

The updated decomposer recognizes cyclic shifts (increment and decrement by 1
on a register) and emits a known O(p^2) increment/decrement circuit:

```
# Cyclic increment on p qubits:
for i in (0..p-1).rev():
    MCX(qubits[0..i], target=qubits[i+1])
```

Each MCX uses the V-chain decomposition (O(i) Toffoli gates), giving O(p^2)
total.

| Configuration | Before | After | Reduction |
|---|---|---|---|
| quantum_walk_permutation, 10 qubits, 10 steps | ~41 million ops | ~6,800 ops | ~6,000x |

---

## 8. Configuration Details

### 8.1 Transpiler Optimization Levels

The Qiskit transpiler accepts levels 0 through 3:

| Level | Effect |
|---|---|
| 0 | Basis translation and connectivity mapping only. No optimization passes. Fastest compilation, lowest circuit quality. |
| 1 | Layout and routing with basic gate cancellation. Default. Balances compile time and circuit depth. |
| 2 | Full optimization including commutation-based cancellation. Uses gate durations from calibration if available. |
| 3 | Aggressive noise-adaptive optimization. Uses per-qubit error rates to minimize expected infidelity. Slowest compilation. |

Values greater than 3 are clamped to 3 with a warning printed to stderr.

### 8.2 HTTP Retry Policy

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

### 8.3 Polling Backoff and Timeout

Job status polling uses a separate, gentler backoff from the HTTP retry policy:

| Parameter | Value |
|---|---|
| Initial interval | 2 seconds |
| Backoff multiplier | 1.5x per poll |
| Maximum interval | 30 seconds |
| Default total timeout | 1800 seconds |

The interval reaches the 30-second cap after approximately 8 polls
(2 → 3 → 4.5 → 6.75 → ... → 30). The total timeout defaults to 1800 seconds
(30 minutes) and can be overridden at the CLI via `--qpu-timeout <secs>`. This
increased default (from an earlier 600s value) accommodates realistic IBM queue
wait times on busy devices.

### 8.4 Device Auto-Discovery

`IbmQpuBackend::from_device` calls `GET /api/v1/backends/{name}/configuration`
to fetch the device's coupling map and qubit count. The coupling map is a list
of directed `[control, target]` pairs; `ConnectivityGraph::from_edges`
normalizes these to undirected edges with deduplication.

To enumerate all available devices, call `list_backends` on an initialized
`IbmRestClient`. Each `BackendInfo` contains `name`, `num_qubits`, `status`
(e.g. `"online"`, `"offline"`, `"maintenance"`), and `simulator` (bool).

### 8.5 Calibration

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
  priority over `id`.
- **Two-qubit gate error**: read from `cx` or `ecr` gate entries (both are
  supported; IBM Eagle uses ECR, Heron uses CZ).
- **Gate times**: the last `sx`/`x` `gate_length` seen sets the single-qubit
  gate time; the last `cx`/`ecr` `gate_length` sets the two-qubit gate time.
- **Missing properties**: default to `f64::NAN`. NaN error values are replaced
  with a 1e-2 fallback before being passed to the Qiskit transpiler (NaN
  propagation causes routing failures in the transpiler's cost model).
- **Synthetic defaults** (used if properties endpoint fails): 35 ns
  single-qubit gate time, 660 ns two-qubit gate time.

If `refresh_calibration` fails, the existing synthetic calibration is preserved
and execution continues.

### 8.6 Convergence Criterion

The `ConvergenceCriterion` passed to `CircuitBackend` controls when the
adaptive shot loop terminates early:

| Field | Default | Description |
|---|---|---|
| `confidence` | 0.95 | Bayesian confidence level for convergence (from `--qpu-confidence`) |
| `max_relative_error` | 0.05 | Maximum acceptable relative error in estimated outcome probabilities |
| `min_batch_size` | 100 | Minimum shots per REST submission |

The `confidence` field is set from `--qpu-confidence`. If the shot budget is
exhausted before convergence, the accumulated results are returned with a
warning (not an error).

---

## 9. Troubleshooting

### `error: could not find native library 'qiskit'`

The build script could not locate `libqiskit`. It searches `QISKIT_C_DIR`
first, then `$HOME/.local/qiskit/dist/c`, and clones + builds into
`$HOME/.local/qiskit` if neither is populated (unless `CQAM_NO_QISKIT_BUILD=1`
is set). Verify one of these exists:

```sh
ls $HOME/.local/qiskit/dist/c/lib/
# or
ls $QISKIT_C_DIR/lib/
# Expected: libqiskit.dylib (macOS) or libqiskit.so (Linux)
```

If the cargo-managed build is running but failing, ensure `git`, `make`, and
`python3` are on `PATH`, and check the build log for the upstream `make c`
error.

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
cargo build --features ibm -p cqam-run
```

### `IBM Quantum token not found`

No token was found at any of the three resolution levels. Provide one via:

```sh
export IBM_QUANTUM_TOKEN=your_api_key_here
# or
cqam-run program.cqam --backend ibm --ibm-token your_api_key_here
# or
echo "your_api_key_here" > ~/.qiskit/ibm_quantum_token
```

### `IBM backend initialization failed for 'ibm_torino': ...`

The `from_device` call to `GET /api/v1/backends/ibm_torino/configuration`
failed. Common causes:

- Invalid or expired API key — the IAM exchange may succeed but the subsequent
  IBM Quantum API calls return HTTP 401 or 403.
- Device name misspelled: check available devices via `list_backends`.
- Network connectivity issue: the REST client retries 5 times before failing.
- IBM Quantum Platform outage: check https://quantum.cloud.ibm.com/services.

### `IAM token exchange failed`

The API key could not be exchanged for an IAM bearer token. This typically
means the API key is invalid, expired, or was copied with trailing whitespace.
Verify the key at https://cloud.ibm.com/iam/apikeys.

### `Service CRN discovery failed`

No quantum service instance was found in the account's resource instances. This
means either the IBM Quantum service is not provisioned in this IBM Cloud
account, or the account's API key does not have permission to list resource
instances. Verify at https://cloud.ibm.com/resources.

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
inspection via the IBM Quantum dashboard at https://quantum.cloud.ibm.com/jobs.

### Job timeout

`poll_until_done` timed out waiting for the job to complete. IBM queue wait
times can be long on busy devices. Options:

- Retry later when the device queue is shorter.
- Increase the timeout with `--qpu-timeout <secs>` (default is 1800).

### `--noise is not compatible with QPU backends`

Noise injection operates at the simulation layer and has no meaning for real
hardware execution. Remove `--noise` when using `--backend ibm`.

---

## 10. Hardware Test Results (IBM Torino, Heron r2, 133 qubits)

The following results were obtained running against `ibm_torino` (IBM Torino,
Heron r2 processor, 133 qubits) using the default optimization level 1 and
4096 shots per section.

| Suite | Total Programs | Pass | Notes |
|---|---|---|---|
| `examples/basic/` | 16 | 16 | All circuits shallow (<50 CX), all pass clean |
| `examples/intermediate/` | 30 | ~28 | 2 deep 16q circuits execute but are noise-dominated |
| `examples/advanced_nothreads/` | 18 | 18 | All hardware-compatible after rewrites (see Section 11) |

The 2 intermediate failures are noise-dominated (not logic failures): the
circuits execute and return counts, but the fidelity is too low for algorithm
correctness. They are not considered execution failures.

---

## 11. Advanced Examples: Hardware Compatibility Summary

All 18 programs in `examples/advanced_nothreads/` were audited and rewritten
for hardware compatibility. The key migration patterns applied were:

- **QENCODE → QPREPSM**: Arbitrary statevector prep replaced with product state
  prep for states with product structure (e.g., |+>, |0>, |1> per-qubit states).
- **QPTRACE → full-register observation**: Partial trace (simulation-only) replaced
  by direct observation of the full register.
- **QOBSERVE AMP (removed) → QOBSERVE DIST + HREDUCE**: The AMP observe mode
  was removed from the ISA (density matrix element extraction is not physically
  realizable on hardware). Programs previously using AMP have been rewritten
  using DIST or PROB mode with appropriate post-processing.

Programs in the audit that are RED (noise-dominated or infeasible on current
hardware) are documented in `design/analysis/ADVANCED_EXAMPLES_AUDIT.md`.
See `reference/HARDWARE_COMPATIBILITY.md` for the full compatibility table.

---

## 12. Architecture Overview

### 12.1 Crate Dependency Diagram

```
cqam-run (binary, optional feature: ibm)
  |
  +-- cqam-vm          (instruction executor, HFORK/HMERGE)
  |     |
  |     +-- cqam-core  (ISA, types, circuit_ir, native_ir, error types)
  |
  +-- cqam-sim         (SimulationBackend, CircuitBackend, noise layer)
  |     |
  |     +-- cqam-core
  |
  +-- cqam-micro       (5-stage compilation pipeline: decompose, route, native
  |     |               map, optimize, cache)
  |     +-- cqam-core
  |
  +-- cqam-qpu         (QpuBackend trait, MockQpuBackend, BayesianEstimator,
  |     |               ConvergenceCriterion, CalibrationData trait)
  |     +-- cqam-core
  |
  +-- cqam-qpu-ibm     (IbmQpuBackend, IbmRestClient, transpile, qasm emitter,
        |               Qiskit C FFI bindings)
        |               [compiled only with --features ibm]
        |
        +-- cqam-core
        +-- cqam-qpu
        +-- libqiskit   (Qiskit C API, dynamic library, external)
```

### 12.2 Crate Responsibilities

| Crate | Responsibility |
|---|---|
| `cqam-core` | ISA definition, `native_ir::Circuit` IR, error types, connectivity types |
| `cqam-vm` | Instruction fetch-execute loop, HFORK/HMERGE, ISR dispatch |
| `cqam-sim` | Statevector and density-matrix quantum simulation; `CircuitBackend` |
| `cqam-micro` | Five-stage compilation pipeline (decompose, route, native map, optimize, cache) |
| `cqam-qpu` | QPU backend trait (`QpuBackend`), mock backend, `BayesianEstimator`, `ConvergenceCriterion` |
| `cqam-qpu-ibm` | IBM-specific: REST client, FFI bindings to `libqiskit`, transpilation, OpenQASM 2.0 emission, calibration parsing |
| `cqam-run` | CLI argument parsing, config loading, backend selection and wiring, token resolution |

### 12.3 Feature Flag Boundaries

The `ibm` feature in `cqam-run` gates exactly one dependency: `cqam-qpu-ibm`.
All IBM-specific code is isolated inside that crate or behind `#[cfg(feature = "ibm")]`
guards in `runner.rs`. The rest of the workspace compiles identically with or
without the feature.

This boundary means that the `libqiskit` dynamic library is required when:

1. `cqam-qpu-ibm` is compiled (always, as a workspace member), AND
2. The resulting binary is launched (link-time search happens at runtime for
   dynamic libraries on macOS and Linux).

Build-only verification of the workspace without a Qiskit installation is not
currently possible because `cqam-qpu-ibm` always links against `libqiskit`,
even when `cqam-run` is built without `--features ibm`.

### 12.4 Key Source Files

| File | Lines | Role |
|---|---|---|
| `cqam-qpu-ibm/src/backend.rs` | 466 | `IbmQpuBackend`: QpuBackend impl, adaptive shot loop, `from_device`, `with_poll_timeout` |
| `cqam-qpu-ibm/src/rest.rs` | 1250 | `IbmRestClient`: IAM auth, CRN discovery, job lifecycle, retry policy, result parsing |
| `cqam-qpu-ibm/src/qasm.rs` | 455 | OpenQASM 2.0 emitter: `circuit_to_qasm3`, `write_f64` |
| `cqam-qpu-ibm/src/ffi.rs` | 354 | Raw FFI bindings: `QkCircuit`, `QkTarget`, `QkGate` constants, `extern "C"` |
| `cqam-qpu-ibm/src/transpile.rs` | 591 | `QkTarget` builders (global, calibrated, device-specific), 3 transpile entry points |
| `cqam-sim/src/circuit_backend.rs` | 1327 | `CircuitBackend`: gate buffering, `WireAllocator`, gate recognition, ZYZ decompose |
| `cqam-micro/src/decompose/mod.rs` | 1179 | Kernel decomposition dispatch, `PrepProduct` decompose, ZYZ helper |
| `cqam-micro/src/decompose/controlled.rs` | 617 | `add_control` wrapper, Toffoli, relative-phase Toffoli chain for MCZ |
| `cqam-micro/src/native_map.rs` | 408 | Native gate mapping (SX, X, Rz, CX), U3 decompose, depth calculation |
| `cqam-run/src/runner.rs` | — | `resolve_ibm_token`, `build_ibm_qpu`, `with_poll_timeout` wiring |
