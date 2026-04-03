# Demo 05: Policy Enforcement

This demo introduces the policy engine. A TOML policy file says "fs_write is only allowed when the declared intent matches write, build, or deploy." The same program runs twice with different intents.

With intent "read and review", the policy denies the volume mount because the intent does not match the write policy. With intent "build output", the regex matches and the write goes through. This is intent-based access control - capabilities are granted or denied based on what the agent claims it is doing.

To run: `bash demo.sh`
