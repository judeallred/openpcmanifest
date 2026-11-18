# OpenPCManifest

**An open-source Speccy alternative: find out exactly what's inside your Windows PC — free, portable, and with nothing bundled.**

[![CI](https://github.com/judeallred/openpcmanifest/actions/workflows/ci.yml/badge.svg)](https://github.com/judeallred/openpcmanifest/actions/workflows/ci.yml)
[![Nightly](https://github.com/judeallred/openpcmanifest/actions/workflows/nightly.yml/badge.svg)](https://github.com/judeallred/openpcmanifest/releases/tag/nightly)
[![Latest release](https://img.shields.io/github/v/release/judeallred/openpcmanifest?sort=semver)](https://github.com/judeallred/openpcmanifest/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

OpenPCManifest is a hardware inventory for Windows. One click shows your CPU, memory and free
RAM slots, motherboard and expansion slots, graphics card and monitors, drives and their health,
audio, peripherals, USB devices and network adapters. It also answers the questions people
usually open a tool like this to ask:

- *Can I add more RAM, and what kind?*
- *Is there a free PCIe slot for that card?*
- *Is this drive about to fail?*
- *Can this PC run Windows 11?*
- *Which drivers are years out of date, and which devices have problems?*

It is a single portable `.exe`. There is no installer, no account, no telemetry, no ads and no
bundled software, and the program never connects to the internet. It is MIT licensed, so you can
read every line of what it runs.

> **Status: early (v0.1).** Builds are unsigned until v1.0, so Windows SmartScreen may warn the
> first time you run one. See [Download](#download).

---

## Contents

- [What you get](#what-you-get)
- [Download](#download)
- [Supported systems](#supported-systems)
- [Using it](#using-it)
- [Privacy](#privacy)
- [How it works](#how-it-works)
- [Help us test: the testing grid](#help-us-test-the-testing-grid)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

## What you get

**Everything Speccy shows about your hardware**, section by section:

| Section | Highlights |
| --- | --- |
| Summary | One line per component, ready to paste into a forum post |
| Operating System | Edition, version and build, install date, uptime, UAC, firewall and antivirus state, runtimes, power plan, installed updates |
| CPU | Model, code name and process node, cores and threads, socket, instruction sets, virtualization state, cache layout |
| RAM | Total, used and free slots, type, channels, speed, maximum supported, and each module's maker, part number, speed and voltage |
| Motherboard | Maker, model and revision, BIOS version and age, and every expansion slot with its type, lane width and whether it's free |
| Graphics | GPU, real video memory (not Windows' 4 GB-capped figure), card maker, driver version and age; monitors with model, size and manufacture date |
| Storage | Every drive with interface, capacity, firmware, a **Good / Caution / Bad health verdict with the reason**, temperature, power-on hours, the full SMART table, and partitions |
| Optical, Audio, Peripherals, USB, Network | Drives, sound devices and endpoints, keyboards, mice and printers, USB devices by class with vendor and product IDs, adapters, addresses, Wi-Fi link details |

**Plus things Speccy doesn't do:**

- **Upgrade Advisor** — free RAM slots and how much more the board takes, memory running slower than it's rated for (a missing XMP/EXPO setting), free PCIe slots with lane widths, and which drive to replace first.
- **Drive health in plain words** — "Caution: 16 reallocation events" instead of a wall of SMART numbers.
- **Device Problems** — every device Device Manager is unhappy about, with what the error code means.
- **Drivers & Firmware Age** — drivers and BIOS older than three years, flagged.
- **Windows 11 Readiness** — TPM, Secure Boot, UEFI, CPU, memory and disk checks, plus the 2023 Secure Boot certificate status.
- **Search** across every section.
- **Redact for sharing** — one switch hides serial numbers, MAC and IP addresses, product IDs, Wi-Fi network names and computer and user names. Exports and copies are redacted by default.
- **Share anywhere** — copy a short summary as Markdown or forum BBCode, or save the full report as HTML, Markdown, BBCode, text or JSON.
- **Snapshots and compare** — save a snapshot, then later see exactly what changed: *"BIOS F1 → F3"*, *"new drive"*.
- **A full command-line interface** for scripts and IT inventory.

A summary, as it would look pasted into a forum:

```text
Operating system: Windows 11 Pro 64-bit (24H2, build 26100.7019)
CPU: AMD Ryzen 5 3600 6-Core Processor (Matisse, 7 nm) — 6 cores / 12 threads
RAM: 48 GB DDR4 dual-channel @ 2133 MT/s (1066 MHz) (4 of 4 slots used)
Motherboard: Gigabyte Technology Co., Ltd. B550 EAGLE WIFI6 (AM4)
Graphics: LG HDR WQHD (3840 × 1600 @ 60 Hz, 32-bit); NVIDIA GeForce GTX 1070 Ti, 8 GB
Storage: 4 TB WDC WD40EZRZ-00GXCB0 (SATA HDD) — Good; 512 GB Crucial_CT512M550SSD1 (SATA SSD) — Caution; …
```

**What it deliberately doesn't do:** live monitoring of temperatures, voltages and fan speeds over
time. That needs a kernel driver, and doing it well is a project of its own. If you need it, use
[LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) (open
source) or [HWiNFO](https://www.hwinfo.com/). OpenPCManifest also has no "cleaner", "booster" or
tweak tools, and never will.

## Download

| Channel | What it is | Where |
| --- | --- | --- |
| **Stable** | Tagged, tested releases | [Latest release](https://github.com/judeallred/openpcmanifest/releases/latest) |
| **Nightly** | Built from `main` on every change; labelled like `0.1.0-nightly.20261115.42+1a2b3c4` | [Nightly](https://github.com/judeallred/openpcmanifest/releases/tag/nightly) |
| **From source** | Build it yourself with Rust | See below |

Download the `.exe` (or the `.zip`) and run it. Nothing is installed. Every release lists SHA-256
checksums in `SHA256SUMS.txt`.

Pick **`windows-x64`** for 64-bit Windows (almost every PC since 2010) or **`windows-x86`** for older
32-bit machines.

**SmartScreen warning?** Builds are not code-signed yet (signing is planned for v1.0). Windows may
say it "protected your PC". Choose **More info → Run anyway**, or verify the checksum first:

```powershell
Get-FileHash .\openpcmanifest-0.1.0-windows-x64.exe -Algorithm SHA256
```

**winget and Chocolatey** packages arrive with v1.0. The packaging is already in this repository.

### Run from source

You need [Rust](https://rustup.rs/). The repository pins its toolchain in `rust-toolchain.toml`,
so `rustup` installs the right version automatically.

```powershell
git clone https://github.com/judeallred/openpcmanifest
cd openpcmanifest
cargo run --release                      # opens the window
cargo run --release -- --summary         # prints the summary
```

Or install straight from GitHub:

```powershell
cargo install --git https://github.com/judeallred/openpcmanifest
```

## Supported systems

| | Minimum | Notes |
| --- | --- | --- |
| **Windows** | Windows 10 version 1809, Windows 11 | 64-bit (x64) and 32-bit (x86) builds |
| **Windows Server** | 2019, 2022, 2025 | x64, with Desktop Experience for the GUI; the CLI should also work on Server Core |
| **Windows PowerShell** | 5.1 | Built into every supported version; nothing to install |
| **Administrator rights** | Optional | Without them, SMART attributes, drive reliability counters and TPM details are skipped and the report says so |

Windows on ARM, Linux and macOS collectors are not available yet. On other platforms the tool can
still open and compare snapshots taken on Windows.

These are the versions OpenPCManifest is designed for. What has actually been tested is recorded
in the [testing grid](docs/TESTED_SYSTEMS.md).

## Using it

### The window

Double-click `openpcmanifest.exe`. The window opens immediately and fills in as your hardware is
read, with Summary, CPU, RAM and Motherboard first; everything is in within a few seconds. Sections
are on the left, details on the right, and everything is searchable from the box at the top.

- **Copy summary** puts a short "my PC" block on the clipboard as Markdown, BBCode or text.
- **File → Save report as** writes HTML, Markdown, BBCode, text or JSON.
- **Hide private details** switches redaction on for the screen. Exports and copies are redacted
  unless you turn that off under **View**.
- **File → Save snapshot** keeps the raw data; **File → Compare with snapshot** shows what changed.
- **Help** links to the [user guide](https://github.com/judeallred/openpcmanifest/wiki) and to
  [report a bug](https://github.com/judeallred/openpcmanifest/issues/new/choose).

Right-click the exe and choose **Run as administrator** to include drive SMART data and TPM details.

### The command line

Any argument runs the command-line mode instead of the window:

```powershell
openpcmanifest --summary --markdown            # short summary for a forum post
openpcmanifest --html --redact --out pc.html   # full report, private details hidden
openpcmanifest --json --section cpu,ram,storage
openpcmanifest --save-raw before.json          # keep a snapshot...
openpcmanifest --compare before.json           # ...and later see what changed
openpcmanifest --list-sections
openpcmanifest --help
```

| Option | Meaning |
| --- | --- |
| `--text` `--markdown` `--bbcode` `--json` `--html` | Output format (default: text) |
| `--summary` | Only the short summary |
| `--section <ids>` | Only these sections; see `--list-sections` |
| `--redact` | Hide serial numbers, addresses, names and keys |
| `--out <file>` | Write to a file |
| `--save-raw <file>` / `--from-raw <file>` | Save a raw snapshot / build a report from one |
| `--compare <old> [new]` | Changes between two snapshots, or between one and this PC now |

Exit codes: `0` success, `1` invalid arguments, `2` collection failed, `3` could not write output.
The JSON format is versioned (`schemaVersion`) and documented in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#json-report-format).

## Privacy

- **No network access.** The program contains no networking code. The only thing that can reach
  the internet is your browser, when you click a Help link.
- **Read-only.** Collection only reads system information. It changes no settings and installs
  nothing.
- **You decide what to share.** Values that identify your PC or you (serial numbers, MAC and IP
  addresses, product IDs, Wi-Fi network names, computer, user and printer port names, volume
  serials) are marked private and replaced with `[redacted]` whenever redaction is on.
- **Raw snapshots are not redacted.** They hold everything that was collected, so keep them to
  yourself and share redacted reports instead.

## How it works

OpenPCManifest is written in Rust and built as a pipeline:

1. **Collect.** One embedded, read-only PowerShell script ([`src/collect/collect.ps1`](src/collect/collect.ps1))
   queries Windows' own inventory (CIM/WMI). Its queries are split into seven groups that run in
   parallel, and each group's results appear as soon as they arrive. CPU details come straight from
   the processor's `CPUID` instruction.
2. **Parse.** Pure Rust decoders read the binary parts: SMBIOS firmware tables (memory slots,
   expansion slots, BIOS), SMART pages (drive health) and CPUID.
3. **Build.** The snapshot becomes a report tree of sections and labelled values.
4. **Render.** The same tree drives the window and every export format.

Everything after collection runs on any OS and is covered by unit tests and golden-file tests
against fixture machines. Read more in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Help us test: the testing grid

OpenPCManifest has to work on hardware we don't own. The [testing grid](docs/TESTED_SYSTEMS.md)
records which machines it has been tried on and what worked.

**If you run it on a PC that isn't listed, please add a row:** edit
[docs/TESTED_SYSTEMS.md](docs/TESTED_SYSTEMS.md) on GitHub and open a pull request. It takes two
minutes and doesn't need any coding. Problems you find go in an
[issue](https://github.com/judeallred/openpcmanifest/issues/new/choose).

## Roadmap

- **v0.1** — Speccy-parity inventory, upgrade advisor, drive health, device problems, driver age,
  Windows 11 readiness, search, redaction, exports, snapshots and compare, CLI. *(this release)*
- **v0.2** — USB port speeds and topology, more EDID detail (HDR, max refresh), NVMe health without
  administrator rights where possible.
- **v1.0** — code signing, branding under Brooklyn Spaceport, and distribution on winget,
  Chocolatey and crates.io.
- **Later** — native collectors replacing PowerShell queries, Linux support, optional
  LibreHardwareMonitor integration for live sensors.

Ideas are welcome in [issues](https://github.com/judeallred/openpcmanifest/issues/new/choose).

## Contributing

Contributions of every size are welcome, from a testing-grid row to a new collector. Start with
[CONTRIBUTING.md](CONTRIBUTING.md); it covers building, the (strict) lint rules, golden-file tests
and how to add a field. Please follow the [Code of Conduct](CODE_OF_CONDUCT.md), and report
security issues privately as described in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE) © Jude Allred and OpenPCManifest contributors.

OpenPCManifest is an independent project and is not affiliated with Piriform, Gen Digital or the
Speccy product.
