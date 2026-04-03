#!/usr/bin/env bash
set -euo pipefail

# -- Demo 04: Fuel Limits -----------------------------------------------------
# Scenario: CPU-intensive program runs with different fuel budgets.
# Expected: Low fuel = killed, high fuel = completes.

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
echo -e "${BOLD}  DEMO 04: Fuel Limits${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Scenario: 100M-iteration loop with two different fuel budgets"
echo "  Expected: --fuel 100000 fails, --fuel 10000000000 succeeds"
echo ""

# -- Compile -------------------------------------------------------------------
echo -e "${YELLOW}Compiling program.rs to WASM...${NC}"
rustc --target wasm32-wasip1 --edition 2021 -o "$TMPDIR/program.wasm" "$SCRIPT_DIR/program.rs"
echo -e "${GREEN}Compiled${NC}"
echo ""

# -- Run 1: Low fuel -----------------------------------------------------------
echo -e "${BOLD}-- RUN 1: --fuel 100000 (too low) --${NC}"
echo ""

OUTPUT=$("$CONTAINMENT" run --fuel 100000 "$TMPDIR/program.wasm" 2>&1 || true)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"fuel"*|*"Fuel"*|*"exceeded"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"Sum:"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"[containment]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""

# -- Run 2: High fuel ----------------------------------------------------------
echo -e "${BOLD}-- RUN 2: --fuel 10000000000 (enough) --${NC}"
echo ""

OUTPUT=$("$CONTAINMENT" run --fuel 10000000000 "$TMPDIR/program.wasm" 2>&1)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"Sum:"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"fuel"*|*"Fuel"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *"[containment]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""
echo -e "${BOLD}-- Explanation --${NC}"
echo ""
echo "  Fuel is a CPU budget measured in wasmtime fuel units. Every WASM"
echo "  instruction costs fuel. When the budget runs out, the runtime"
echo "  kills the program immediately - no graceful shutdown, no retry."
echo ""
echo "  With --fuel 100000, the 100M-iteration loop exhausts its budget"
echo "  almost instantly. With --fuel 10000000000, it has enough room"
echo "  to finish. This is how you prevent runaway computation without"
echo "  relying on wall-clock timeouts."
echo ""
