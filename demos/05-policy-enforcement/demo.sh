#!/usr/bin/env bash
set -euo pipefail

# -- Demo 05: Policy Enforcement -----------------------------------------------
# Scenario: Policy allows fs_write only when intent matches write/build/deploy.
# Expected: "read and review" intent = DENIED, "build output" intent = ALLOWED.

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
echo -e "${BOLD}  DEMO 05: Policy Enforcement${NC}"
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

POLICY="$SCRIPT_DIR/policy.toml"

# -- Run 1: Wrong intent (DENIED) ---------------------------------------------
echo -e "${BOLD}-- RUN 1: --intent \"read and review\" (mismatched) --${NC}"
echo ""

WORKSPACE1="$TMPDIR/ws1"
mkdir -p "$WORKSPACE1"

OUTPUT=$("$CODEJAIL" run \
  --policy "$POLICY" \
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
    *"[codejail]"*)
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

OUTPUT=$("$CODEJAIL" run \
  --policy "$POLICY" \
  --intent "build output" \
  -v "$WORKSPACE2:/workspace" \
  "$TMPDIR/program.wasm" 2>&1)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"[+]"*|*"allowed"*|*"succeeded"*|*"Contents:"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"[x]"*|*"denied"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[codejail]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""
echo -e "${BOLD}-- Explanation --${NC}"
echo ""
echo "  The policy file defines rules that match capabilities against"
echo "  the declared intent. The policy here allows fs_write only when"
echo "  the intent contains 'write', 'build', or 'deploy'."
echo ""
echo "  With intent 'read and review', the fs_write capability for the"
echo "  volume mount is denied by policy. The sandbox either runs with"
echo "  no write access or refuses to start. With intent 'build output',"
echo "  the regex matches and the write is authorized."
echo ""
echo "  This is how the policy engine enforces least-privilege based on"
echo "  what the agent says it wants to do."
echo ""
