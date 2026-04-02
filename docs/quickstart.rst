Quick start
===========

Your first sandbox
------------------

The simplest thing you can do is run a WASM module:

.. code-block:: bash

   $ containment run hello.wasm
   a1b2c3d4-e5f
   [containment] Sandbox: hello-8f3a2b1c
   [containment] Image:   /path/to/hello.wasm
   [containment]   (no capabilities granted, fully isolated)

   Hello from WASM sandbox!

That first line is the container ID (like Docker). The module ran with zero capabilities. It could print to stdout and stderr, and that was it. No filesystem, no network, no environment variables.

Granting capabilities
---------------------

Most programs need to read or write files. Use ``--cap`` to grant specific access:

.. code-block:: bash

   # Read-only access to a directory
   $ containment run analyzer.wasm --cap fs:read:/home/you/project

   # Read+write access
   $ containment run builder.wasm --cap fs:write:/tmp/output

   # Volume mount (Docker-style, always read+write)
   $ containment run agent.wasm -v ./project:/workspace

You can also grant network and environment access:

.. code-block:: bash

   $ containment run agent.wasm \
       --cap net:api.openai.com:443 \
       -e API_KEY=sk-1234

Building from Rust source
-------------------------

If you have a Rust file, you can compile it to WASM and import it as an image in one step:

.. code-block:: bash

   $ cat > hello.rs << 'EOF'
   fn main() {
       println!("Hello from the sandbox!");
   }
   EOF

   $ cat > Containmentfile.toml << 'EOF'
   [sandbox]
   name = "hello"
   entrypoint = "hello.rs"

   [capabilities]
   stdout = true
   EOF

   $ containment build .
   [containment] Compiling hello.rs -> wasm32-wasip1...
   [containment] Image 'hello' ready (1.9 MB)

   $ containment run hello

Managing images and containers
------------------------------

Import a pre-compiled WASM file:

.. code-block:: bash

   $ containment import myapp /path/to/app.wasm
   Imported: myapp (2.1 MB)

List images:

.. code-block:: bash

   $ containment images
   NAME                 SIZE         PATH
   hello                1.9 MB       /home/you/.containment/images/hello.wasm
   myapp                2.1 MB       /home/you/.containment/images/myapp.wasm

See past runs:

.. code-block:: bash

   $ containment ps -a
   CONTAINER ID   NAME            STATUS         CREATED              IMAGE
   a1b2c3d4-e5f   hello-8f3a2b1c  Exited (0)     2026-04-02 19:48:01  hello

Clean up:

.. code-block:: bash

   $ containment prune
   Removed 5 stopped container(s)

Inspect a module
----------------

See what a WASM module imports and exports:

.. code-block:: bash

   $ containment inspect hello
   Image:   hello
   Path:    /home/you/.containment/images/hello.wasm
   Size:    1.9 MB

   Exports (3):
     memory (...)
     _start (...)
     __main_void (...)

   Imports (4):
     wasi_snapshot_preview1::fd_write (...)
     wasi_snapshot_preview1::proc_exit (...)
     ...

The imports tell you what WASI functions the module needs. If a module imports something containment does not support, it will fail at link time with a clear error.
