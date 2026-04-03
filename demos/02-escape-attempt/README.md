# Demo 02: Escape Attempt

This demo runs a program that actively tries to escape the sandbox by reading sensitive files, writing to the host filesystem, and reading environment variables.

Eight escape vectors are tested: /etc/passwd, /etc/shadow, /home, /root/.ssh, /proc/self/environ, writing to /tmp, and reading HOME and SSH_AUTH_SOCK. All are blocked because the WASM sandbox starts with nothing. These paths do not exist inside the sandbox, not because of permission checks, but because the WASI runtime provides no preopened directories or environment unless explicitly granted.

To run: `bash demo.sh`
