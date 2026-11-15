# Security policy

## Supported versions

Security fixes go into the latest release and the nightly build. Older releases are not patched;
please update.

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Instead, use GitHub's
[private vulnerability reporting](https://github.com/judeallred/openpcmanifest/security/advisories/new)
for this repository. Include what you found, how to reproduce it, and its impact.

You should get an acknowledgement within a week. Once a fix is released, the advisory is published
with credit to you unless you prefer otherwise.

## Scope notes

- OpenPCManifest runs one embedded, read-only PowerShell script with
  `-ExecutionPolicy Bypass` for that process only. The script is written to a fresh temporary
  folder, run by the full path to the inbox `powershell.exe`, and deleted afterwards.
- The program contains no networking code. Help links open in your browser only when clicked.
- Raw snapshots are deliberately unredacted. Treat them like any file containing serial numbers
  and network addresses.
