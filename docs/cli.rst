CLI reference
=============

cask run
--------

Run a WASM module in a capability-restricted sandbox.

.. code-block:: text

   cask run [OPTIONS] <IMAGE> [-- <ARGS>...]

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
   * - ``-f, --caskfile <PATH>``
     - Load capabilities from a Caskfile. See :doc:`caskfile`.
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
   $ cask run hello.wasm

   # With filesystem and network
   $ cask run agent.wasm \
       --cap fs:read:/project \
       --cap net:api.openai.com:443 \
       -v /tmp/out:/output \
       -e API_KEY

   # With a Caskfile
   $ cask run agent.wasm -f Caskfile.toml

cask build
----------

Build an image from a Caskfile.

.. code-block:: text

   cask build [DIR] [-f FILE]

Reads the Caskfile, compiles the entrypoint (if it is a ``.rs`` file), and imports the result as a named image.

**Options:**

- ``DIR`` - Build context directory (default: ``.``)
- ``-f, --file <FILE>`` - Caskfile name (default: ``Caskfile.toml``)

cask ps
-------

List containers.

.. code-block:: text

   cask ps [-a]

By default, only shows running containers. Pass ``-a`` to include stopped and failed ones.

cask stop
---------

Stop a running container by sending SIGTERM.

.. code-block:: text

   cask stop <ID>

Accepts a full container ID, short ID, or container name.

cask rm
-------

Remove a stopped container record.

.. code-block:: text

   cask rm <ID>

cask prune
----------

Remove all stopped and failed container records.

.. code-block:: text

   cask prune

cask images
-----------

List images in the local store (``~/.cask/images/``).

.. code-block:: text

   cask images

cask import
-----------

Import a ``.wasm`` file as a named image.

.. code-block:: text

   cask import <NAME> <PATH>

cask rmi
--------

Remove an image from the local store.

.. code-block:: text

   cask rmi <NAME>

cask inspect
------------

Show metadata about a WASM module: file size, exports, and WASI imports.

.. code-block:: text

   cask inspect <IMAGE>

cask info
---------

Show system information: wasmtime version, data directory, available features.

.. code-block:: text

   cask info
