# Containment Demos

Seven reproducible demonstrations of WASM sandbox isolation and arbiter policy enforcement. Each demo is self-contained: one script, one explanation.

Demos 01-04 show sandbox isolation (the cell). Demos 05-07 show arbiter policy enforcement (the guard). Together they demonstrate the recommended production configuration.

## Prerequisites

Build containment from the project root:

```bash
cargo build --release
```

You also need the WASM compilation target:

```bash
rustup target add wasm32-wasip1
```

Each demo script expects the binary at `../../target/release/containment` (relative to the demo directory).

## Running a demo

```bash
cd demos/01-sandbox-isolation
bash demo.sh
```

Each script will:
1. Compile a small Rust program to WASM
2. Run it inside a containment sandbox
3. Show what was allowed and what was blocked
4. Print an explanation

## The seven demos

### Sandbox isolation (the cell)

| # | Demo | Scenario | Expected |
|---|------|----------|----------|
| 01 | Sandbox Isolation | Program runs with zero capabilities | No filesystem, network, or env access |
| 02 | Escape Attempt | Program tries to read /etc/passwd, /home, /proc, write /tmp | All 8 vectors blocked |
| 03 | Capability Grants | Program writes to a mounted workspace | Write succeeds in granted dir only |
| 04 | Fuel Limits | CPU-heavy program with low fuel budget | Terminated when fuel runs out |

### Arbiter policy enforcement (the guard) — recommended

| # | Demo | Scenario | Expected |
|---|------|----------|----------|
| 05 | Arbiter Policy | Write cap requested with read-only intent | Denied by arbiter policy |
| 06 | Intent Drift | Write operation flagged against read intent | Drift detected, operation denied |
| 07 | Audit Trail | Mixed capabilities evaluated by arbiter | JSONL audit log of every decision |

Demos 05-07 use `--arbiter` to route capability requests through the policy engine. This is the recommended way to run containment. Without arbiter, the operator's flags are granted unconditionally — there is no policy evaluation, drift detection, or audit trail.

For additional attack scenario demonstrations, see the [arbiter demos](../arbiter-mcp-firewall/demos/) which cover 10 attack patterns including unauthenticated access, tool escalation, session replay, parameter tampering, and credential exfiltration.

## Color coding

- Green text = allowed (legitimate operation succeeded)
- Red text = blocked (unauthorized operation stopped)
- Yellow text = informational

## Architecture

Containment uses wasmtime (WASI preview 1) for capability isolation — the cell that keeps untrusted code in. The arbiter integration routes capability requests through a deny-by-default policy engine — the guard that controls what crosses the boundary. Together they form containment.

All enforcement is real. No simulation, no mocks, no stubs.
