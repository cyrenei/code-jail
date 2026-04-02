CLI reference
=============

containment run
---------------

Run a WASM module in a capability-restricted sandbox.

.. code-block:: text

   containment run [OPTIONS] <IMAGE> [-- <ARGS>...]

**Arguments:**

- ``<IMAGE>`` - Path to a ``.wasm`` file, or name of an imported image.
- ``<ARGS>`` - Arguments passed to the WASM module (after ``--``).

**Options:**

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Flag
     - Description
   * - ``--cap <GRANT>``
     - Capability grant. Repeatable. See :doc:`capabilities`.
   * - ``-v, --volume <HOST:GUEST>``
     - Mount a host directory into the sandbox (read+write).
   * - ``-e, --env <KEY=VALUE>``
     - Set an environment variable. If only KEY is given, inherits from host.
   * - ``--net``
     - Allow all network access.
   * - ``-f, --containmentfile <PATH>``
     - Load capabilities from a Containmentfile. See :doc:`containmentfile`.
   * - ``--name <NAME>``
     - Set the container name (default: auto-generated).
   * - ``--fuel <N>``
     - CPU fuel limit (default: 1000000000). Set to 0 for unlimited.
   * - ``--timeout <SECS>``
     - Wall-clock timeout in seconds (default: 300).
   * - ``--bwrap``
     - Enable bubblewrap outer sandbox.
   * - ``-d, --detach``
     - Run in background (not yet implemented).

**Examples:**

.. code-block:: bash

   # Minimal: just stdout/stderr
   $ containment run hello.wasm

   # With filesystem and network
   $ containment run agent.wasm \
       --cap fs:read:/project \
       --cap net:api.openai.com:443 \
       -v /tmp/out:/output \
       -e API_KEY

   # With a Containmentfile
   $ containment run agent.wasm -f Containmentfile.toml

containment build
-----------------

Build an image from a Containmentfile.

.. code-block:: text

   containment build [DIR] [-f FILE]

Reads the Containmentfile, compiles the entrypoint (if it is a ``.rs`` file), and imports the result as a named image.

**Options:**

- ``DIR`` - Build context directory (default: ``.``)
- ``-f, --file <FILE>`` - Containmentfile name (default: ``Containmentfile.toml``)

containment ps
--------------

List containers.

.. code-block:: text

   containment ps [-a]

By default, only shows running containers. Pass ``-a`` to include stopped and failed ones.

containment stop
----------------

Stop a running container by sending SIGTERM.

.. code-block:: text

   containment stop <ID>

Accepts a full container ID, short ID, or container name.

containment rm
--------------

Remove a stopped container record.

.. code-block:: text

   containment rm <ID>

containment prune
-----------------

Remove all stopped and failed container records.

.. code-block:: text

   containment prune

containment images
------------------

List images in the local store (``~/.containment/images/``).

.. code-block:: text

   containment images

containment import
------------------

Import a ``.wasm`` file as a named image.

.. code-block:: text

   containment import <NAME> <PATH>

containment rmi
---------------

Remove an image from the local store.

.. code-block:: text

   containment rmi <NAME>

containment inspect
-------------------

Show metadata about a WASM module: file size, exports, and WASI imports.

.. code-block:: text

   containment inspect <IMAGE>

containment info
----------------

Show system information: wasmtime version, data directory, available features.

.. code-block:: text

   containment info
