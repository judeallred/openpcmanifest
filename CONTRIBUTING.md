# Contributing to OpenPCManifest

Thanks for helping. This project stays useful only if it works on hardware its maintainers don't
own, so every contribution counts: a testing-grid row, a bug report with a redacted report, a
code-name table entry, or a new collector.

## Ways to help

| You have... | Do this |
| --- | --- |
| Five minutes and a PC | Run the latest release and [add a row to the testing grid](docs/TESTED_SYSTEMS.md) with a pull request. No coding needed. |
| A problem | [Open an issue](https://github.com/judeallred/openpcmanifest/issues/new/choose) with a **redacted** report (`openpcmanifest --text --redact`). |
| An idea | [Request a feature](https://github.com/judeallred/openpcmanifest/issues/new/choose). Describe the question you want answered first. |
| Some Rust | Pick an issue labelled `good first issue`, or see [Common changes](#common-changes) below. |

## Development setup

1. Install [Rust](https://rustup.rs/). The pinned toolchain in `rust-toolchain.toml` installs
   automatically the first time you run `cargo`.
2. Clone and build:

   ```powershell
   git clone https://github.com/judeallred/openpcmanifest
   cd openpcmanifest
   cargo run              # the window
   cargo run -- --summary # the CLI
   ```

3. On Linux or macOS you can work on everything except collection. Build without the GUI and use
   the fixture snapshots:

   ```sh
   cargo run --no-default-features -- --from-raw tests/fixtures/desktop.json
   ```

## Before you open a pull request

CI runs these on Windows and Linux, and all of them must pass:

```powershell
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

### The lint rules are strict on purpose

Clippy runs with the `all`, `pedantic`, `nursery` and `cargo` groups as errors, plus a set of
`restriction` lints (see `[lints]` in `Cargo.toml`). In practice:

- **No `unwrap`, `expect`, `panic!` or `[index]`** outside tests. Collectors read data from
  firmware and drivers that is sometimes wrong; use `.get()`, `?` and `Option` instead.
- **No `unsafe`**, except in the one documented FFI function in `src/main.rs`.
- **No `println!`**; write through `std::io::Write`.
- **Every `#[allow(...)]` needs a `reason = "..."`.** Prefer fixing the lint.
- **Every public item has a doc comment.**

If a lint is genuinely wrong for a case, allow it on the smallest item possible and explain why.

### Golden files

`tests/golden/` holds the expected text and JSON output for the fixture machines in
`tests/fixtures/`. If your change alters output on purpose, regenerate and review:

```powershell
$env:UPDATE_GOLDEN = 1; cargo test; Remove-Item Env:UPDATE_GOLDEN   # PowerShell
UPDATE_GOLDEN=1 cargo test                                          # bash
git diff tests/golden
```

Commit the updated golden files with your change, and say in the PR why the output changed.

## Common changes

### Show a new field

1. Find the WMI/CIM class that has it. Try it in PowerShell first:
   `Get-CimInstance Win32_VideoController | Select-Object *`.
2. Add the property to the matching query in `src/collect/collect.ps1`. Keep queries read-only;
   a test enforces that.
3. Read it in the section builder under `src/build/` with `text(...)` or `num(...)`, and push a
   `Node`. Mark anything that identifies a person or machine with `.private()`, and anything that
   changes from run to run (temperatures, free space) with `.volatile()`.
4. Add the field to `tests/fixtures/desktop.json` and a test assertion in `src/build/tests.rs`,
   then update the golden files.

### Add a CPU code name

Add a line to `code_name` in `src/cpu.rs` with the vendor, family and model (in hex) and the
process node. Cite your source in the PR.

### Add a monitor or graphics card brand

Extend `monitor_vendor` (EDID three-letter codes) or `pci_vendor` (PCI vendor IDs) in
`src/build/devices.rs`.

### Add a fixture machine

Real snapshots contain serial numbers and addresses. If you want to contribute one, take it with
`openpcmanifest --save-raw mypc.json`, then **replace every private value** (serials, MACs, IPs,
names, product IDs, SSIDs) with obviously fake ones before committing it under `tests/fixtures/`.

## Commit and PR style

- One logical change per PR; small PRs get reviewed fastest.
- Write commit messages in the imperative: "Add SODIMM form factor to RAM section".
- Add a line under **Unreleased** in `CHANGELOG.md` for anything a user would notice.

## Licensing

By contributing, you agree that your contribution is licensed under the [MIT License](LICENSE).
Only add dependencies with licences allowed in `deny.toml`; CI checks this.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). Be kind; assume good faith.
