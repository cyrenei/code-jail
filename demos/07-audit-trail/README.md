# Demo 07: Audit Trail

This demo shows the arbiter's audit logging. A simple program runs with several capability requests - some allowed, some denied - and every decision is written to a JSONL audit log file.

The demo then parses the audit log with Python and pretty-prints each entry showing the tool, decision, and matching policy. Every capability request gets a timestamped, structured record regardless of whether it was allowed or denied. This is the compliance and forensics layer - you can answer "what did this agent try to do, and what did we let it do?" after the fact.

To run: `bash demo.sh`
