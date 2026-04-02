Installation
============

From source
-----------

Cask is a Rust project. You need a working Rust toolchain (stable is fine).

.. code-block:: bash

   $ git clone https://github.com/cyrenei/agent-wasm-containers.git
   $ cd agent-wasm-containers
   $ cargo install --path .

This puts the ``cask`` binary in ``~/.cargo/bin/``. Make sure that is in your PATH.

WASM compilation target
-----------------------

If you want to build Rust source files into WASM modules (using ``cask build``), you also need the wasm32-wasip1 compilation target:

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

   $ cask info

This shows your runtime version, available features, and whether the wasm32-wasip1 target is installed.
