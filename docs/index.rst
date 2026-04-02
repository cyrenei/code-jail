containment
===========

A WASM sandbox that works like Docker. Run untrusted code with deny-by-default capabilities.

Containment wraps `wasmtime <https://wasmtime.dev/>`_ with a Docker-familiar CLI. Programs run as WebAssembly modules and can only access what you explicitly grant: specific directories, specific network destinations, specific environment variables.

If you have ever wanted to run an AI coding agent or a random script without worrying about what it does to your system, this is the tool for that.

.. code-block:: bash

   # Fully isolated. No filesystem, no network, no env vars.
   $ containment run program.wasm

   # Grant read access to one directory and network to one API
   $ containment run agent.wasm \
       --cap fs:read:/home/you/project \
       --cap net:api.openai.com:443

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
   :caption: Internals

   architecture
   security

.. toctree::
   :maxdepth: 1
   :caption: Project

   contributing
   changelog
