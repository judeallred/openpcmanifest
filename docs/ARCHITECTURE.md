# Architecture

OpenPCManifest is one Rust crate with a library and a single binary that is both the GUI and the
CLI. It is a four-stage pipeline; only the first stage touches Windows.

```text
 collect (Windows)         parse (pure)             build (pure)            render (pure)
 ─────────────────         ────────────             ────────────            ─────────────
 collect.ps1 ─┐            smbios.rs  SMBIOS        build/*.rs              render.rs
 CPUID ───────┴─► Snapshot ─► smart.rs  SMART  ───► Report tree ──redact──► text, Markdown,
                  (JSON)      cpu.rs    CPUID       (sections →             BBCode, JSON, HTML
                     ▲        json.rs   JSON         labelled nodes)        gui.rs (egui)
 --from-raw file ────┘        base64.rs                  │                  diff.rs
                                                         └──── compare two reports
```

| Module | Role |
| --- | --- |
| `collect/` | Runs `collect.ps1` with the inbox Windows PowerShell 5.1 and reads back one JSON document; adds CPUID leaves. Defines `Snapshot`. |
| `json.rs`, `base64.rs` | Small dependency-free readers for the collector's output. Accessors are forgiving: PowerShell sometimes emits an object where a list was expected. |
| `smbios.rs` | Decodes raw SMBIOS tables: BIOS, system, baseboard, processor, slots, memory arrays and devices. |
| `smart.rs` | Decodes ATA SMART attribute and threshold pages and produces a health verdict. |
| `cpu.rs` | Reads and decodes CPUID: brand, family/model/stepping, features, caches, code names. |
| `build/` | Turns a snapshot into a `Report`. One function per section; `insights.rs` holds the advisor-style sections. |
| `model.rs` | The report tree: `Report` → `Section` → `Node`, plus redaction, search filtering and flattening. |
| `render.rs` | Text, Markdown, BBCode, JSON and HTML output, and the short "rig summary". |
| `diff.rs` | Compares two reports by flattened paths and presents the changes as a report. |
| `cli.rs` | Argument parsing and the CLI workflow. |
| `gui.rs` | The egui window (behind the default `gui` feature). |

## Design decisions

**PowerShell for collection, for now.** Querying CIM/WMI from Rust means a large amount of unsafe
COM binding code. A single readable script lets contributors test any query by pasting it into a
terminal, and keeps the Rust side safe. The cost is about one to three seconds of startup and a
dependency on Windows PowerShell 5.1, which ships with every supported Windows version. The
`Snapshot` JSON is the seam: native collectors can replace queries one at a time without touching
anything downstream. WMIC is not used; Microsoft removed it from Windows 11 25H2.

**Raw snapshots.** The collector's JSON can be saved and reloaded. This makes bug reports
reproducible, lets the test suite run against fixture machines on Linux CI, and powers
`--compare`.

**One tree, many views.** Builders produce a single `Report`; the GUI and every format render the
same tree. Redaction is applied to the tree before rendering, so no renderer can leak a private
value it doesn't know about.

**No panics on bad data.** Firmware tables and driver output are often wrong. Parsers are
bounds-checked (`clippy::indexing_slicing` is denied) and fuzz-style tests feed them truncated and
random input.

**No kernel driver.** Everything comes from documented user-mode interfaces. This is why live
sensors are out of scope.

## Node flags

| Flag | Meaning | Used by |
| --- | --- | --- |
| `private` | Identifies the machine or its owner | Redaction |
| `volatile` | Changes between runs (temperatures, free space, uptime) | Snapshot diff ignores it |
| `status` | `good`, `caution`, `bad` or `note` | Colours in the GUI and HTML; JSON consumers |

Builders put summary lines in their own section under keys starting with `summary-`; `build()`
moves them into the Summary section.

## JSON report format

`--json` writes:

```json
{
  "schemaVersion": 1,
  "generator": "OpenPCManifest 0.1.0",
  "collectedAt": "2026-11-15T19:04:44.0000000Z",
  "notes": ["..."],
  "sections": [
    {
      "id": "storage",
      "title": "Storage",
      "items": [
        {
          "key": "disk-0",
          "label": "WDC WD40EZRZ-00GXCB0",
          "children": [
            { "key": "health", "label": "Health", "value": "Good", "status": "good" },
            { "key": "serial-number", "label": "Serial number", "value": "[redacted]", "private": true }
          ]
        }
      ]
    }
  ]
}
```

- `value`, `status` (omitted when normal), `private` (omitted when false) and `children` are
  optional.
- Section `id`s are listed by `--list-sections` and are stable. Node `key`s are stable within a
  section; siblings may share a key (for example, several drives).
- Breaking changes to this shape increment `schemaVersion`.

The **raw snapshot** format (`--save-raw`) is the collector's own output and is not a stable
interface; it carries its own `schema` number.

## Testing

- Unit tests sit next to the code they test.
- `src/build/tests.rs` builds reports from `tests/fixtures/*.json` (fictional machines) and checks
  specific values, redaction, and the golden files in `tests/golden/`.
- The Windows CI job also runs a live collection on the GitHub runner as a smoke test and keeps the
  redacted output as a build artifact.
