#!/bin/sh
# End-to-end demo of containment.
# Usage: ./demos/demo.sh
#
# Prerequisites:
#   - containment binary in PATH (or set CONTAINMENT env var)
#   - rustc with wasm32-wasip1 target (rustup target add wasm32-wasip1)

set -eu

CONTAINMENT="${CONTAINMENT:-containment}"
DEMO_DIR="$(cd "$(dirname "$0")" && pwd)"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

export CONTAINMENT_HOME="$WORK_DIR/state"

header() { printf "\n\033[1;36m%s\033[0m\n%s\n" "$1" "--------------------------------------------"; }

# Compile demo programs
header "Compiling demo programs to WASM"
for src in hello spy writer burner; do
    rustc --target wasm32-wasip1 --edition 2021 -o "$WORK_DIR/${src}.wasm" "$DEMO_DIR/${src}.rs"
    printf "  %s.rs -> %s.wasm\n" "$src" "$src"
done

header "1. System info"
$CONTAINMENT info

header "2. Fully isolated sandbox (zero capabilities)"
$CONTAINMENT run "$WORK_DIR/hello.wasm"

header "3. Sandbox escape attempt (8 vectors, all blocked)"
$CONTAINMENT run "$WORK_DIR/spy.wasm"

header "4. Capability grants: write to mounted workspace"
mkdir -p "$WORK_DIR/output"
$CONTAINMENT run "$WORK_DIR/writer.wasm" -v "$WORK_DIR/output:/workspace"
printf "\nHost-side verification:\n"
cat "$WORK_DIR/output/analysis.txt"

header "5. Fuel limit enforcement"
printf "Low fuel (100K) - terminated:\n"
$CONTAINMENT run "$WORK_DIR/burner.wasm" --fuel 100000 2>&1 | grep -E "(trap|Error)" | head -1 || true
printf "\nHigh fuel (10B) - completes:\n"
$CONTAINMENT run "$WORK_DIR/burner.wasm" --fuel 10000000000

header "6. Image management"
$CONTAINMENT import hello "$WORK_DIR/hello.wasm"
$CONTAINMENT import spy "$WORK_DIR/spy.wasm"
$CONTAINMENT images
printf "\nRun by name:\n"
$CONTAINMENT run hello

header "7. Container lifecycle"
$CONTAINMENT ps -a
$CONTAINMENT prune

header "8. Arbiter policy enforcement"
printf "Read intent + write cap (DENIED by policy):\n"
$CONTAINMENT run "$WORK_DIR/writer.wasm" \
    --arbiter "$DEMO_DIR/arbiter-policy.toml" \
    --intent "read and review code" \
    -v "$WORK_DIR/output:/workspace" 2>&1 || true

printf "\nBuild intent + write cap (ALLOWED by policy):\n"
$CONTAINMENT run "$WORK_DIR/writer.wasm" \
    --arbiter "$DEMO_DIR/arbiter-policy.toml" \
    --intent "build artifacts and write output" \
    -v "$WORK_DIR/output:/workspace"

header "9. Audit log"
$CONTAINMENT run hello \
    --arbiter "$DEMO_DIR/arbiter-policy.toml" \
    --intent "read and analyze" \
    --audit-log "$WORK_DIR/audit.jsonl" \
    --cap fs:read:/tmp \
    --cap net:evil.com:443

printf "\nAudit entries:\n"
if command -v python3 >/dev/null 2>&1; then
    python3 -c "
import json
with open('$WORK_DIR/audit.jsonl') as f:
    for line in f:
        e = json.loads(line)
        print(f\"  {e['tool_called']:15s} -> {e['authorization_decision']:5s}  (policy: {e.get('policy_matched', 'none')})\")
"
else
    cat "$WORK_DIR/audit.jsonl"
fi

header "Demo complete"
printf "All features demonstrated.\n"
