# Demo 06: Intent Drift

This demo shows the behavioral drift detector. The declared intent is "read and analyze" but the requested capabilities include a writable volume mount. The policy engine flags this mismatch as drift - the agent is asking for write access while claiming it only wants to read.

Drift detection is a separate layer from policy enforcement. Even if a policy happened to allow the operation, the drift detector would still flag the mismatch between intent and action. This catches agents that escalate their own privileges beyond what they said they needed.

To run: `bash demo.sh`
