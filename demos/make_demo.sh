#!/bin/bash
# codejail make — end-to-end demonstration
#
# This script proves the exit criteria:
#   codejail make /path/to/binary -o "jailed-binary"
#   && ./jailed-binary works correctly
#
# It tests with multiple binary types (ELF, scripts) and
# demonstrates the WASM Supervisor Pattern in action.

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CODEJAIL="${PROJECT_DIR}/target/debug/codejail"
DEMO_DIR="$(mktemp -d /tmp/codejail-make-demo.XXXXX)"
PASS=0
FAIL=0

cleanup() {
    rm -rf "$DEMO_DIR"
}
trap cleanup EXIT

info() { echo -e "${CYAN}[demo]${NC} $*"; }
pass() { echo -e "${GREEN}[PASS]${NC} $*"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}[FAIL]${NC} $*"; FAIL=$((FAIL + 1)); }

# Build codejail first
info "Building codejail..."
(cd "$PROJECT_DIR" && cargo build --quiet 2>/dev/null)

if [ ! -f "$CODEJAIL" ]; then
    echo "Error: codejail binary not found at $CODEJAIL"
    echo "Run 'cargo build' first."
    exit 1
fi

echo
echo "============================================="
echo "  codejail make — End-to-End Demo"
echo "============================================="
echo

# ── Test 1: ELF binary (echo) ────────────────────────────────────

info "Test 1: Package /bin/echo as jailed-echo"
cd "$DEMO_DIR"
"$CODEJAIL" make /bin/echo -o jailed-echo --permissive 2>&1 | sed 's/^/  /'
echo

info "Running: ./jailed-echo 'Hello from the WASM sandbox!'"
OUTPUT=$(./jailed-echo "Hello from the WASM sandbox!" 2>/dev/null)
if echo "$OUTPUT" | grep -q "Hello from the WASM sandbox!"; then
    pass "jailed-echo produced correct output: $OUTPUT"
else
    fail "jailed-echo output unexpected: $OUTPUT"
fi

# ── Test 2: Exit code propagation ────────────────────────────────

info "Test 2: Exit code propagation with /bin/false"
"$CODEJAIL" make /bin/false -o jailed-false --permissive 2>/dev/null
if ! ./jailed-false 2>/dev/null; then
    pass "jailed-false correctly propagated non-zero exit code"
else
    fail "jailed-false should have exited non-zero"
fi

# ── Test 3: Argument passthrough ─────────────────────────────────

info "Test 3: Argument passthrough"
OUTPUT=$(./jailed-echo "arg1" "arg2" "arg3" 2>/dev/null)
if echo "$OUTPUT" | grep -q "arg1 arg2 arg3"; then
    pass "Arguments passed through correctly: $OUTPUT"
else
    fail "Argument passthrough failed: $OUTPUT"
fi

# ── Test 4: Shell script binary ──────────────────────────────────

info "Test 4: Package a shell script"
cat > "$DEMO_DIR/hello.sh" << 'SCRIPT'
#!/bin/sh
echo "Hello from shell script inside codejail!"
echo "Args: $@"
SCRIPT
chmod +x "$DEMO_DIR/hello.sh"

"$CODEJAIL" make "$DEMO_DIR/hello.sh" -o jailed-hello --permissive 2>/dev/null
OUTPUT=$(./jailed-hello "test-arg" 2>/dev/null)
if echo "$OUTPUT" | grep -q "Hello from shell script inside codejail!"; then
    pass "Shell script executed through WASM bridge"
else
    fail "Shell script execution failed: $OUTPUT"
fi

# ── Test 5: Analyze-only mode ───────────────────────────────────

info "Test 5: Analyze-only mode"
if "$CODEJAIL" make /bin/ls -o test --analyze-only 2>&1 | grep -q "analysis complete"; then
    pass "Analyze-only mode works"
else
    fail "Analyze-only mode failed"
fi

# ── Test 6: Generated artifacts ──────────────────────────────────

info "Test 6: Verify generated artifacts"
ALL_OK=true
for f in jailed-echo jailed-echo.d/bridge.wasm jailed-echo.d/JailFile.toml; do
    if [ ! -f "$DEMO_DIR/$f" ]; then
        fail "Missing artifact: $f"
        ALL_OK=false
    fi
done
if $ALL_OK; then
    pass "All artifacts generated correctly"
fi

# ── Test 7: Bridge WASM module is valid ──────────────────────────

info "Test 7: Bridge module inspection"
if "$CODEJAIL" inspect "$DEMO_DIR/jailed-echo.d/bridge.wasm" 2>/dev/null | grep -q "codejail_host"; then
    pass "Bridge module imports codejail_host.exec"
else
    fail "Bridge module missing codejail_host import"
fi

# ── Test 8: Claude Code (if available) ───────────────────────────

info "Test 8: Claude Code binary"
CLAUDE_BIN=$(which claude 2>/dev/null || true)
if [ -n "$CLAUDE_BIN" ]; then
    "$CODEJAIL" make "$CLAUDE_BIN" -o jailed-claude --permissive 2>/dev/null
    CLAUDE_VERSION=$(./jailed-claude --version 2>/dev/null || true)
    if echo "$CLAUDE_VERSION" | grep -q "Claude Code"; then
        pass "jailed-claude --version: $CLAUDE_VERSION"
    else
        fail "jailed-claude --version unexpected: $CLAUDE_VERSION"
    fi
else
    info "(skipped — claude not found in PATH)"
fi

# ── Test 9: Performance ──────────────────────────────────────────

info "Test 9: Bridge overhead measurement"
START_NS=$(date +%s%N)
./jailed-echo "perf test" >/dev/null 2>&1
END_NS=$(date +%s%N)
ELAPSED_MS=$(( (END_NS - START_NS) / 1000000 ))
if [ "$ELAPSED_MS" -lt 1000 ]; then
    pass "Bridge overhead: ${ELAPSED_MS}ms (< 1000ms)"
else
    fail "Bridge overhead too high: ${ELAPSED_MS}ms"
fi

# ── Summary ──────────────────────────────────────────────────────

echo
echo "============================================="
echo -e "  Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}"
echo "============================================="
echo

# List artifacts
info "Demo artifacts in $DEMO_DIR:"
ls -la "$DEMO_DIR"/ 2>/dev/null | head -20

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
