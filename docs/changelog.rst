Changelog
=========

0.1.0 (2026-04-02)
-------------------

Initial release.

- WASM sandbox with deny-by-default capabilities
- Docker-like CLI: run, build, ps, stop, rm, prune, images, import, rmi, inspect, info
- Capability grants: filesystem (read/write), network, environment variables
- Volume mounts (``-v host:guest``)
- CPU fuel limits and wall-clock timeouts
- Container state tracking
- Image store
- Build from JailFile (Rust source to wasm32-wasip1)
- Module inspection
- Optional bubblewrap outer sandbox (``--bwrap``)
