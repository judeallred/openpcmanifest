# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - unreleased

### Added

- Hardware inventory with Speccy's sections: Summary, Operating System, CPU, RAM, Motherboard,
  Graphics, Storage, Optical Drives, Audio, Peripherals and Network, plus USB.
- Upgrade Advisor: free RAM slots and maximum supported memory, memory running below its rated
  speed, free expansion slots with lane widths, and drive warnings.
- Drive health verdicts (Good / Caution / Bad) with plain-language reasons, from SMART data,
  Windows health status and reliability counters.
- Device Problems, Drivers & Firmware Age and Windows 11 Readiness sections.
- Monitor details from EDID: model, maker, manufacture date and screen size.
- Search across all sections.
- Redaction of private values on screen, in exports and in copies.
- Exports to text, Markdown, BBCode, JSON and self-contained HTML; one-click summary copy.
- Raw snapshots (`--save-raw`, `--from-raw`) and snapshot comparison (`--compare`).
- Command-line interface with stable exit codes.
- GitHub Actions: CI on Windows and Linux, rolling nightly builds, tagged releases, and
  package-manager publishing (switched off until v1.0).
