containment
===========

A WASM sandbox with policy-enforced access control. Run untrusted code with deny-by-default capabilities and arbiter policy enforcement on every grant.

The sandbox is the cell. The `arbiter <../arbiter-mcp-firewall/>`_ is the guard. Together they form containment.

.. code-block:: bash

   # Run with arbiter policy enforcement (recommended)
   $ containment run agent.wasm \
       --arbiter policy.toml \
       --intent "read and analyze" \
       --cap fs:read:/home/you/project

   # Simple mode — no policy, not recommended
   $ containment run program.wasm

.. toctree::
   :maxdepth: 2
   :caption: Getting started

   install
   quickstart

.. toctree::
   :maxdepth: 2
   :caption: Usage

   cli
   capabilities
   containmentfile
   resource-limits

.. toctree::
   :maxdepth: 2
   :caption: Arbiter policy enforcement

   arbiter

.. toctree::
   :maxdepth: 2
   :caption: Internals

   architecture
   security

.. toctree::
   :maxdepth: 1
   :caption: Project

   contributing
   changelog
