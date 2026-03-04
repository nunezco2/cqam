// CQAM Kernel Template: diffuse (Grover diffusion)
// Inversion about the mean on {{SRC}}, result in {{DST}}
// Classical context: {{PARAM0}}, {{PARAM1}}
h {{SRC}}[0];
h {{SRC}}[1];
h {{SRC}}[2];
h {{SRC}}[3];
x {{SRC}}[0];
x {{SRC}}[1];
x {{SRC}}[2];
x {{SRC}}[3];
// Multi-controlled Z gate (3 controls + 1 target)
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
