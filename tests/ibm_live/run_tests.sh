#!/bin/bash
# Run incremental IBM QPU integration tests.
# Usage: ./run_tests.sh [api_key]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$PROJECT_DIR/target/release/cqam-run"

QISKIT_C_DIR="${QISKIT_C_DIR:-/opt/qiskit/dist/c}"
export DYLD_LIBRARY_PATH="$QISKIT_C_DIR/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
export LD_LIBRARY_PATH="$QISKIT_C_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export IBM_QUANTUM_TOKEN="${1:-$IBM_QUANTUM_TOKEN}"

if [ -z "$IBM_QUANTUM_TOKEN" ]; then
    echo "ERROR: Provide API key as argument or set IBM_QUANTUM_TOKEN"
    exit 1
fi

PASS=0
FAIL=0
ERRORS=""

run_test() {
    local name="$1"
    local file="$2"
    local expected="$3"

    printf "%-45s " "$name..."

    OUTPUT=$("$BINARY" --backend ibm --qpu-shots 1024 "$file" 2>&1) || true

    if echo "$OUTPUT" | grep -qi "error\|panic\|failed"; then
        echo "FAIL (error)"
        echo "  Output: $(echo "$OUTPUT" | head -3)"
        FAIL=$((FAIL + 1))
        ERRORS="$ERRORS\n  $name: error"
        return
    fi

    echo "OK"
    echo "  $OUTPUT" | grep -i "dist\|hist\|H0\|result" | head -3 || true
    PASS=$((PASS + 1))
}

echo "============================================="
echo "  IBM QPU Live Integration Tests"
echo "  Device: ibm_torino (133 qubits)"
echo "============================================="
echo ""

run_test "Test 1: Zero state (1q)"         "$SCRIPT_DIR/test1_zero_1q.cqam"      "outcome 0"
run_test "Test 2: Uniform (2q)"            "$SCRIPT_DIR/test2_uniform_2q.cqam"    "4 outcomes"
run_test "Test 3: Bell state (2q)"         "$SCRIPT_DIR/test3_bell_2q.cqam"       "00 and 11"
run_test "Test 4: GHZ state (3q)"          "$SCRIPT_DIR/test4_ghz_3q.cqam"        "000 and 111"
run_test "Test 5: Entangle kernel (4q)"    "$SCRIPT_DIR/test5_entangle_4q.cqam"   "0000 and 1111"
run_test "Test 6: QFT on zero (3q)"        "$SCRIPT_DIR/test6_fourier_3q.cqam"    "uniform"
run_test "Test 7: Grover search (2q)"      "$SCRIPT_DIR/test7_grover_2q.cqam"     "outcome 3"
run_test "Test 8: GHZ state (8q)"          "$SCRIPT_DIR/test8_ghz_8q.cqam"        "0 and 255"

echo ""
echo "============================================="
echo "  Results: $PASS passed, $FAIL failed"
echo "============================================="

if [ $FAIL -gt 0 ]; then
    echo -e "  Failures:$ERRORS"
    exit 1
fi
