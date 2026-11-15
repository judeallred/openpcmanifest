//! Raw data collection.
//!
//! On Windows, collection runs one embedded PowerShell script
//! (`collect.ps1`) with the inbox Windows PowerShell 5.1 and reads back a
//! single JSON document. CPUID is read directly in Rust. The result is a
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

    fn set(&mut self, key: &str, value: Value) {
        if let Value::Object(pairs) = &mut self.data {
            pairs.retain(|(k, _)| !k.eq_ignore_ascii_case(key));
            pairs.push((key.to_owned(), value));
        }
    }
}

/// Collects a snapshot of the current machine.
///
/// On platforms other than Windows the snapshot holds only CPUID data and an
/// error entry explaining that full collection needs Windows.
///
/// # Errors
///
/// Fails if PowerShell cannot be run or its output cannot be read.
pub fn collect() -> Result<Snapshot, CollectError> {
    let mut snap = collect_platform()?;
    snap.set("cpuid", crate::cpu::to_json(&crate::cpu::read_local()));
    snap.set("generator", Value::String(crate::build_string()));
    Ok(snap)
}

#[cfg(windows)]
fn collect_platform() -> Result<Snapshot, CollectError> {
    use std::os::windows::process::CommandExt as _;
    use std::process::Command;

    /// Keeps PowerShell from flashing a console window when the GUI runs it.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let dir = std::env::temp_dir().join(format!("openpcmanifest-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let script = dir.join("collect.ps1");
    let out = dir.join("snapshot.json");
    // A UTF-8 byte-order mark makes Windows PowerShell 5.1 read the script as
    // UTF-8 rather than the system code page.
    std::fs::write(&script, format!("\u{feff}{SCRIPT}"))?;

    // Use the full path so a `powershell.exe` earlier on PATH is never run.
    let powershell = std::env::var_os("SystemRoot").map_or_else(
        || std::path::PathBuf::from("powershell.exe"),
        |root| {
            std::path::PathBuf::from(root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        },
    );
    let result = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .arg("-OutFile")
        .arg(&out)
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let text = std::fs::read_to_string(&out);
    // Best effort: a leftover temp folder is harmless.
    let _ = std::fs::remove_dir_all(&dir);

    let output = result.map_err(|e| CollectError::PowerShell(format!("could not start: {e}")))?;
    let text = text.map_err(|_| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        CollectError::PowerShell(format!(
            "exit status {}; {}",
            output.status,
            stderr.lines().take(5).collect::<Vec<_>>().join(" ")
        ))
    })?;
    Snapshot::from_json(&text)
}

#[cfg(not(windows))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "same signature as the Windows implementation"
)]
fn collect_platform() -> Result<Snapshot, CollectError> {
    let error = Value::Object(vec![
        ("source".to_owned(), Value::String("platform".to_owned())),
        (
            "message".to_owned(),
            Value::String(
                "Full collection needs Windows. Only processor details were read; \
                 use --from-raw to view a snapshot taken on Windows."
                    .to_owned(),
            ),
        ),
    ]);
    Ok(Snapshot {
        data: Value::Object(vec![
            ("schema".to_owned(), Value::Number(1.0)),
            (
                "collectedAt".to_owned(),
                Value::String(crate::util::now_iso()),
            ),
            ("errors".to_owned(), Value::Array(vec![error])),
        ]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_embedded_and_read_only() {
        assert!(SCRIPT.contains("param([Parameter(Mandatory = $true)][string]$OutFile)"));
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
    fn collect_always_includes_cpuid_and_generator() {
        let snap = collect().unwrap();
        assert!(snap.data.get("schema").as_u64().is_some());
        assert!(snap.data.get("generator").as_str().is_some());
        if cfg!(target_arch = "x86_64") {
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
