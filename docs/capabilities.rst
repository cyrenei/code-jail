Capabilities
============

Cask uses a deny-by-default capability model. When a WASM module starts, it has no access to anything on the host. You grant capabilities explicitly with ``--cap`` flags, volume mounts, and environment flags.

This is fundamentally different from Docker, where containers share the host kernel and isolation is achieved by hiding things. In cask, there is nothing to hide because the program starts with nothing.

Capability types
----------------

Filesystem
^^^^^^^^^^

Grant access to specific host directories. Each grant creates a WASI preopened directory.

.. code-block:: bash

   --cap fs:read:/path       # Read-only access
   --cap fs:write:/path      # Read and write access
   --cap fs:/path            # Shorthand for read+write
   -v /host/path:/guest/path # Docker-style volume mount (read+write)

The guest path is how the sandboxed program sees the directory. With ``--cap``, the host and guest paths are the same. With ``-v``, you can map to a different path.

A program that tries to access any path not covered by a grant gets a "No such file or directory" error. There is no root filesystem, no /etc, no /home, no /tmp unless you mount one.

Network
^^^^^^^

Grant network access to specific destinations or to everything.

.. code-block:: bash

   --cap net:api.openai.com:443    # Allow one destination
   --cap net:192.168.1.100         # Allow one IP (all ports)
   --cap net:*                     # Allow everything
   --net                           # Shorthand for net:*

Without a network grant, the program cannot open any socket. DNS resolution is also blocked unless network access is granted (it requires ``allow_ip_name_lookup``).

Environment variables
^^^^^^^^^^^^^^^^^^^^^

Pass specific variables or set new ones.

.. code-block:: bash

   --cap env:HOME,PATH,API_KEY     # Pass listed vars from host
   -e API_KEY=sk-1234              # Set a specific value
   -e USER                         # Inherit USER from host

Without env grants, the program sees an empty environment. Not even HOME or PATH are set.

Stdio
^^^^^

By default, cask passes stdin, stdout, and stderr through to the host terminal. You can control this in a Caskfile:

.. code-block:: toml

   [capabilities]
   stdin = true
   stdout = true
   stderr = true

Capability composition
----------------------

Capabilities compose naturally. Each ``--cap`` flag adds to the set. Nothing is removed.

.. code-block:: bash

   cask run agent.wasm \
     --cap fs:read:/project \
     --cap fs:write:/project/output \
     --cap fs:write:/tmp \
     --cap net:api.openai.com:443 \
     --cap net:github.com:443 \
     --cap env:HOME,API_KEY \
     -e EDITOR=vim

This gives the program:

- Read access to /project
- Write access to /project/output and /tmp
- Network access to two specific hosts
- Three environment variables (HOME, API_KEY, EDITOR)
- Nothing else

Caskfile capabilities
---------------------

For reproducibility, put capabilities in a Caskfile instead of passing flags:

.. code-block:: bash

   cask run agent.wasm -f Caskfile.toml

CLI flags and Caskfile capabilities are merged. CLI grants add to whatever the Caskfile declares. See :doc:`caskfile` for the full format.

How it works under the hood
---------------------------

Each filesystem grant becomes a WASI preopened directory on the wasmtime ``WasiCtxBuilder``. The WASI runtime enforces path containment, so a program with access to ``/project`` cannot traverse to ``/project/../etc/passwd``.

Network grants are enforced by a ``socket_addr_check`` callback that runs before every outbound connection. The callback checks the destination against your allow list.

Environment variables are injected into the WASI context directly. The program calls ``environ_get`` and only sees what you provided.
