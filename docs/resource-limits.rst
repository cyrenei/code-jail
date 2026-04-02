Resource limits
===============

Containment enforces two resource limits on sandboxed programs: CPU fuel and wall-clock time.

CPU fuel
--------

Wasmtime tracks "fuel" as a proxy for CPU usage. Every WebAssembly instruction consumes some fuel. When the fuel runs out, the program is terminated immediately.

.. code-block:: bash

   # Default: 1 billion fuel units (roughly a few seconds of compute)
   $ containment run compute-heavy.wasm

   # More fuel for longer tasks
   $ containment run agent.wasm --fuel 10000000000

   # Unlimited (be careful)
   $ containment run trusted.wasm --fuel 0

The default of 1 billion units is enough for most short-lived programs. If you are running something that does serious computation, increase it.

When a program exceeds its fuel budget, you will see:

.. code-block:: text

   Error: CPU fuel limit exceeded

Wall-clock timeout
------------------

Independent of fuel, there is a wall-clock timeout that kills the sandbox after a fixed number of seconds.

.. code-block:: bash

   # Default: 300 seconds (5 minutes)
   $ containment run agent.wasm

   # Longer timeout
   $ containment run agent.wasm --timeout 3600

This catches cases where a program is blocked on I/O (waiting for network, stuck on stdin) rather than burning CPU. Fuel does not tick during I/O waits, so the wall-clock timeout is the backstop.

In a Containmentfile
--------------------

.. code-block:: toml

   [limits]
   fuel = 5_000_000_000
   wall_time_secs = 600

Memory limits
-------------

The ``memory_mb`` field in the Containmentfile is reserved for future use. Currently, memory is limited by the WASM linear memory model (4 GB maximum for 32-bit WASM modules).

Resource usage reporting
------------------------

After each run, containment prints resource usage to stderr:

.. code-block:: text

   [containment] Fuel used: 1894 / 1000000000 (0.0%)
   [containment] Wall time: 0.02s

This helps you tune limits. If a program consistently uses 90% of its fuel budget, you probably want to increase it.
