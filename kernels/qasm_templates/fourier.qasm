// CQAM Kernel Template: fourier (QFT)
// Applies QFT to {{SRC}}, result in {{DST}}
// Classical context: {{PARAM0}}, {{PARAM1}}
h {{SRC}}[0];
cp(pi/2) {{SRC}}[1], {{SRC}}[0];
cp(pi/4) {{SRC}}[2], {{SRC}}[0];
cp(pi/8) {{SRC}}[3], {{SRC}}[0];
h {{SRC}}[1];
cp(pi/2) {{SRC}}[2], {{SRC}}[1];
cp(pi/4) {{SRC}}[3], {{SRC}}[1];
h {{SRC}}[2];
cp(pi/2) {{SRC}}[3], {{SRC}}[2];
h {{SRC}}[3];
// Swap to bit-reverse order
swap {{SRC}}[0], {{SRC}}[3];
swap {{SRC}}[1], {{SRC}}[2];
