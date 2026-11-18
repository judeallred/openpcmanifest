//! Raw data collection.
//!
//! On Windows, collection runs the embedded PowerShell script
//! (`collect.ps1`) with the inbox Windows PowerShell 5.1, once per query
//! group and in parallel, and merges the JSON documents as they arrive.
//! CPUID is read directly in Rust. The result is a
//! [`Snapshot`], which can be saved and loaded so reports can be rebuilt,
//! shared for bug reports and compared later.
//!
//! The script is read-only and makes no network requests.

use std::fmt;
use std::path::Path;

use crate::json::{self, Value};

/// The embedded collection script.
pub const SCRIPT: &str = include_str!("collect.ps1");

/// A raw snapshot: the collector's JSON document.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The parsed document.
    pub data: Value,
}

/// Why collection or snapshot loading failed.
#[derive(Debug)]
pub enum CollectError {
    /// Reading or writing a file failed.
    Io(std::io::Error),
    /// PowerShell could not be started or exited with an error.
    PowerShell(String),
    /// The collector's output (or a snapshot file) was not valid JSON.
    Parse(json::ParseError),
    /// The file is JSON but not a snapshot.
    NotASnapshot,
}

impl fmt::Display for CollectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "file error: {e}"),
            Self::PowerShell(msg) => write!(f, "PowerShell collection failed: {msg}"),
            Self::Parse(e) => write!(f, "could not read collector output: {e}"),
            Self::NotASnapshot => {
                f.write_str("this JSON file is not an OpenPCManifest raw snapshot")
            }
        }
    }
}

impl std::error::Error for CollectError {}

impl From<std::io::Error> for CollectError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<json::ParseError> for CollectError {
    fn from(e: json::ParseError) -> Self {
        Self::Parse(e)
    }
}

impl Snapshot {
    /// Parses a snapshot from JSON text.
    ///
    /// # Errors
    ///
    /// Fails if the text is not JSON or lacks the snapshot `schema` field.
    pub fn from_json(text: &str) -> Result<Self, CollectError> {
        let data = json::parse(text)?;
        if data.get("schema").as_u64().is_none() {
            return Err(CollectError::NotASnapshot);
        }
        Ok(Self { data })
    }

    /// Loads a snapshot file.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be read or is not a snapshot.
    pub fn load(path: &Path) -> Result<Self, CollectError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }

    /// Saves the snapshot as indented JSON.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), CollectError> {
        std::fs::write(path, self.data.to_json_pretty())?;
        Ok(())
    }

    /// When the snapshot was collected, as recorded by the collector.
    #[must_use]
    pub fn collected_at(&self) -> Option<&str> {
        self.data.get("collectedAt").as_str()
    }
}

/// Query groups in `collect.ps1`, in the order their results are most
/// useful. Each group runs in its own PowerShell process, in parallel.
pub const GROUPS: [&str; 7] = [
    "core", "graphics", "storage", "network", "devices", "security", "drivers",
];

/// Keys that every group's output repeats; the first value wins.
const HEADER_KEYS: [&str; 4] = ["schema", "collectedAt", "isAdmin", "psVersion"];

/// Merges one group's output into a snapshot document. Lists of errors and
/// of completed groups are concatenated; header fields keep their first
/// value; everything else is copied over.
pub fn merge(into: &mut Value, part: Value) {
    let Value::Object(target) = into else { return };
    let Value::Object(pairs) = part else { return };
    for (key, value) in pairs {
        let existing = target
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key));
        match existing {
            Some((_, Value::Array(list))) if key == "errors" || key == "groups" => {
                list.extend(value.items().into_iter().cloned());
            }
            Some((_, Value::Object(map))) if key == "timings" => {
                if let Value::Object(more) = value {
                    map.extend(more);
                }
            }
            Some(_) if HEADER_KEYS.contains(&key.as_str()) => {}
            Some((_, slot)) => *slot = value,
            None => target.push((key, value)),
        }
    }
}

