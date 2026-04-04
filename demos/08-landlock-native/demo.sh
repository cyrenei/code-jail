#!/usr/bin/env bash
set -euo pipefail

# -- Demo 08: Landlock Native Bridge Enforcement --------------------------------
# Proves that codejail's native bridge mode enforces filesystem restrictions
# via Linux Landlock LSM. The JailFile mount list is no longer advisory —
# paths not listed are denied at the kernel level.
#
# Requires: Linux 5.13+ with CONFIG_SECURITY_LANDLOCK=y

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
DIM='\033[2m'
NC='\033[0m'

pass() { printf "  ${GREEN}PASS${NC} %s\n" "$1"; }
fail() { printf "  ${RED}FAIL${NC} %s\n" "$1"; exit 1; }

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
export CODEJAIL_HOME="$TMPDIR/state"

echo ""
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  DEMO 08: Landlock Native Bridge Enforcement${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Native bridge mode wraps a host binary in a WASM supervisor."
echo "  Landlock LSM enforces the JailFile's filesystem restrictions"
echo "  at the kernel level — no root, no containers, no namespaces."
echo ""
echo -e "${DIM}  Kernel: $(uname -r)${NC}"
echo ""

# -- Package /bin/cat -----------------------------------------------------------
echo -e "${YELLOW}Packaging /bin/cat into a sandbox...${NC}"
CODEJAIL_HOME="$TMPDIR/state" "$CODEJAIL" make /bin/cat -o "$TMPDIR/jailed-cat" >/dev/null 2>&1
echo -e "${GREEN}Sandbox created${NC}"
echo ""

BRIDGE="$TMPDIR/jailed-cat.d/bridge.wasm"
JAILFILE="$TMPDIR/jailed-cat.d/JailFile.toml"

run_jailed() {
  CODEJAIL_HOME="$TMPDIR/state" "$CODEJAIL" run \
    --native-exec "$1" \
    --jailfile "$JAILFILE" \
    --fuel 0 --timeout 0 \
    "$BRIDGE" -- "${@:2}" 2>&1 | grep -v "^\[codejail\]" | grep -v "^$" | grep -v "^[0-9a-f]*-[0-9a-f]*$"
}

# -- Test 1: Allowed path (/tmp) ------------------------------------------------
echo -e "${BOLD}-- Test 1: Read file in /tmp (allowed — in fs_write) --${NC}"
echo "ACCESS GRANTED" > "$TMPDIR/allowed.txt"
OUTPUT=$(run_jailed /bin/cat "$TMPDIR/allowed.txt" || true)
if echo "$OUTPUT" | grep -q "ACCESS GRANTED"; then
  pass "jailed cat read the file"
else
  fail "expected output not found"
fi
echo ""

# -- Test 2: Blocked path (/var/tmp) --------------------------------------------
echo -e "${BOLD}-- Test 2: Read file in /var/tmp (BLOCKED — not in mounts) --${NC}"
echo "TOP SECRET" > /var/tmp/codejail-demo-08.txt
OUTPUT=$(run_jailed /bin/cat /var/tmp/codejail-demo-08.txt 2>&1 || true)
if echo "$OUTPUT" | grep -qi "permission denied"; then
  pass "Landlock denied access — secret is safe"
else
  fail "expected permission denied"
fi
rm -f /var/tmp/codejail-demo-08.txt
echo ""

# -- Test 3: Blocked path (/home) -----------------------------------------------
echo -e "${BOLD}-- Test 3: List /home (BLOCKED — not in mounts) --${NC}"
OUTPUT=$(CODEJAIL_HOME="$TMPDIR/state" "$CODEJAIL" run \
  --native-exec /bin/ls \
  --jailfile "$JAILFILE" \
  --fuel 0 --timeout 0 \
  "$BRIDGE" -- /home 2>&1 || true)
if echo "$OUTPUT" | grep -qi "permission denied\|cannot open"; then
  pass "Landlock denied access — /home is invisible"
else
  fail "expected permission denied for /home"
fi
echo ""

# -- Test 4: Add path to JailFile, re-test --------------------------------------
echo -e "${BOLD}-- Test 4: Add /var/tmp to JailFile, prove access is granted --${NC}"
echo "NOW VISIBLE" > /var/tmp/codejail-demo-08.txt
sed -i 's|^fs_read = \[|fs_read = [\n    "/var/tmp",|' "$JAILFILE"
OUTPUT=$(run_jailed /bin/cat /var/tmp/codejail-demo-08.txt || true)
if echo "$OUTPUT" | grep -q "NOW VISIBLE"; then
  pass "access granted — mount list controls Landlock policy"
else
  fail "expected output after adding mount"
fi
rm -f /var/tmp/codejail-demo-08.txt
echo ""

# -- Summary --------------------------------------------------------------------
echo -e "${BOLD}-- Explanation --${NC}"
echo ""
echo "  The native bridge wraps a host binary (/bin/cat) in a WASM supervisor."
echo "  Before the child process executes, Landlock LSM is applied via pre_exec."
echo "  The child inherits the restriction — only JailFile paths are accessible."
echo ""
echo "  Test 4 proves the JailFile is the control variable: same binary, same"
echo "  bridge, same Landlock code — only the mount list changed."
echo ""
echo "  This enforcement is kernel-level. It cannot be bypassed by the sandboxed"
echo "  process regardless of what code it runs."
echo ""
