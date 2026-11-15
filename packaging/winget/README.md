# winget

winget manifests live in [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs),
not in this repository. The package identifier is `JudeAllred.OpenPCManifest`.

The first version is submitted by hand with `wingetcreate new` (see
[docs/RELEASING.md](../../docs/RELEASING.md)). After that, the `winget` job in
`.github/workflows/release.yml` submits each new release automatically.
