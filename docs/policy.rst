Policy enforcement
==================

The policy engine is the enforcement layer for codejail. When you pass ``--policy policy.toml``, every capability grant is evaluated against a deny-by-default policy before the sandbox starts.

Without a policy, the operator's flags are granted unconditionally. With a policy active, there is a gap between requesting a capability and receiving it -- and that gap is where security decisions live.

Why use a policy?
-----------------

WASM isolation (the cell) keeps untrusted code from accessing what you did not grant. But it trusts whoever configured the grants. The policy engine (the guard) evaluates every grant against rules you define, so the configuration itself is checked.

This matters because:

- Operators make mistakes. A policy file catches overly broad grants before they take effect.
- Intent drifts from action. Drift detection flags when a "read and analyze" session requests write access.
- Compliance needs records. The audit log captures every decision with full context.
- Sessions need limits. Call budgets and time limits prevent runaway agents.

Enabling policy enforcement
---------------------------

Pass ``--policy`` with a policy file:

.. code-block:: bash

   codejail run agent.wasm --policy policy.toml

Or set the environment variable:

.. code-block:: bash

   export POLICY_FILE=policy.toml
   codejail run agent.wasm

Optional flags:

- ``--intent "description"`` -- declare session intent for drift detection (default: ``general``)
- ``--audit-log path.jsonl`` -- write decisions to structured JSONL audit log

Writing policies
----------------

A minimal policy file:

.. code-block:: toml

   [[policies]]
   id = "allow-read"
   effect = "allow"
   allowed_tools = ["fs_read", "env_read"]

   [policies.intent_match]
   keywords = ["read", "analyze"]

Policies use deny-by-default. If no policy matches a capability request, it is denied.

For the full policy language -- effects, matching criteria, specificity ordering, parameter constraints, and real-world examples -- see the `policy guide <https://github.com/cyrenei/arbiter-mcp-firewall/blob/main/docs/sphinx/guides/policy.md>`_ in the arbiter-mcp-firewall project.

What the policy engine evaluates
--------------------------------

Each ``--cap`` flag becomes a tool call evaluated by the policy engine:

.. list-table::
   :header-rows: 1

   * - Flag
     - Tool name
     - What is checked
   * - ``--cap fs:read:/path``
     - ``fs_read``
     - Policy allows fs_read + intent matches + path constraints
   * - ``--cap fs:write:/path``
     - ``fs_write``
     - Policy allows fs_write + intent matches + path constraints
   * - ``--cap net:host:port``
     - ``net_connect``
     - Policy allows net_connect + destination constraints
   * - ``--cap env:VAR``
     - ``env_read``
     - Policy allows env_read + variable name constraints
   * - ``--net``
     - ``net_connect``
     - Policy allows net_connect (broad)

Denied grants are removed before the sandbox starts. The audit log records every decision.

Drift detection
---------------

When ``--intent`` is passed, the policy engine classifies each capability's operation type (read, write, admin) and compares it to the declared intent. Mismatches are flagged:

.. code-block:: bash

   $ codejail run agent.wasm \
       --policy policy.toml \
       --intent "read and analyze source code" \
       --cap fs:write:/tmp

   [codejail]   drift detected: fs_write (Write) vs intent 'read and analyze source code'

Drift detection runs independently of policy evaluation. Even if a policy allows the grant, drift still flags the mismatch. This catches agents that game policy rules while violating their stated purpose.

Further reading
---------------

The policy engine is built on the `arbiter-mcp-firewall <https://github.com/cyrenei/arbiter-mcp-firewall>`_ project. These links point to its documentation:

- `Architecture <https://github.com/cyrenei/arbiter-mcp-firewall/blob/main/docs/sphinx/understanding/architecture.md>`_ -- full middleware chain and crate dependency graph
- `Security model <https://github.com/cyrenei/arbiter-mcp-firewall/blob/main/docs/sphinx/understanding/security-model.md>`_ -- threat model and defense philosophy
- `Policy guide <https://github.com/cyrenei/arbiter-mcp-firewall/blob/main/docs/sphinx/guides/policy.md>`_ -- complete policy language reference
- `Attack scenarios <https://github.com/cyrenei/arbiter-mcp-firewall/blob/main/docs/sphinx/reference/attack-scenarios.md>`_ -- 10 attack patterns and how the policy engine blocks them
- `Demos <https://github.com/cyrenei/arbiter-mcp-firewall/tree/main/demos>`_ -- reproducible attack demonstrations
