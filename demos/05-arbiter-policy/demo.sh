#!/usr/bin/env bash
set -euo pipefail

# -- Demo 05: Arbiter Policy ---------------------------------------------------
# Scenario: Policy allows fs_write only when intent matches write/build/deploy.
# Expected: "read and review" intent = DENIED, "build output" intent = ALLOWED.

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
echo -e "${BOLD}  DEMO 05: Arbiter Policy${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Policy: fs_write allowed only when intent matches write/build/deploy"
echo "  Test 1: intent = \"read and review\" (should be DENIED)"
echo "  Test 2: intent = \"build output\" (should be ALLOWED)"
echo ""

# -- Compile -------------------------------------------------------------------
echo -e "${YELLOW}Compiling program.rs to WASM...${NC}"
rustc --target wasm32-wasip1 --edition 2021 -o "$TMPDIR/program.wasm" "$SCRIPT_DIR/program.rs"
echo -e "${GREEN}Compiled${NC}"
echo ""

POLICY="$SCRIPT_DIR/arbiter-policy.toml"

# -- Run 1: Wrong intent (DENIED) ---------------------------------------------
echo -e "${BOLD}-- RUN 1: --intent \"read and review\" (mismatched) --${NC}"
echo ""

WORKSPACE1="$TMPDIR/ws1"
mkdir -p "$WORKSPACE1"

OUTPUT=$("$CONTAINMENT" run \
  --arbiter "$POLICY" \
  --intent "read and review" \
  -v "$WORKSPACE1:/workspace" \
  "$TMPDIR/program.wasm" 2>&1 || true)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"[x]"*|*"denied"*|*"Deny"*|*"deny"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[+]"*|*"allowed"*|*"Allow"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"failed"*|*"Failed"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[containment]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""

# -- Run 2: Correct intent (ALLOWED) ------------------------------------------
echo -e "${BOLD}-- RUN 2: --intent \"build output\" (matched) --${NC}"
echo ""

WORKSPACE2="$TMPDIR/ws2"
mkdir -p "$WORKSPACE2"

OUTPUT=$("$CONTAINMENT" run \
  --arbiter "$POLICY" \
  --intent "build output" \
  -v "$WORKSPACE2:/workspace" \
  "$TMPDIR/program.wasm" 2>&1)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"[+]"*|*"allowed"*|*"succeeded"*|*"Contents:"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"[x]"*|*"denied"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[containment]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""
echo -e "${BOLD}-- Explanation --${NC}"
echo ""
echo "  The arbiter policy file defines rules that match capabilities"
echo "  against the declared intent. The policy here allows fs_write"
echo "  only when the intent contains 'write', 'build', or 'deploy'."
echo ""
echo "  With intent 'read and review', the fs_write capability for the"
echo "  volume mount is denied by policy. The sandbox either runs with"
echo "  no write access or refuses to start. With intent 'build output',"
echo "  the regex matches and the write is authorized."
echo ""
echo "  This is how arbiter enforces least-privilege based on what the"
echo "  agent says it wants to do."
echo ""
