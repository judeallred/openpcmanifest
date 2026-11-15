# Releasing

## Builds at a glance

| Build | Trigger | Version label | Where |
| --- | --- | --- | --- |
| Nightly | Every push to `main`, and daily | `<version>-nightly.<YYYYMMDD>.<run>+<sha>` | The rolling [`nightly`](https://github.com/judeallred/openpcmanifest/releases/tag/nightly) pre-release, replaced each time |
| Release | Pushing a tag `vX.Y.Z` | `X.Y.Z` | A GitHub release with the zip, the bare exe and `SHA256SUMS.txt` |
| From source | `cargo run` / `cargo install --git` | `X.Y.Z` | Local |

The build label is compiled in from the `OPM_BUILD` environment variable and shown in the title
bar, the About box, `--version` and every report.

## Cutting a release

1. Update `version` in `Cargo.toml`.
2. Move the **Unreleased** notes in `CHANGELOG.md` under a new version heading with today's date.
3. Commit, then tag and push:

   ```powershell
   git tag v0.1.0
   git push origin main v0.1.0
   ```

4. The **Release** workflow checks that the tag matches `Cargo.toml`, runs the tests, builds,
   packages and creates the GitHub release.

## Package managers (planned for v1.0)

Publishing to crates.io, winget and Chocolatey is wired into `release.yml` but switched off. To turn
it on, set the repository variable `PUBLISH_PACKAGES` to `true` (Settings → Secrets and variables →
Actions → Variables) and add these secrets:

| Secret | Used for | How to get it |
| --- | --- | --- |
| `CARGO_REGISTRY_TOKEN` | `cargo publish` | crates.io → Account settings → API tokens |
| `WINGET_TOKEN` | Opening PRs on `microsoft/winget-pkgs` | A classic GitHub personal access token with `public_repo`, for an account that has forked `microsoft/winget-pkgs` |
| `CHOCO_API_KEY` | `choco push` | community.chocolatey.org → Account → API key |

### First winget submission (manual, once)

The automated job only updates an existing package. Submit the first version by hand:

```powershell
winget install Microsoft.WingetCreate
wingetcreate new https://github.com/judeallred/openpcmanifest/releases/download/v1.0.0/openpcmanifest-1.0.0-windows-x64.zip
```

Use the identifier `JudeAllred.OpenPCManifest`, installer type `zip` with a nested `portable`
installer `openpcmanifest.exe`, and command alias `openpcmanifest`. Microsoft reviews the PR;
after it merges, later releases are submitted automatically.

### First Chocolatey submission

The release job packs `packaging/chocolatey/` with the exe embedded and pushes it. The first version
of a new package goes through Chocolatey's community moderation, which can take several days;
later versions are usually approved automatically once the package is trusted.

## Code signing (planned for v1.0)

Builds are unsigned until v1.0. Options under consideration are the SignPath Foundation's free
signing for open-source projects and a separately purchased certificate (for example, Azure
Trusted Signing). Until then the README explains the SmartScreen prompt and how to verify checksums.
