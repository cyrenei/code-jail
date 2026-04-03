#!/usr/bin/env bash
set -euo pipefail

# ── Demo 02: Escape Attempt ──────────────────────────────────────────
# Attack: Program tries to read sensitive files, write to /tmp, read env.
# Expected: All 8 vectors blocked.

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
echo -e "${BOLD}  DEMO 02: Escape Attempt${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Attack: Read /etc/passwd, /home, /proc, write /tmp, read env"
echo "  Expected: All 8 vectors blocked"
echo ""

# ── Compile ──────────────────────────────────────────────────────────
echo -e "${YELLOW}Compiling program.rs to WASM...${NC}"
rustc --target wasm32-wasip1 --edition 2021 -o "$TMPDIR/program.wasm" "$SCRIPT_DIR/program.rs"
echo -e "${GREEN}Compiled${NC}"
echo ""

# ── Attack ───────────────────────────────────────────────────────────
echo -e "${BOLD}── ATTACK: 8 escape vectors, zero capabilities ──${NC}"
echo ""

OUTPUT=$("$CODEJAIL" run "$TMPDIR/program.wasm" 2>&1)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"[BLOCKED]"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"[LEAK]"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"8/8"*|*"0/8"*)
      echo -e "  ${BOLD}$line${NC}" ;;
    *"[codejail]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""
echo -e "${BOLD}── Explanation ──${NC}"
echo ""
echo "  The program tried to read /etc/passwd, /etc/shadow, /home,"
echo "  /root/.ssh, /proc/self/environ, write to /tmp, and read the"
echo "  HOME and SSH_AUTH_SOCK environment variables. All 8 attempts"
echo "  failed because the sandbox starts with nothing."
echo ""
echo "  This is not a file-permission check. These paths simply do"
echo "  not exist inside the WASM sandbox. There is no root filesystem,"
echo "  no /proc, no /tmp, and no environment unless you mount them."
echo ""
