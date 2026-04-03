#!/usr/bin/env bash
set -euo pipefail

# -- Demo 03: Capability Grants -----------------------------------------------
# Scenario: Program writes a file, reads it back. Volume mount grants access.
# Expected: Write succeeds, file exists on host after run.

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
echo -e "${BOLD}  DEMO 03: Capability Grants${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Scenario: Grant filesystem write access via volume mount"
echo "  Expected: Program writes a file, reads it back, file exists on host"
echo ""

# -- Compile -------------------------------------------------------------------
echo -e "${YELLOW}Compiling program.rs to WASM...${NC}"
rustc --target wasm32-wasip1 --edition 2021 -o "$TMPDIR/program.wasm" "$SCRIPT_DIR/program.rs"
echo -e "${GREEN}Compiled${NC}"
echo ""

# -- Setup ---------------------------------------------------------------------
echo -e "${BOLD}-- SETUP: Mount a temp directory as /workspace --${NC}"
echo ""
WORKSPACE="$TMPDIR/workspace"
mkdir -p "$WORKSPACE"
echo -e "  ${YELLOW}Host path:  $WORKSPACE${NC}"
echo -e "  ${YELLOW}Guest path: /workspace${NC}"
echo ""

# -- Run -----------------------------------------------------------------------
echo -e "${BOLD}-- RUN: With volume mount -v $WORKSPACE:/workspace --${NC}"
echo ""

OUTPUT=$("$CONTAINMENT" run -v "$WORKSPACE:/workspace" "$TMPDIR/program.wasm" 2>&1)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"succeeded"*|*"Contents:"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"failed"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[containment]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""

# -- Verify --------------------------------------------------------------------
echo -e "${BOLD}-- Verify: Check host filesystem --${NC}"
echo ""
if [ -f "$WORKSPACE/report.txt" ]; then
  echo -e "  ${GREEN}File exists on host: $WORKSPACE/report.txt${NC}"
  echo -e "  ${GREEN}Contents: $(cat "$WORKSPACE/report.txt")${NC}"
else
  echo -e "  ${RED}File NOT found on host (unexpected)${NC}"
fi

echo ""
echo -e "${BOLD}-- Explanation --${NC}"
echo ""
echo "  The program wrote to /workspace/report.txt inside the sandbox."
echo "  Because we mounted a host directory with -v, the write went"
echo "  through to the real filesystem. The file is visible on the host"
echo "  after the sandbox exits."
echo ""
echo "  Without the -v mount, /workspace would not exist inside the"
echo "  sandbox and the write would fail (as shown in demos 01 and 02)."
echo ""
