Quick start
===========

Your first sandbox (with arbiter)
---------------------------------

The recommended way to run containment is with arbiter policy enforcement. Start with a minimal policy file:

.. code-block:: bash

   $ cat > policy.toml << 'EOF'
   [[policies]]
   id = "allow-read"
   effect = "allow"
   allowed_tools = ["fs_read", "env_read"]

   [policies.intent_match]
   keywords = ["read", "analyze"]
   EOF

Now run a WASM module with arbiter evaluating every capability grant:

.. code-block:: bash

   $ containment run hello.wasm --arbiter policy.toml
   a1b2c3d4-e5f
   [containment] arbiter mode: policy policy.toml
   [containment] Sandbox: hello-8f3a2b1c
   [containment] Image:   /path/to/hello.wasm
   [containment]   (no capabilities granted, fully isolated)

   Hello from WASM sandbox!

The module ran with zero capabilities. Arbiter had nothing to evaluate because no grants were requested. Now request a capability:

.. code-block:: bash

   $ containment run analyzer.wasm \
       --arbiter policy.toml \
       --intent "read and analyze" \
       --cap fs:read:/home/you/project

   [containment] arbiter mode: policy policy.toml
   [containment]   [+] fs_read: allowed by policy allow-read
   [containment] session abc123 (agent def456, intent: 'read and analyze')

The ``[+]`` means arbiter's policy allowed the grant. If you try a capability the policy doesn't allow:

.. code-block:: bash

   $ containment run agent.wasm \
       --arbiter policy.toml \
       --intent "read and analyze" \
       --cap fs:write:/tmp/output

   [containment] arbiter mode: policy policy.toml
   [containment]   [x] fs_write: no matching policy found (deny-by-default)
   [containment] arbiter denied 1 of 1 capability requests

The write grant was denied. The sandbox still runs, but without write access. This is the gap between requesting and receiving — arbiter policy sits in that gap.

Drift detection
---------------

Declare your intent and arbiter flags mismatches:

.. code-block:: bash

   $ containment run agent.wasm \
       --arbiter policy.toml \
       --intent "read and analyze source code" \
       --audit-log audit.jsonl \
       --cap fs:read:/project \
       --cap fs:write:/tmp

   [containment]   [+] fs_read: allowed by policy allow-read
   [containment]   [x] fs_write: no matching policy found (deny-by-default)
   [containment]   drift detected: fs_write (Write) vs intent 'read and analyze source code'

Even if a policy had allowed the write, drift detection would still flag it because writing contradicts a read intent. The audit log records both the decision and the drift flag.

Granting capabilities
---------------------

Most programs need to read or write files. Use ``--cap`` to request access (arbiter evaluates each one):

.. code-block:: bash

   # Read-only access to a directory
   $ containment run analyzer.wasm --arbiter policy.toml --cap fs:read:/home/you/project

   # Read+write access
   $ containment run builder.wasm --arbiter policy.toml --cap fs:write:/tmp/output

   # Volume mount (Docker-style, always read+write)
   $ containment run agent.wasm --arbiter policy.toml -v ./project:/workspace

You can also grant network and environment access:

.. code-block:: bash

   $ containment run agent.wasm \
       --arbiter policy.toml \
       --cap net:api.openai.com:443 \
       -e API_KEY=sk-1234

Audit log
---------

Pass ``--audit-log`` to get structured JSONL records of every arbiter decision:

.. code-block:: bash

   $ containment run agent.wasm \
       --arbiter policy.toml \
       --audit-log audit.jsonl \
       --cap fs:read:/project

   $ cat audit.jsonl
   {"timestamp":"...","tool_called":"fs_read","authorization_decision":"allow","policy_matched":"allow-read",...}

Every decision — allowed or denied — is recorded with the timestamp, tool name, arguments, matched policy, agent ID, and session ID.

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

   $ containment run hello --arbiter policy.toml

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

Quick start without arbiter (simple mode)
-----------------------------------------

.. note::

   Simple mode runs without policy enforcement, drift detection, session tracking, or audit logging. It is **not recommended** for production use or when running untrusted agents.

You can run containment without arbiter for quick local testing:

.. code-block:: bash

   $ containment run hello.wasm
   $ containment run analyzer.wasm --cap fs:read:/home/you/project
   $ containment run agent.wasm -v ./project:/workspace --cap net:api.openai.com:443 -e API_KEY

In simple mode, the operator's flags are the only authorization layer. There is no gap between requesting a capability and receiving it.
