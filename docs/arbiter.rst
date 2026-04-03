Arbiter policy enforcement
==========================

Arbiter is the policy enforcement layer for containment. When you pass ``--arbiter policy.toml``, every capability grant is evaluated against a deny-by-default policy before the sandbox starts.

Without arbiter, the operator's flags are granted unconditionally. With arbiter, there is a gap between requesting a capability and receiving it — and that gap is where security policy lives.

Why arbiter?
------------

WASM isolation (the cell) keeps untrusted code from accessing what you did not grant. But it trusts whoever configured the grants. Arbiter (the guard) evaluates every grant against policy, so the configuration itself is checked.

This matters because:

- Operators make mistakes. A policy file catches overly broad grants before they take effect.
- Intent drifts from action. Drift detection flags when a "read and analyze" session requests write access.
- Compliance needs records. The audit log captures every decision with full context.
- Sessions need limits. Call budgets and time limits prevent runaway agents.

Enabling arbiter
----------------

Pass ``--arbiter`` with a policy file:

.. code-block:: bash

   containment run agent.wasm --arbiter policy.toml

Or set the environment variable:

.. code-block:: bash

   export ARBITER_POLICY=policy.toml
   containment run agent.wasm

Optional flags:

- ``--intent "description"`` — declare session intent for drift detection (default: ``general``)
- ``--audit-log path.jsonl`` — write decisions to structured JSONL audit log

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

For the full policy language — effects, matching criteria, specificity ordering, parameter constraints, and real-world examples — see the `policy guide <../arbiter-mcp-firewall/docs/sphinx/guides/policy.md>`_.

What arbiter evaluates
----------------------

Each ``--cap`` flag becomes a tool call evaluated by arbiter:

.. list-table::
   :header-rows: 1

   * - Flag
     - Tool name
     - What arbiter checks
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

When ``--intent`` is passed, arbiter classifies each capability's operation type (read, write, admin) and compares it to the declared intent. Mismatches are flagged:

.. code-block:: bash

   $ containment run agent.wasm \
       --arbiter policy.toml \
       --intent "read and analyze source code" \
       --cap fs:write:/tmp

   [containment]   drift detected: fs_write (Write) vs intent 'read and analyze source code'

Drift detection runs independently of policy evaluation. Even if a policy allows the grant, drift still flags the mismatch. This catches agents that game policy rules while violating their stated purpose.

Further reading
---------------

- `Arbiter architecture <../arbiter-mcp-firewall/docs/sphinx/understanding/architecture.md>`_ — full middleware chain and crate dependency graph
- `Security model <../arbiter-mcp-firewall/docs/sphinx/understanding/security-model.md>`_ — threat model and defense philosophy
- `Policy guide <../arbiter-mcp-firewall/docs/sphinx/guides/policy.md>`_ — complete policy language reference
- `Attack scenarios <../arbiter-mcp-firewall/docs/sphinx/reference/attack-scenarios.md>`_ — 10 attack patterns and how arbiter blocks them
- `Arbiter demos <../arbiter-mcp-firewall/demos/>`_ — reproducible attack demonstrations
