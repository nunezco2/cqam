OPENQASM 3.0;
include "stdgates.inc";

// === CQAM Register Declarations ===
int[64] R0;
int[64] R1;
qubit[16] q0;
bit[16] H0;

// === Program Body ===
R0 = 5;
R1 = R0 + R0;
reset q0;
// QPrep: initialize q0 with distribution 'uniform'
H0 = measure q0;

// === End CQAM Generated QASM ===