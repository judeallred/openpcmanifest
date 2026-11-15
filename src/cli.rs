//! Command-line interface.
//!
//! Running with no arguments opens the GUI. Any other argument runs the
//! command-line mode described by [`HELP`].

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;

use crate::build::{SECTIONS, build};
use crate::collect::{Snapshot, collect};
use crate::diff;
use crate::model::Report;
use crate::render::{Format, render, rig_summary};

/// Exit code: success.
pub const EXIT_OK: u8 = 0;
/// Exit code: invalid arguments.
pub const EXIT_USAGE: u8 = 1;
/// Exit code: collecting data or reading a snapshot failed.
pub const EXIT_COLLECT: u8 = 2;
/// Exit code: writing output failed.
pub const EXIT_IO: u8 = 3;

/// Command-line help text.
pub const HELP: &str = "\
OpenPCManifest - an open-source hardware inventory for Windows.

USAGE:
    openpcmanifest                 Open the graphical interface
    openpcmanifest [OPTIONS]       Print a report

OUTPUT FORMAT (default: --text):
    --text            Indented plain text
    --markdown        Markdown
    --bbcode          Forum BBCode
    --json            JSON (schema documented in docs/ARCHITECTURE.md)
    --html            Self-contained HTML page

OPTIONS:
    --summary             Only the short \"my PC\" summary
    --section <ids>       Comma-separated sections, e.g. --section cpu,ram
    --list-sections       List section IDs
    --redact              Hide serial numbers, MAC/IP addresses, names and keys
    --out <file>          Write to a file instead of the console
    --save-raw <file>     Also save the raw snapshot (attach it to bug reports
                          with --redact output, or keep it for --compare)
    --from-raw <file>     Build the report from a saved raw snapshot
    --compare <old> [new] Show what changed between two raw snapshots; with one
                          file, compare it against this machine now
    --gui                 Open the graphical interface
    -h, --help            Show this help
    -V, --version         Show the version

EXIT CODES:
    0 success, 1 invalid arguments, 2 collection failed, 3 could not write output

Run as administrator to include SMART attributes, drive reliability counters
and TPM details.
";

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Open the GUI.
    Gui,
    /// Print help.
    Help,
    /// Print the version.
    Version,
    /// List section IDs.
    ListSections,
    /// Produce a report.
    Report(Options),
}

/// Options for a command-line report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Output format.
    pub format: Format,
    /// Only the short summary.
    pub summary: bool,
    /// Sections to include; empty means all.
    pub sections: Vec<String>,
    /// Hide private values.
    pub redact: bool,
    /// Output file.
    pub out: Option<PathBuf>,
    /// Where to save the raw snapshot.
    pub save_raw: Option<PathBuf>,
    /// Read the snapshot from here instead of collecting.
    pub from_raw: Option<PathBuf>,
    /// Compare against this older snapshot.
    pub compare_old: Option<PathBuf>,
    /// The newer snapshot to compare; `None` means this machine now.
    pub compare_new: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: Format::Text,
            summary: false,
            sections: Vec::new(),
            redact: false,
            out: None,
            save_raw: None,
            from_raw: None,
            compare_old: None,
            compare_new: None,
        }
    }
}

/// Parses command-line arguments (without the program name).
///
/// # Errors
///
/// Returns a message describing the first invalid argument.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Gui);
    }
    let mut o = Options::default();
    let mut it = args.iter().peekable();
    let value = |it: &mut std::iter::Peekable<std::slice::Iter<'_, String>>, flag: &str| {
        it.next()
            .cloned()
            .ok_or_else(|| format!("{flag} needs a value"))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" | "/?" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--list-sections" => return Ok(Command::ListSections),
            "--gui" => return Ok(Command::Gui),
            "--text" => o.format = Format::Text,
            "--markdown" | "--md" => o.format = Format::Markdown,
            "--bbcode" => o.format = Format::BbCode,
            "--json" => o.format = Format::Json,
            "--html" => o.format = Format::Html,
            "--summary" => o.summary = true,
            "--redact" => o.redact = true,
            "--section" | "--sections" => {
                let v = value(&mut it, arg)?;
                for id in v.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    if !SECTIONS.iter().any(|(s, _)| *s == id) {
                        return Err(format!("unknown section '{id}'; see --list-sections"));
                    }
                    o.sections.push(id.to_owned());
                }
            }
            "--out" | "-o" => o.out = Some(PathBuf::from(value(&mut it, arg)?)),
            "--save-raw" => o.save_raw = Some(PathBuf::from(value(&mut it, arg)?)),
            "--from-raw" => o.from_raw = Some(PathBuf::from(value(&mut it, arg)?)),
            "--compare" => {
                o.compare_old = Some(PathBuf::from(value(&mut it, arg)?));
                if let Some(next) = it.peek()
                    && !next.starts_with('-')
                {
                    o.compare_new = it.next().map(PathBuf::from);
                }
            }
            other => return Err(format!("unknown argument '{other}'; see --help")),
        }
    }
    if o.compare_old.is_some() && o.from_raw.is_some() {
        return Err(
            "use --compare <old> <new> instead of combining --compare with --from-raw".to_owned(),
        );
    }
    Ok(Command::Report(o))
}

