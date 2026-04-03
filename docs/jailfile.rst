JailFile reference
==================

A JailFile is a TOML file that declares the capabilities and limits for a sandbox. It is the equivalent of a Dockerfile, but for permissions rather than build steps.

Format
------

.. code-block:: toml

   [sandbox]
   name = "my-agent"
   entrypoint = "agent.wasm"

   [capabilities]
   fs_read = ["/project"]
   fs_write = ["/project/output", "/tmp"]
   net_allow = ["api.openai.com:443", "github.com:443"]
   env = ["HOME", "PATH", "API_KEY"]
   inherit_env = false
   stdin = true
   stdout = true
   stderr = true

   [limits]
   fuel = 1_000_000_000
   wall_time_secs = 300
   memory_mb = 512

Sections
--------

[sandbox]
^^^^^^^^^

.. list-table::
   :header-rows: 1

   * - Field
     - Required
     - Description
   * - ``name``
     - No
     - Image name. Defaults to the entrypoint filename without extension.
   * - ``entrypoint``
     - Yes
     - Path to the WASM module or Rust source file (relative to build context).

[capabilities]
^^^^^^^^^^^^^^

.. list-table::
   :header-rows: 1

   * - Field
     - Default
     - Description
   * - ``fs_read``
     - ``[]``
     - Directories with read-only access.
   * - ``fs_write``
     - ``[]``
     - Directories with read+write access.
   * - ``net_allow``
     - ``[]``
     - Allowed network destinations. Use ``"*"`` for unrestricted.
   * - ``env``
     - ``[]``
     - Environment variable names to pass from the host.
   * - ``inherit_env``
     - ``false``
     - If true, pass all host environment variables.
   * - ``stdin``
     - ``true``
     - Connect stdin to the host terminal.
   * - ``stdout``
     - ``true``
     - Connect stdout to the host terminal.
   * - ``stderr``
     - ``true``
     - Connect stderr to the host terminal.

[limits]
^^^^^^^^

.. list-table::
   :header-rows: 1

   * - Field
     - Default
     - Description
   * - ``fuel``
     - ``1_000_000_000``
     - CPU fuel budget (wasmtime fuel units). Roughly a few seconds of compute.
   * - ``wall_time_secs``
     - ``300``
     - Maximum wall-clock time before the sandbox is killed.
   * - ``memory_mb``
     - ``256``
     - Memory limit in megabytes (reserved for future use).

Usage
-----

Build an image from a JailFile:

.. code-block:: bash

   $ codejail build .
   $ codejail build /path/to/project -f custom-jailfile.toml

Run with a JailFile (apply its capabilities):

.. code-block:: bash

   $ codejail run my-image -f JailFile.toml

When you use ``-f``, the JailFile capabilities are the base set. Any ``--cap`` flags you add on the command line are merged on top.
