CLI reference
=============

codejail run
------------

Run a WASM module in a capability-restricted sandbox.

.. code-block:: text

   codejail run [OPTIONS] <IMAGE> [-- <ARGS>...]

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
   * - ``-f, --jailfile <PATH>``
     - Load capabilities from a JailFile. See :doc:`jailfile`.
   * - ``--name <NAME>``
     - Set the container name (default: auto-generated).
   * - ``--fuel <N>``
     - CPU fuel limit (default: 1000000000). Set to 0 for unlimited.
   * - ``--timeout <SECS>``
     - Wall-clock timeout in seconds (default: 300).
   * - ``--policy <PATH>``
     - Policy file for capability authorization (enables policy mode).
   * - ``--intent <DESC>``
     - Declared intent for drift detection (default: general).
   * - ``--audit-log <PATH>``
     - Write policy decisions to a structured JSONL audit log.
   * - ``--bwrap``
     - Enable bubblewrap outer sandbox.
   * - ``-d, --detach``
     - Run in background (not yet implemented).

**Examples:**

.. code-block:: bash

   # Minimal: just stdout/stderr
   $ codejail run hello.wasm

   # With filesystem and network
   $ codejail run agent.wasm \
       --cap fs:read:/project \
       --cap net:api.openai.com:443 \
       -v /tmp/out:/output \
       -e API_KEY

   # With a JailFile
   $ codejail run agent.wasm -f JailFile.toml

   # With policy enforcement
   $ codejail run agent.wasm --policy policy.toml --intent "read and analyze"

codejail build
--------------

Build an image from a JailFile.

.. code-block:: text

   codejail build [DIR] [-f FILE]

Reads the JailFile, compiles the entrypoint (if it is a ``.rs`` file), and imports the result as a named image.

**Options:**

- ``DIR`` - Build context directory (default: ``.``)
- ``-f, --file <FILE>`` - JailFile name (default: ``JailFile.toml``)

codejail ps
-----------

List containers.

.. code-block:: text

   codejail ps [-a]

By default, only shows running containers. Pass ``-a`` to include stopped and failed ones.

codejail stop
-------------

Stop a running container by sending SIGTERM.

.. code-block:: text

   codejail stop <ID>

Accepts a full container ID, short ID, or container name.

codejail rm
-----------

Remove a stopped container record.

.. code-block:: text

   codejail rm <ID>

codejail prune
--------------

Remove all stopped and failed container records.

.. code-block:: text

   codejail prune

codejail images
---------------

List images in the local store (``~/.codejail/images/``).

.. code-block:: text

   codejail images

codejail import
---------------

Import a ``.wasm`` file as a named image.

.. code-block:: text

   codejail import <NAME> <PATH>

codejail rmi
------------

Remove an image from the local store.

.. code-block:: text

   codejail rmi <NAME>

codejail inspect
----------------

Show metadata about a WASM module: file size, exports, and WASI imports.

.. code-block:: text

   codejail inspect <IMAGE>

codejail info
-------------

Show system information: wasmtime version, data directory, available features.

.. code-block:: text

   codejail info
