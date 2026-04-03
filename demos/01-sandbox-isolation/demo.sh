#!/usr/bin/env bash
set -euo pipefail

# ── Demo 01: Sandbox Isolation ────────────────────────────────────────
# Scenario: Program runs with zero capabilities granted.
# Expected: No filesystem, network, or env access.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="${SCRIPT_DIR}/../.."
CODEJAIL="${ROOT}/target/release/codejail"
if [ ! -x "$CODEJAIL" ]; then
  CODEJAIL="${ROOT}/target/debug/codejail"
fi
if [ ! -x "$CODEJAIL" ]; then
  echo -e "\033[0;31mNo codejail binary found. Run 'cargo build' first.\033[0m"
  exit 1
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
export CODEJAIL_HOME="$TMPDIR/state"

echo ""
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  DEMO 01: Sandbox Isolation${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Scenario: Run a WASM module with zero capabilities"
echo "  Expected: No filesystem, no network, no environment variables"
echo ""

# ── Compile ──────────────────────────────────────────────────────────
echo -e "${YELLOW}Compiling program.rs to WASM...${NC}"
rustc --target wasm32-wasip1 --edition 2021 -o "$TMPDIR/program.wasm" "$SCRIPT_DIR/program.rs"
echo -e "${GREEN}Compiled${NC}"
echo ""

# ── Run ──────────────────────────────────────────────────────────────
echo -e "${BOLD}── RUN: Zero capabilities ──${NC}"
echo ""

OUTPUT=$("$CODEJAIL" run "$TMPDIR/program.wasm" 2>&1)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"not visible"*|*"Good"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"should not happen"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[codejail]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""
echo -e "${BOLD}── Explanation ──${NC}"
echo ""
echo "  When you run a module with no --cap flags, no -v mounts, and"
echo "  no -e variables, it starts with absolutely nothing. The WASI"
echo "  runtime has no preopened directories, no network sockets, and"
echo "  no environment variables. Every access returns 'not found'."
echo ""
echo "  This is deny-by-default. You have to grant every capability"
echo "  explicitly."
echo ""
