#!/usr/bin/env bash
set -euo pipefail

# -- Demo 07: Audit Trail ------------------------------------------------------
# Scenario: Run with multiple capabilities and inspect the JSONL audit log.
# Expected: Each capability decision is logged with allow/deny.

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
echo -e "${BOLD}  DEMO 07: Audit Trail${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "  Scenario: Run with mixed capabilities, then inspect the audit log"
echo "  Caps: fs:read:/tmp (allow), net:evil.com:443 (deny),"
echo "        env:USER (allow), fs:write:/tmp (deny)"
echo ""

# -- Compile -------------------------------------------------------------------
echo -e "${YELLOW}Compiling program.rs to WASM...${NC}"
rustc --target wasm32-wasip1 --edition 2021 -o "$TMPDIR/program.wasm" "$SCRIPT_DIR/program.rs"
echo -e "${GREEN}Compiled${NC}"
echo ""

POLICY="$SCRIPT_DIR/arbiter-policy.toml"
AUDIT_LOG="$TMPDIR/audit.jsonl"

# -- Run -----------------------------------------------------------------------
echo -e "${BOLD}-- RUN: Mixed capabilities with --audit-log --${NC}"
echo ""

OUTPUT=$("$CONTAINMENT" run \
  --arbiter "$POLICY" \
  --audit-log "$AUDIT_LOG" \
  --intent "read system info" \
  --cap "fs:read:/tmp" \
  --cap "net:evil.com:443" \
  --cap "env:USER" \
  "$TMPDIR/program.wasm" 2>&1 || true)
echo "$OUTPUT" | while IFS= read -r line; do
  case "$line" in
    *"[x]"*|*"arbiter denied"*)
      echo -e "  ${RED}$line${NC}" ;;
    *"[+]"*|*"allowed by"*)
      echo -e "  ${GREEN}$line${NC}" ;;
    *"drift detected"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *"[containment]"*)
      echo -e "  ${YELLOW}$line${NC}" ;;
    *)
      echo "  $line" ;;
  esac
done

echo ""

# -- Parse audit log -----------------------------------------------------------
echo -e "${BOLD}-- AUDIT LOG: $AUDIT_LOG --${NC}"
echo ""

if [ ! -f "$AUDIT_LOG" ]; then
  echo -e "  ${RED}Audit log not found at $AUDIT_LOG${NC}"
  echo ""
else
  python3 -c "
import json, sys

RED = '\033[0;31m'
GREEN = '\033[0;32m'
YELLOW = '\033[1;33m'
BOLD = '\033[1m'
NC = '\033[0m'

with open('$AUDIT_LOG') as f:
    for i, line in enumerate(f, 1):
        line = line.strip()
        if not line:
            continue
        entry = json.loads(line)
        tool = entry.get('tool_called', '?')
        decision = entry.get('authorization_decision', '?')
        policy = entry.get('policy_matched', None)
        ts = entry.get('timestamp', '?')

        if decision == 'allow':
            color = GREEN
            icon = 'ALLOW'
        else:
            color = RED
            icon = 'DENY '

        policy_str = f' (policy: {policy})' if policy else ''
        print(f'  {color}[{icon}] {tool}{policy_str}{NC}')
        args = entry.get('arguments', {})
        if args and args != 'null' and args is not None:
            for k, v in (args.items() if isinstance(args, dict) else []):
                print(f'  {YELLOW}         {k}: {v}{NC}')
"
fi

echo ""
echo -e "${BOLD}-- Explanation --${NC}"
echo ""
echo "  The --audit-log flag writes a JSONL file where each line is a"
echo "  structured record of one capability decision. Every capability"
echo "  request - whether allowed or denied - gets a timestamped entry"
echo "  with the tool name, arguments, decision, and matching policy."
echo ""
echo "  This demo requested fs:read:/tmp (allowed by policy),"
echo "  net:evil.com:443 (denied - no net_connect policy), env:USER"
echo "  (allowed by policy). The audit log captures all three decisions"
echo "  as machine-parseable JSONL for compliance and forensics."
echo ""
