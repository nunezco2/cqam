// CQAM Kernel Template: entangle
// Entangles qubits in {{DST}} sourced from {{SRC}}
// Classical context: {{PARAM0}}, {{PARAM1}}
h {{SRC}}[0];
cx {{SRC}}[0], {{DST}}[1];
cx {{SRC}}[0], {{DST}}[2];
cx {{SRC}}[0], {{DST}}[3];
// GHZ-like entanglement structure
