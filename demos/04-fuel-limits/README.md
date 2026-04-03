# Demo 04: Fuel Limits

This demo shows CPU fuel enforcement. A program runs a 100M-iteration loop that burns through computation. First it runs with a tiny fuel budget (100,000 units) and gets killed mid-loop. Then it runs with a large budget (10 billion units) and finishes normally.

Fuel is not a wall-clock timeout. It counts actual WASM instructions executed, so a program cannot cheat by sleeping or doing I/O. When the budget hits zero, the runtime kills the program immediately with no grace period. This is how you cap computation cost for untrusted code.

To run: `bash demo.sh`
