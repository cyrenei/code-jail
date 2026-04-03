# Demo 01: Sandbox Isolation

This demo shows what happens when a program runs inside codejail with zero capabilities granted.

The program tries to read the HOME environment variable and list the root filesystem. Both fail. The WASI runtime has no preopened directories and no environment variables unless you explicitly grant them with `--cap`, `-v`, or `-e` flags.

This is the foundation of codejail's security model: deny-by-default. Every subsequent demo builds on this by showing what happens when you selectively grant capabilities.

To run: `bash demo.sh`