/// Runs a parsed command and returns the process exit code.
#[must_use]
pub fn run(cmd: &Command) -> u8 {
    match cmd {
        Command::Gui => EXIT_USAGE,
        Command::Help => emit(HELP, None),
        Command::Version => emit(&format!("{}\n", crate::build_string()), None),
        Command::ListSections => {
            let list = SECTIONS.iter().fold(String::new(), |mut acc, (id, title)| {
                let _ = writeln!(acc, "{id:<12} {title}");
                acc
            });
            emit(&list, None)
        }
        Command::Report(o) => match produce(o) {
            Ok(text) => emit(&text, o.out.as_ref()),
            Err((code, message)) => {
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!("openpcmanifest: {message}\n"),
                );
                code
            }
        },
    }
}

fn load_or_collect(path: Option<&PathBuf>) -> Result<Snapshot, (u8, String)> {
    path.map_or_else(
        || collect().map_err(|e| (EXIT_COLLECT, e.to_string())),
        |p| Snapshot::load(p).map_err(|e| (EXIT_COLLECT, format!("{}: {e}", p.display()))),
    )
}

/// Produces the report text for `o`.
///
/// # Errors
///
/// Returns an exit code and message if collection, loading or saving fails.
pub fn produce(o: &Options) -> Result<String, (u8, String)> {
    let report: Report = if let Some(old_path) = &o.compare_old {
        let old = load_or_collect(Some(old_path))?;
        let new = load_or_collect(o.compare_new.as_ref())?;
        save_raw(o, &new)?;
        let (old, new) = (build(&old), build(&new));
        diff::to_report(&diff::compare(&old, &new, !o.redact), &old, &new)
    } else {
        let snap = load_or_collect(o.from_raw.as_ref())?;
        save_raw(o, &snap)?;
        build(&snap)
    };
    let report = if o.sections.is_empty() {
        report
    } else {
        let ids: Vec<&str> = o.sections.iter().map(String::as_str).collect();
        report.only_sections(&ids)
    };
    let report = if o.redact { report.redacted() } else { report };
    Ok(if o.summary {
        rig_summary(&report, o.format)
    } else {
        render(&report, o.format)
    })
}

fn save_raw(o: &Options, snap: &Snapshot) -> Result<(), (u8, String)> {
    if let Some(p) = &o.save_raw {
        snap.save(p)
            .map_err(|e| (EXIT_IO, format!("{}: {e}", p.display())))?;
    }
    Ok(())
}

fn emit(text: &str, out: Option<&PathBuf>) -> u8 {
    let result = out.map_or_else(
        || {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(text.as_bytes())
                .and_then(|()| stdout.flush())
        },
        |path| std::fs::write(path, text),
    );
    match result {
        Ok(()) => EXIT_OK,
        Err(e) => {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("openpcmanifest: could not write output: {e}\n"),
            );
            EXIT_IO
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(ToOwned::to_owned).collect()
    }

    #[test]
    fn no_args_opens_gui() {
        assert_eq!(parse(&[]).unwrap(), Command::Gui);
    }

    #[test]
    fn parses_options() {
        let Command::Report(o) =
            parse(&args("--json --redact --section cpu,ram --out x.json")).unwrap()
        else {
            panic!("expected a report");
        };
        assert_eq!(o.format, Format::Json);
        assert!(o.redact);
        assert_eq!(o.sections, vec!["cpu", "ram"]);
        assert_eq!(o.out, Some(PathBuf::from("x.json")));
    }

    #[test]
    fn compare_takes_one_or_two_files() {
        let Command::Report(o) = parse(&args("--compare a.json b.json --markdown")).unwrap() else {
            panic!()
        };
        assert_eq!(o.compare_new, Some(PathBuf::from("b.json")));
        assert_eq!(o.format, Format::Markdown);
        let Command::Report(o) = parse(&args("--compare a.json --json")).unwrap() else {
            panic!()
        };
        assert_eq!(o.compare_new, None);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&args("--section nope")).is_err());
        assert!(parse(&args("--out")).is_err());
        assert!(parse(&args("--frobnicate")).is_err());
        assert!(parse(&args("--compare a --from-raw b")).is_err());
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse(&args("--version")).unwrap(), Command::Version);
        assert_eq!(parse(&args("--json -h")).unwrap(), Command::Help);
    }

    fn fixture() -> PathBuf {
        PathBuf::from(format!(
            "{}/tests/fixtures/desktop.json",
            env!("CARGO_MANIFEST_DIR")
        ))
    }

    #[test]
    fn produces_reports_from_raw_snapshots() {
        let o = Options {
            from_raw: Some(fixture()),
            summary: true,
            format: Format::BbCode,
            ..Options::default()
        };
        let text = produce(&o).unwrap();
        assert!(text.starts_with("[b]My PC[/b]"));
        let o = Options {
            from_raw: Some(fixture()),
            sections: vec!["storage".into()],
            redact: true,
            ..Options::default()
        };
        let text = produce(&o).unwrap();
        assert!(text.contains("Storage\n=======") && !text.contains("WD-WCC7K0000001"));
    }

    #[test]
    fn compares_snapshots() {
        let o = Options {
            compare_old: Some(fixture()),
            compare_new: Some(fixture()),
            ..Options::default()
        };
        assert!(
            produce(&o)
                .unwrap()
                .contains("No hardware or configuration changes")
        );
    }

    #[test]
    fn missing_snapshot_is_a_collect_error() {
        let o = Options {
            from_raw: Some(PathBuf::from("/definitely/missing.json")),
            ..Options::default()
        };
        assert_eq!(produce(&o).unwrap_err().0, EXIT_COLLECT);
    }
}
