#!/usr/bin/env bash
set -euo pipefail

# -- Demo 09: Policy Enforcement on Native Bridge (Claude Code) -----------------
#
# Two-layer enforcement:
#   Layer 1: Arbiter policy gate — evaluates capability grants against policy
#            rules BEFORE they reach the runtime. Denied caps are stripped.
#   Layer 2: Landlock kernel enforcement — authorized caps are enforced at
#            the kernel level on the native process.
#
# This demo shows the same Claude Code binary, same JailFile, same Landlock —
# but different policy verdicts based on declared intent.
#
#   Run 1: intent = "code review"      → fs_write DENIED by policy
#   Run 2: intent = "develop features" → fs_write ALLOWED by policy
#
# Requires: Linux 5.13+, Claude Code installed, Rust toolchain

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

POLICY="$SCRIPT_DIR/policy.toml"

echo ""
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  DEMO 09: Policy Enforcement on Native Bridge${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Two enforcement layers:"
echo "    1. Arbiter policy gate — filters capability grants by intent"
echo "    2. Landlock LSM — enforces surviving grants at kernel level"
echo ""
echo "  Same binary. Same JailFile. Different intent. Different access."
echo ""

# -- Find Claude Code -----------------------------------------------------------
CLAUDE_BIN=""
if command -v claude >/dev/null 2>&1; then
  CLAUDE_BIN="$(readlink -f "$(command -v claude)")"
fi
if [ -z "$CLAUDE_BIN" ] || [ ! -x "$CLAUDE_BIN" ]; then
  echo -e "${YELLOW}Claude Code not found — falling back to /bin/sh for demo${NC}"
  CLAUDE_BIN="/bin/sh"
fi
echo -e "${DIM}  Binary: $CLAUDE_BIN${NC}"

# -- Generate bridge artifacts ---------------------------------------------------
echo ""
echo -e "${YELLOW}Packaging binary into sandbox...${NC}"
CODEJAIL_HOME="$TMPDIR/state" "$CODEJAIL" make "$CLAUDE_BIN" -o "$TMPDIR/test" >/dev/null 2>&1
echo -e "${GREEN}Sandbox created${NC}"

BRIDGE="$TMPDIR/test.d/bridge.wasm"
JAILFILE="$TMPDIR/test.d/JailFile.toml"
AUDIT_LOG="$TMPDIR/audit.jsonl"

# ═══════════════════════════════════════════════════════════════════
# RUN 1: Intent = "code review" → write DENIED
# ═══════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}── RUN 1: intent = \"code review\" ──${NC}"
echo -e "${DIM}  Policy regex for fs_write: (?i)\\b(write|build|deploy|develop|...)\\b${NC}"
echo -e "${DIM}  Intent \"code review\" does NOT match → fs_write should be DENIED${NC}"
echo ""

OUTPUT1=$(CODEJAIL_HOME="$TMPDIR/state" "$CODEJAIL" run \
  --native-exec "$CLAUDE_BIN" \
  --jailfile "$JAILFILE" \
  --policy "$POLICY" \
  --intent "code review" \
  --audit-log "$AUDIT_LOG" \
  --fuel 0 --timeout 0 \
  "$BRIDGE" -- --version 2>&1 || true)

# Show policy decisions
DENIED=false
echo "$OUTPUT1" | while IFS= read -r line; do
  case "$line" in
    *"[x]"*|*"DENIED"*|*"denied"*|*"Deny"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[+]"*|*"ALLOWED"*|*"allowed"*|*"Allow"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"policy:"*|*"intent:"*|*"session:"*|*"agent:"*)
      echo -e "  ${DIM}$line${NC}" ;;
    *"landlock"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      ;;
  esac
done

if echo "$OUTPUT1" | grep -qi "\[x\]\|denied.*fs_write"; then
  echo ""
  pass "policy DENIED fs_write for intent 'code review'"
else
  echo ""
  echo -e "  ${DIM}Full output:${NC}"
  echo "$OUTPUT1" | head -20 | sed 's/^/    /'
  fail "expected fs_write denial"
fi

# ═══════════════════════════════════════════════════════════════════
# RUN 2: Intent = "develop features" → write ALLOWED
# ═══════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}── RUN 2: intent = \"develop features\" ──${NC}"
echo -e "${DIM}  Intent \"develop features\" matches regex → fs_write should be ALLOWED${NC}"
echo ""

OUTPUT2=$(CODEJAIL_HOME="$TMPDIR/state" "$CODEJAIL" run \
  --native-exec "$CLAUDE_BIN" \
  --jailfile "$JAILFILE" \
  --policy "$POLICY" \
  --intent "develop features" \
  --audit-log "$AUDIT_LOG" \
  --fuel 0 --timeout 0 \
  "$BRIDGE" -- --version 2>&1 || true)

echo "$OUTPUT2" | while IFS= read -r line; do
  case "$line" in
    *"[x]"*|*"DENIED"*|*"denied"*|*"Deny"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[+]"*|*"ALLOWED"*|*"allowed"*|*"Allow"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"policy:"*|*"intent:"*|*"session:"*|*"agent:"*)
      echo -e "  ${DIM}$line${NC}" ;;
    *"landlock"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      ;;
  esac
done

if echo "$OUTPUT2" | grep -qi "\[+\].*fs_write\|allowed.*fs_write"; then
  echo ""
  pass "policy ALLOWED fs_write for intent 'develop features'"
else
  echo ""
  echo -e "  ${DIM}Full output:${NC}"
  echo "$OUTPUT2" | head -20 | sed 's/^/    /'
  fail "expected fs_write allowance"
fi

# ═══════════════════════════════════════════════════════════════════
# AUDIT LOG
# ═══════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}── Audit Log ──${NC}"
echo ""
if [ -f "$AUDIT_LOG" ] && [ -s "$AUDIT_LOG" ]; then
  ENTRIES=$(wc -l < "$AUDIT_LOG")
  echo -e "  ${DIM}$ENTRIES entries written to $AUDIT_LOG${NC}"
  echo ""
  # Show a few representative entries
  head -3 "$AUDIT_LOG" | while IFS= read -r line; do
    TOOL=$(echo "$line" | grep -o '"tool_called":"[^"]*"' | head -1)
    DECISION=$(echo "$line" | grep -o '"authorization_decision":"[^"]*"' | head -1)
    if [ -n "$TOOL" ] && [ -n "$DECISION" ]; then
      case "$DECISION" in
        *allow*) echo -e "    ${GREEN}$TOOL → $DECISION${NC}" ;;
        *deny*)  echo -e "    ${RED}$TOOL → $DECISION${NC}" ;;
        *)       echo -e "    ${DIM}$TOOL → $DECISION${NC}" ;;
      esac
    fi
  done
  pass "audit trail recorded"
else
  echo -e "  ${DIM}(no audit log produced — policy may not have generated entries)${NC}"
fi

# ═══════════════════════════════════════════════════════════════════
# EXPLANATION
# ═══════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}── How It Works ──${NC}"
echo ""
echo "  The arbiter policy gate evaluates each JailFile capability as an"
echo "  MCP tool call BEFORE the native process launches. Denied capabilities"
echo "  are stripped from the resolved set. Only authorized capabilities"
echo "  reach Landlock enforcement."
echo ""
echo "  This creates two enforcement layers:"
echo "    1. Policy: decides what capabilities are GRANTED (intent-based)"
echo "    2. Landlock: decides what the kernel ENFORCES (path-based)"
echo ""
echo "  The policy is not runtime interception — it's a compile-time gate."
echo "  Once the process launches, Landlock is the sole enforcement layer."
echo "  The policy's value is in filtering BEFORE launch, not intercepting"
echo "  DURING execution."
echo ""
