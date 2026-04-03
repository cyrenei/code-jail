Installation
============

Quick install (recommended)
---------------------------

The fastest way to get codejail is the install script. It downloads a pre-built binary from GitHub Releases, verifies the SHA256 checksum, and puts it in ``~/.codejail/bin/``.

.. code-block:: bash

   $ curl -sSf https://raw.githubusercontent.com/cyrenei/code-jail/main/install.sh | sh

The script detects your OS (Linux, macOS) and architecture (amd64, arm64) automatically.

To pin a specific version or change the install directory:

.. code-block:: bash

   $ CODEJAIL_VERSION=v0.1.0 curl -sSf \
       https://raw.githubusercontent.com/cyrenei/code-jail/main/install.sh | sh

   # Or install somewhere else
   $ CODEJAIL_INSTALL_DIR=/usr/local/bin curl -sSf \
       https://raw.githubusercontent.com/cyrenei/code-jail/main/install.sh | sh

The script will offer to add ``~/.codejail/bin`` to your PATH if it is not there already.

From source (cargo)
-------------------

If you have the Rust toolchain installed:

.. code-block:: bash

   $ git clone https://github.com/cyrenei/code-jail.git
   $ cd codejail
   $ cargo install --path .

This puts the ``codejail`` binary in ``~/.cargo/bin/``. Make sure that is in your PATH.

Docker
------

You can run codejail in a Docker container without installing anything locally. The image includes bubblewrap and everything needed.

.. code-block:: bash

   $ docker build -t codejail .
   $ docker run --rm codejail info

To run WASM modules from a host directory, mount it as a volume:

.. code-block:: bash

   $ docker run --rm -v ./workspace:/data/workspace codejail run /data/workspace/program.wasm

For ``--bwrap`` support inside Docker, grant the ``SYS_ADMIN`` capability:

.. code-block:: bash

   $ docker run --rm --cap-add SYS_ADMIN --security-opt apparmor=unconfined \
       codejail run --bwrap program.wasm

WASM compilation target
-----------------------

If you want to build Rust source files into WASM modules (using ``codejail build``), you also need the wasm32-wasip1 compilation target:

.. code-block:: bash

   $ rustup target add wasm32-wasip1

This is optional. You can always run pre-compiled ``.wasm`` files without it.

Optional: bubblewrap
--------------------

For the ``--bwrap`` flag (defense-in-depth namespace isolation), install bubblewrap:

.. code-block:: bash

   # Debian/Ubuntu
   $ sudo apt install bubblewrap

   # Fedora
   $ sudo dnf install bubblewrap

   # Arch
   $ sudo pacman -S bubblewrap

This is optional. The WASM sandbox works fine without it.

Verify the install
------------------

.. code-block:: bash

   $ codejail info

This shows your runtime version, available features, and whether the wasm32-wasip1 target is installed.
