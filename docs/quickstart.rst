Quick start
===========

Your first sandbox (with policy enforcement)
---------------------------------------------

The recommended way to run codejail is with policy enforcement enabled. Start with a minimal policy file:

.. code-block:: bash

   $ cat > policy.toml << 'EOF'
   [[policies]]
   id = "allow-read"
   effect = "allow"
   allowed_tools = ["fs_read", "env_read"]

   [policies.intent_match]
   keywords = ["read", "analyze"]
   EOF

Now run a WASM module with the policy engine evaluating every capability grant:

.. code-block:: bash

   $ codejail run hello.wasm --policy policy.toml
   a1b2c3d4-e5f
   [codejail] policy mode: policy policy.toml
   [codejail] Sandbox: hello-8f3a2b1c
   [codejail] Image:   /path/to/hello.wasm
   [codejail]   (no capabilities granted, fully isolated)

   Hello from WASM sandbox!

The module ran with zero capabilities. The policy engine had nothing to evaluate because no grants were requested. Now request a capability:

.. code-block:: bash

   $ codejail run analyzer.wasm \
       --policy policy.toml \
       --intent "read and analyze" \
       --cap fs:read:/home/you/project

   [codejail] policy mode: policy policy.toml
   [codejail]   [+] fs_read: allowed by policy allow-read
   [codejail] session abc123 (agent def456, intent: 'read and analyze')

The ``[+]`` means the policy allowed the grant. If you try a capability the policy doesn't allow:

.. code-block:: bash

   $ codejail run agent.wasm \
       --policy policy.toml \
       --intent "read and analyze" \
       --cap fs:write:/tmp/output

   [codejail] policy mode: policy policy.toml
   [codejail]   [x] fs_write: no matching policy found (deny-by-default)
   [codejail] policy denied 1 of 1 capability requests

The write grant was denied. The sandbox still runs, but without write access. This is the gap between requesting and receiving -- the policy engine sits in that gap.

Drift detection
---------------

Declare your intent and the policy engine flags mismatches:

.. code-block:: bash

   $ codejail run agent.wasm \
       --policy policy.toml \
       --intent "read and analyze source code" \
       --audit-log audit.jsonl \
       --cap fs:read:/project \
       --cap fs:write:/tmp

   [codejail]   [+] fs_read: allowed by policy allow-read
   [codejail]   [x] fs_write: no matching policy found (deny-by-default)
   [codejail]   drift detected: fs_write (Write) vs intent 'read and analyze source code'

Even if a policy had allowed the write, drift detection would still flag it because writing contradicts a read intent. The audit log records both the decision and the drift flag.

Granting capabilities
---------------------

Most programs need to read or write files. Use ``--cap`` to request access (the policy engine evaluates each one):

.. code-block:: bash

   # Read-only access to a directory
   $ codejail run analyzer.wasm --policy policy.toml --cap fs:read:/home/you/project

   # Read+write access
   $ codejail run builder.wasm --policy policy.toml --cap fs:write:/tmp/output

   # Volume mount (Docker-style, always read+write)
   $ codejail run agent.wasm --policy policy.toml -v ./project:/workspace

You can also grant network and environment access:

.. code-block:: bash

   $ codejail run agent.wasm \
       --policy policy.toml \
       --cap net:api.openai.com:443 \
       -e API_KEY=sk-1234

Audit log
---------

Pass ``--audit-log`` to get structured JSONL records of every policy decision:

.. code-block:: bash

   $ codejail run agent.wasm \
       --policy policy.toml \
       --audit-log audit.jsonl \
       --cap fs:read:/project

   $ cat audit.jsonl
   {"timestamp":"...","tool_called":"fs_read","authorization_decision":"allow","policy_matched":"allow-read",...}

Every decision -- allowed or denied -- is recorded with the timestamp, tool name, arguments, matched policy, agent ID, and session ID.

Building from Rust source
-------------------------

If you have a Rust file, you can compile it to WASM and import it as an image in one step:

.. code-block:: bash

   $ cat > hello.rs << 'EOF'
   fn main() {
       println!("Hello from the sandbox!");
   }
   EOF

   $ cat > JailFile.toml << 'EOF'
   [sandbox]
   name = "hello"
   entrypoint = "hello.rs"

   [capabilities]
   stdout = true
   EOF

   $ codejail build .
   [codejail] Compiling hello.rs -> wasm32-wasip1...
   [codejail] Image 'hello' ready (1.9 MB)

   $ codejail run hello --policy policy.toml

Managing images and containers
------------------------------

Import a pre-compiled WASM file:

.. code-block:: bash

   $ codejail import myapp /path/to/app.wasm
   Imported: myapp (2.1 MB)

List images:

.. code-block:: bash

   $ codejail images
   NAME                 SIZE         PATH
   hello                1.9 MB       /home/you/.codejail/images/hello.wasm
   myapp                2.1 MB       /home/you/.codejail/images/myapp.wasm

See past runs:

.. code-block:: bash

   $ codejail ps -a
   CONTAINER ID   NAME            STATUS         CREATED              IMAGE
   a1b2c3d4-e5f   hello-8f3a2b1c  Exited (0)     2026-04-02 19:48:01  hello

Clean up:

.. code-block:: bash

   $ codejail prune
   Removed 5 stopped container(s)

Inspect a module
----------------

See what a WASM module imports and exports:

.. code-block:: bash

   $ codejail inspect hello
   Image:   hello
   Path:    /home/you/.codejail/images/hello.wasm
   Size:    1.9 MB

   Exports (3):
     memory (...)
     _start (...)
     __main_void (...)

   Imports (4):
     wasi_snapshot_preview1::fd_write (...)
     wasi_snapshot_preview1::proc_exit (...)
     ...

The imports tell you what WASI functions the module needs. If a module imports something codejail does not support, it will fail at link time with a clear error.

Quick start without policy enforcement (simple mode)
-----------------------------------------------------

.. note::

   Simple mode runs without policy enforcement, drift detection, session tracking, or audit logging. It is **not recommended** for production use or when running untrusted agents.

You can run codejail without a policy for quick local testing:

.. code-block:: bash

   $ codejail run hello.wasm
   $ codejail run analyzer.wasm --cap fs:read:/home/you/project
   $ codejail run agent.wasm -v ./project:/workspace --cap net:api.openai.com:443 -e API_KEY

In simple mode, the operator's flags are the only authorization layer. There is no gap between requesting a capability and receiving it.
