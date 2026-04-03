codejail
========

A WASM sandbox with policy-enforced access control. Run untrusted code with deny-by-default capabilities and policy enforcement on every grant.

The sandbox is the cell. The `policy engine <https://github.com/cyrenei/arbiter-mcp-firewall>`_ is the guard. Together they form codejail.

.. code-block:: bash

   # Run with policy enforcement (recommended)
   $ codejail run agent.wasm \
       --policy policy.toml \
       --intent "read and analyze" \
       --cap fs:read:/home/you/project

   # Simple mode -- no policy, not recommended
   $ codejail run program.wasm

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
   jailfile
   resource-limits

.. toctree::
   :maxdepth: 2
   :caption: Policy enforcement

   policy

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
