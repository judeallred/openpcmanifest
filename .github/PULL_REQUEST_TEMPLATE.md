## What this changes

<!-- One or two sentences. Link the issue it fixes, if any: "Fixes #123". -->

## How I tested it

<!-- Machines, Windows versions, admin or not. For collector changes, paste a
     redacted before/after (`openpcmanifest --text --redact`). -->

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --all-features` passes
- [ ] Golden files updated with `UPDATE_GOLDEN=1 cargo test` and the diff reviewed (if output changed)
- [ ] New private values (serials, addresses, names) are marked `.private()`
- [ ] `CHANGELOG.md` updated under *Unreleased* (for user-visible changes)
