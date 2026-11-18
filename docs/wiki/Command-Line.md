# Command Line

Run `openpcmanifest` with any option to get the command-line mode instead of the window. Open PowerShell or Command Prompt in the folder with the exe (or anywhere, if it's on your PATH).

## Examples

```powershell
openpcmanifest --summary --markdown            # short summary for a forum post
openpcmanifest --html --redact --out pc.html   # full report, private details hidden
openpcmanifest --json --section cpu,ram,storage
openpcmanifest --save-raw before.json          # keep a snapshot
openpcmanifest --compare before.json           # what changed since then
openpcmanifest --list-sections
```

## Options

| Option | Meaning |
| --- | --- |
| `--text`, `--markdown`, `--bbcode`, `--json`, `--html` | Output format. Default: text. |
| `--summary` | Only the short summary. |
| `--section <ids>` | Only these sections, comma-separated. See `--list-sections`. |
| `--list-sections` | List section IDs. |
| `--redact` | Hide serial numbers, addresses, names and keys. |
| `--out <file>` | Write to a file instead of the console. |
| `--save-raw <file>` | Also save the raw snapshot. |
| `--from-raw <file>` | Build the report from a saved snapshot instead of this PC. |
| `--compare <old> [new]` | Changes between two snapshots, or between one and this PC now. |
| `--gui` | Open the window. |
| `-h`, `--help` | Help. |
| `-V`, `--version` | Version and build. |

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | Invalid arguments |
| `2` | Collecting data or reading a snapshot failed |
| `3` | Could not write the output |

## JSON for scripts and inventory

`--json` output has a `schemaVersion`, stable section IDs and stable keys. The format is documented in [ARCHITECTURE.md](https://github.com/judeallred/openpcmanifest/blob/main/docs/ARCHITECTURE.md#json-report-format). For example, to list drive health across a fleet:

```powershell
$r = openpcmanifest --json --section storage | ConvertFrom-Json
$r.sections[0].items | ForEach-Object { "$($_.label): $(($_.children | Where-Object key -eq 'health').value)" }
```

Tip: when piping, use `--out` if your shell has trouble capturing output from a GUI-subsystem exe.
