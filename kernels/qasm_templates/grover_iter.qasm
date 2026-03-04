// CQAM Kernel Template: grover_iter
// One Grover iteration on {{SRC}}, result in {{DST}}
// Target state encoded in {{PARAM0}}, {{PARAM1}} unused

// --- Oracle: phase-flip target state ---
// Mark target state (controlled-Z on target bit pattern)
// Target is read from classical register {{PARAM0}}
x {{SRC}}[0];
x {{SRC}}[1];
h {{SRC}}[3];
ccx {{SRC}}[0], {{SRC}}[1], {{SRC}}[3];
h {{SRC}}[3];
x {{SRC}}[0];
x {{SRC}}[1];

// --- Diffusion: inversion about mean ---
h {{SRC}}[0];
h {{SRC}}[1];
h {{SRC}}[2];
h {{SRC}}[3];
x {{SRC}}[0];
x {{SRC}}[1];
x {{SRC}}[2];
x {{SRC}}[3];
h {{SRC}}[3];
ccx {{SRC}}[0], {{SRC}}[1], {{SRC}}[3];
h {{SRC}}[3];
x {{SRC}}[0];
x {{SRC}}[1];
x {{SRC}}[2];
x {{SRC}}[3];
h {{SRC}}[0];
h {{SRC}}[1];
h {{SRC}}[2];
h {{SRC}}[3];