fn base_snapshot() -> Snapshot {
    Snapshot {
        data: Value::Object(vec![
            ("schema".to_owned(), Value::Number(1.0)),
            (
                "collectedAt".to_owned(),
                Value::String(crate::util::now_iso()),
            ),
            ("generator".to_owned(), Value::String(crate::build_string())),
            (
                "cpuid".to_owned(),
                crate::cpu::to_json(&crate::cpu::read_local()),
            ),
            ("groups".to_owned(), Value::Array(Vec::new())),
            ("errors".to_owned(), Value::Array(Vec::new())),
            ("timings".to_owned(), Value::Object(Vec::new())),
        ]),
    }
}

fn error_entry(source: &str, message: &str) -> Value {
    Value::Object(vec![
        ("source".to_owned(), Value::String(source.to_owned())),
        ("message".to_owned(), Value::String(message.to_owned())),
    ])
}

/// Collects a snapshot of the current machine.
///
/// # Errors
///
/// Fails if PowerShell cannot be run at all. Failures of individual query
/// groups are recorded in the snapshot's `errors` list instead.
pub fn collect() -> Result<Snapshot, CollectError> {
    collect_with_progress(|_, _| {})
}

/// Collects a snapshot, calling `on_progress` with the merged snapshot so far
/// and the groups finished so far: once before any group (CPU details only),
/// then after each group completes.
///
/// # Errors
///
/// Fails if every group fails, for example because PowerShell is missing.
pub fn collect_with_progress(
    mut on_progress: impl FnMut(&Snapshot, &[&'static str]),
) -> Result<Snapshot, CollectError> {
    let mut snap = base_snapshot();
    let mut done: Vec<&'static str> = Vec::new();
    let mut first_error = None;
    on_progress(&snap, &done);
    run_groups(&mut |group, result, millis| {
        merge(
            &mut snap.data,
            Value::Object(vec![(
                "timings".to_owned(),
                Value::Object(vec![(group.to_owned(), Value::Number(f64::from(millis)))]),
            )]),
        );
        match result {
            Ok(part) => merge(&mut snap.data, part),
            Err(e) => {
                merge(
                    &mut snap.data,
                    Value::Object(vec![(
                        "errors".to_owned(),
                        Value::Array(vec![error_entry(group, &e.to_string())]),
                    )]),
                );
                first_error.get_or_insert(e);
            }
        }
        done.push(group);
        on_progress(&snap, &done);
    })?;
    let any_succeeded = snap
        .data
        .get("groups")
        .items()
        .iter()
        .any(|g| g.as_str().is_some());
    match first_error {
        Some(e) if !any_succeeded && cfg!(windows) => Err(e),
        _ => Ok(snap),
    }
}

#[cfg(windows)]
fn powershell_path() -> std::path::PathBuf {
    use std::path::PathBuf;
    let root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let tail = ["WindowsPowerShell", "v1.0", "powershell.exe"];
    // A 32-bit build on 64-bit Windows sees the 32-bit System32; Sysnative
    // reaches the real 64-bit PowerShell so registry and storage queries
    // aren't redirected.
    let sysnative = tail
        .iter()
        .fold(root.join("Sysnative"), |p, part| p.join(part));
    if sysnative.exists() {
        return sysnative;
    }
    // Always use the full path so a `powershell.exe` earlier on PATH is never run.
    tail.iter()
        .fold(root.join("System32"), |p, part| p.join(part))
}

#[cfg(windows)]
fn run_group(script: &Path, dir: &Path, group: &str) -> Result<Value, CollectError> {
    use std::os::windows::process::CommandExt as _;
    use std::process::Command;

    /// Keeps PowerShell from flashing a console window when the GUI runs it.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let out = dir.join(format!("{group}.json"));
    let output = Command::new(powershell_path())
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-OutFile")
        .arg(&out)
        .arg("-Groups")
        .arg(group)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CollectError::PowerShell(format!("could not start: {e}")))?;
    let text = std::fs::read_to_string(&out).map_err(|_| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        CollectError::PowerShell(format!(
            "exit status {}; {}",
            output.status,
            stderr.lines().take(5).collect::<Vec<_>>().join(" ")
        ))
    })?;
    Ok(json::parse(&text)?)
}

