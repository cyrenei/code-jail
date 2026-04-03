#!/usr/bin/env bash
set -euo pipefail

# -- Demo 06: Intent Drift -----------------------------------------------------
# Scenario: Intent says "read and analyze" but a write volume is mounted.
# Expected: Drift detection message in output.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="${SCRIPT_DIR}/../.."
CONTAINMENT="${ROOT}/target/release/containment"
if [ ! -x "$CONTAINMENT" ]; then
  CONTAINMENT="${ROOT}/target/debug/containment"
fi
if [ ! -x "$CONTAINMENT" ]; then
  echo -e "\033[0;31mNo containment binary found. Run 'cargo build' first.\033[0m"
  exit 1
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
export CONTAINMENT_HOME="$TMPDIR/state"

echo ""
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  DEMO 06: Intent Drift${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Scenario: Intent is \"read and analyze\" but a write mount is requested"
echo "  Expected: Drift detection - write capability vs read intent"
echo ""

# -- Compile -------------------------------------------------------------------
echo -e "${YELLOW}Compiling program.rs to WASM...${NC}"
rustc --target wasm32-wasip1 --edition 2021 -o "$TMPDIR/program.wasm" "$SCRIPT_DIR/program.rs"
echo -e "${GREEN}Compiled${NC}"
echo ""

POLICY="$SCRIPT_DIR/arbiter-policy.toml"
WORKSPACE="$TMPDIR/workspace"
mkdir -p "$WORKSPACE"

# -- Run: Read intent with write mount -----------------------------------------
echo -e "${BOLD}-- RUN: --intent \"read and analyze\" with write volume mount --${NC}"
echo ""

OUTPUT=$("$CONTAINMENT" run \
  --arbiter "$POLICY" \
  --intent "read and analyze" \
  -v "$WORKSPACE:/workspace" \
  "$TMPDIR/program.wasm" 2>&1 || true)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"drift detected"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[x]"*|*"denied"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[+]"*|*"allowed"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"[containment]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *"failed"*|*"Failed"*)
      echo -e "  ${RED}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""
echo -e "${BOLD}-- Explanation --${NC}"
echo ""
echo "  The arbiter's behavioral anomaly detector compares what the"
echo "  agent declared it wants to do (the intent) with what it is"
echo "  actually requesting (the capabilities). Here the intent says"
echo "  'read and analyze' but a writable volume mount was requested."
echo ""
echo "  The drift detector flags this mismatch:"
echo "    [containment] drift detected: fs_write (Write) vs intent 'read and analyze'"
echo ""
echo "  This is not the same as policy denial. Drift detection is a"
echo "  separate behavioral layer that watches for intent-action"
echo "  misalignment, even when the policy might allow the operation."
echo ""
