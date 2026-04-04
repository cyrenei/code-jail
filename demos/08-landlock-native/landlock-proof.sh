#!/bin/sh
# ── codejail Landlock Enforcement Proof ──────────────────────────
#
# Demonstrates that native bridge mode now enforces filesystem
# restrictions via Landlock LSM. The mount list is no longer theater.
#
# Usage: ./demo/landlock-proof.sh

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
DIM='\033[2m'
BOLD='\033[1m'
RESET='\033[0m'

pass() { printf "  ${GREEN}PASS${RESET} %s\n" "$1"; }
fail() { printf "  ${RED}FAIL${RESET} %s\n" "$1"; exit 1; }
step() { printf "\n${BOLD}[$1]${RESET} $2\n"; }
cmd()  { printf "  ${DIM}\$ %s${RESET}\n" "$*"; }

cd "$(dirname "$0")/.."
DEMO_DIR=$(mktemp -d /tmp/codejail-landlock-demo.XXXXXX)
trap 'rm -rf "$DEMO_DIR"' EXIT

printf "\n${BOLD}${CYAN}=== codejail Landlock Enforcement Demo ===${RESET}\n"
printf "${DIM}Kernel: $(uname -r)${RESET}\n"

# ── Build ────────────────────────────────────────────────────────
step 1 "Build codejail"
cmd "cargo build --release 2>/dev/null"
cargo build --release 2>/dev/null
CODEJAIL="$(pwd)/target/release/codejail"
export CODEJAIL_HOME="$DEMO_DIR/home"
mkdir -p "$CODEJAIL_HOME"
pass "built $CODEJAIL"

# ── Package /bin/cat ─────────────────────────────────────────────
step 2 "Package /bin/cat into a sandbox"
cmd "codejail make /bin/cat -o $DEMO_DIR/jailed-cat"
"$CODEJAIL" make /bin/cat -o "$DEMO_DIR/jailed-cat" 2>&1 | sed 's/^/  /'
pass "sandbox created"

# Show the mount list
step 3 "JailFile mounts (these are now ENFORCED by Landlock)"
grep -E '^\s+"/' "$DEMO_DIR/jailed-cat.d/JailFile.toml" | sed 's/^/  /'
echo ""
printf "  ${DIM}Note: /home, /var, /opt, /root — NOT listed, NOT accessible${RESET}\n"

# ── Positive test: /tmp is mounted ───────────────────────────────
step 4 "Read a file in /tmp (allowed — in fs_write)"
echo "HELLO FROM THE SANDBOX" > "$DEMO_DIR/allowed.txt"
cmd "jailed-cat $DEMO_DIR/allowed.txt"
OUTPUT=$("$DEMO_DIR/jailed-cat" "$DEMO_DIR/allowed.txt" 2>/dev/null)
if echo "$OUTPUT" | grep -q "HELLO FROM THE SANDBOX"; then
    pass "jailed cat read the file"
else
    fail "expected output not found"
fi

# ── Negative test: /var/tmp is NOT mounted ───────────────────────
step 5 "Read a file in /var/tmp (BLOCKED — not in mount list)"
echo "TOP SECRET RSA KEY" > /var/tmp/codejail-demo-secret.txt
cmd "jailed-cat /var/tmp/codejail-demo-secret.txt"
if "$DEMO_DIR/jailed-cat" /var/tmp/codejail-demo-secret.txt 2>/dev/null; then
    fail "jailed cat should NOT be able to read /var/tmp"
else
    pass "Landlock denied access — secret is safe"
fi
rm -f /var/tmp/codejail-demo-secret.txt

# ── Negative test: /home ─────────────────────────────────────────
step 6 "List /home (BLOCKED — the original bug)"
cmd "codejail run --native-exec /bin/ls ... -- /home"
if "$CODEJAIL" run \
    --native-exec /bin/ls \
    --jailfile "$DEMO_DIR/jailed-cat.d/JailFile.toml" \
    --fuel 0 --timeout 0 \
    "$DEMO_DIR/jailed-cat.d/bridge.wasm" \
    -- /home 2>/dev/null; then
    fail "jailed ls should NOT be able to list /home"
else
    pass "Landlock denied access — /home is invisible"
fi

# ── Contrast: add /var/tmp to JailFile, re-test ─────────────────
step 7 "Add /var/tmp to JailFile, prove access is now granted"
echo "TOP SECRET RSA KEY" > /var/tmp/codejail-demo-secret.txt
# Patch the JailFile
sed -i 's|^fs_read = \[|fs_read = [\n    "/var/tmp",|' "$DEMO_DIR/jailed-cat.d/JailFile.toml"
cmd "# After adding /var/tmp to fs_read:"
cmd "jailed-cat /var/tmp/codejail-demo-secret.txt"
OUTPUT=$("$DEMO_DIR/jailed-cat" /var/tmp/codejail-demo-secret.txt 2>/dev/null)
if echo "$OUTPUT" | grep -q "TOP SECRET RSA KEY"; then
    pass "access granted — mount list controls Landlock policy"
else
    fail "expected output after adding mount"
fi
rm -f /var/tmp/codejail-demo-secret.txt

# ── Summary ──────────────────────────────────────────────────────
printf "\n${BOLD}${GREEN}=== All checks passed ===${RESET}\n"
printf "${DIM}The mount list printed by codejail is now enforced by Landlock LSM.${RESET}\n"
printf "${DIM}Native bridge mode is no longer a security theater.${RESET}\n\n"