/// Runs every group in parallel and hands each result, with how long it
/// took in milliseconds, to `on_result` as it finishes.
#[cfg(windows)]
fn run_groups(
    on_result: &mut dyn FnMut(&'static str, Result<Value, CollectError>, u32),
) -> Result<(), CollectError> {
    let dir = std::env::temp_dir().join(format!("openpcmanifest-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let script = dir.join("collect.ps1");
    // A UTF-8 byte-order mark makes Windows PowerShell 5.1 read the script as
    // UTF-8 rather than the system code page.
    std::fs::write(&script, format!("\u{feff}{SCRIPT}"))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for group in GROUPS {
            let tx = tx.clone();
            let (script, dir) = (&script, &dir);
            scope.spawn(move || {
                let started = std::time::Instant::now();
                let result = run_group(script, dir, group);
                let millis = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
                // The receiver outlives every sender inside this scope.
                let _ = tx.send((group, result, millis));
            });
        }
        drop(tx);
        for (group, result, millis) in rx {
            on_result(group, result, millis);
        }
    });
    // Best effort: a leftover temp folder is harmless.
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[cfg(not(windows))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "same signature as the Windows implementation"
)]
fn run_groups(
    on_result: &mut dyn FnMut(&'static str, Result<Value, CollectError>, u32),
) -> Result<(), CollectError> {
    let message = "Full collection needs Windows. Only processor details were read; \
                   use --from-raw to view a snapshot taken on Windows.";
    let part = Value::Object(vec![(
        "errors".to_owned(),
        Value::Array(vec![error_entry("platform", message)]),
    )]);
    on_result("platform", Ok(part), 0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_embedded_and_read_only() {
        assert!(SCRIPT.contains("[Parameter(Mandatory = $true)][string]$OutFile"));
        for verb in [
            "Set-",
            "Remove-",
            "New-Item",
            "Invoke-WebRequest",
            "Invoke-RestMethod",
            "Start-Process",
        ] {
            assert!(
                !SCRIPT.contains(verb),
                "collector must stay read-only and offline: found {verb}"
            );
        }
    }

    #[test]
    fn rejects_non_snapshots() {
        assert!(matches!(
            Snapshot::from_json("{\"a\":1}"),
            Err(CollectError::NotASnapshot)
        ));
        assert!(matches!(
            Snapshot::from_json("nope"),
            Err(CollectError::Parse(_))
        ));
    }

    #[test]
    fn merge_combines_groups() {
        let mut snap = base_snapshot();
        let a =
            json::parse(r#"{"schema":1,"collectedAt":"A","groups":["core"],"os":[1],"errors":[]}"#)
                .unwrap();
        let b = json::parse(
            r#"{"schema":1,"collectedAt":"B","groups":["storage"],"diskDrives":[2],"errors":[{"source":"smartData","message":"x"}]}"#,
        )
        .unwrap();
        merge(&mut snap.data, a);
        merge(&mut snap.data, b);
        assert_eq!(snap.data.get("groups").items().len(), 2);
        assert_eq!(snap.data.get("errors").items().len(), 1);
        assert_eq!(snap.data.get("os").items().len(), 1);
        assert_eq!(snap.data.get("diskDrives").items().len(), 1);
        assert_ne!(
            snap.data.get("collectedAt").as_str(),
            Some("B"),
            "header keeps its first value"
        );
    }

    #[test]
    fn progress_is_reported() {
        let mut calls = 0;
        let snap = collect_with_progress(|_, _| calls += 1).unwrap();
        assert!(calls >= 2, "once up front and once per group");
        assert!(snap.data.get("schema").as_u64().is_some());
    }

    #[test]
    fn script_lists_every_group() {
        for g in GROUPS {
            assert!(
                SCRIPT.contains(&format!("'{g}'")),
                "group {g} missing from collect.ps1"
            );
        }
    }

    #[test]
    fn collect_always_includes_cpuid_and_generator() {
        let snap = collect().unwrap();
        assert!(snap.data.get("schema").as_u64().is_some());
        assert!(snap.data.get("generator").as_str().is_some());
        if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
            assert!(!snap.data.get("cpuid").items().is_empty());
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let snap =
            Snapshot::from_json(r#"{"schema":1,"collectedAt":"2026-11-15T00:00:00Z"}"#).unwrap();
        let path = std::env::temp_dir().join(format!("opm-test-{}.json", std::process::id()));
        snap.save(&path).unwrap();
        let back = Snapshot::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back, snap);
        assert_eq!(back.collected_at(), Some("2026-11-15T00:00:00Z"));
    }
}
