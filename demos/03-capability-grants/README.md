# Demo 03: Capability Grants

This demo shows what happens when you actually grant a capability. The program writes a file to /workspace/report.txt and reads it back, which only works because we mount a host directory into the sandbox with `-v`.

After the sandbox exits, the file is sitting on the host filesystem in the temp directory. This is the flip side of demos 01 and 02 - deny-by-default means nothing works, but a single volume mount opens exactly the access you intended and nothing more.

To run: `bash demo.sh`
