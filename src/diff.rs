//! Snapshot diff: what changed between two reports.
//!
//! Reports are flattened to `path = value` pairs (volatile values such as
//! uptime and free space are skipped) and compared. The result is itself a
//! [`Report`], so every renderer and the GUI can show it.

use crate::model::{Node, Report, Section, Status};

/// One difference between two reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Present only in the newer report.
    Added {
        /// Path of the value.
        path: String,
        /// The new value.
        value: String,
    },
    /// Present only in the older report.
    Removed {
        /// Path of the value.
        path: String,
        /// The old value.
        value: String,
    },
    /// Present in both with different values.
    Changed {
        /// Path of the value.
        path: String,
        /// The old value.
        old: String,
        /// The new value.
        new: String,
    },
}

/// Compares two reports. `include_private` controls whether private values
/// (serial numbers and the like) take part.
#[must_use]
pub fn compare(old: &Report, new: &Report, include_private: bool) -> Vec<Change> {
    let a = old.flatten(include_private);
    let b = new.flatten(include_private);
    let mut changes = Vec::new();
    for (path, value) in &a {
        match b.iter().find(|(p, _)| p == path) {
            None => changes.push(Change::Removed {
                path: path.clone(),
                value: value.clone(),
            }),
            Some((_, v)) if v != value => changes.push(Change::Changed {
                path: path.clone(),
                old: value.clone(),
                new: v.clone(),
            }),
            Some(_) => {}
        }
    }
    for (path, value) in &b {
        if !a.iter().any(|(p, _)| p == path) {
            changes.push(Change::Added {
                path: path.clone(),
                value: value.clone(),
            });
        }
    }
    changes
}

/// Presents a list of changes as a report with one section.
#[must_use]
pub fn to_report(changes: &[Change], old: &Report, new: &Report) -> Report {
    let mut s = Section::new("changes", "Changes");
    let when = |r: &Report| {
        r.collected_at
            .clone()
            .unwrap_or_else(|| "unknown time".to_owned())
    };
    s.push(
        Node::item("Compared", format!("{} \u{2192} {}", when(old), when(new)))
            .status(Status::Note),
    );
    if changes.is_empty() {
        s.push(Node::item("Result", "No hardware or configuration changes").status(Status::Good));
    }
    for c in changes {
        let node = match c {
            Change::Added { path, value } => {
                Node::item(format!("Added: {path}"), value.clone()).status(Status::Good)
            }
            Change::Removed { path, value } => {
                Node::item(format!("Removed: {path}"), value.clone()).status(Status::Caution)
            }
            Change::Changed { path, old, new } => {
                Node::item(format!("Changed: {path}"), format!("{old} \u{2192} {new}"))
                    .status(Status::Caution)
            }
        };
        s.push(node);
    }
    Report {
        generator: new.generator.clone(),
        collected_at: new.collected_at.clone(),
        sections: vec![s],
        notes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(bios: &str, drives: &[&str], uptime: &str) -> Report {
        let mut board = Section::new("motherboard", "Motherboard");
        board.push(Node::item("BIOS version", bios));
        board.push(Node::item("Uptime", uptime).volatile());
        let mut storage = Section::new("storage", "Storage");
        for d in drives {
            storage.push(Node::item("Drive", *d));
        }
        Report {
            sections: vec![board, storage],
            ..Report::default()
        }
    }

    #[test]
    fn finds_added_removed_changed_and_ignores_volatile() {
        let old = report("F1", &["WD 4TB"], "1 h");
        let new = report("F3", &["WD 4TB", "Samsung 2TB"], "9 h");
        let changes = compare(&old, &new, false);
        assert_eq!(
            changes,
            vec![
                Change::Changed {
                    path: "motherboard/bios-version".into(),
                    old: "F1".into(),
                    new: "F3".into()
                },
                Change::Added {
                    path: "storage/drive#2".into(),
                    value: "Samsung 2TB".into()
                },
            ]
        );
        let r = to_report(&changes, &old, &new);
        assert_eq!(r.sections[0].nodes.len(), 3);
    }

    #[test]
    fn identical_reports_have_no_changes() {
        let r = report("F1", &["A"], "1 h");
        assert!(compare(&r, &r, true).is_empty());
        let shown = to_report(&[], &r, &r);
        assert_eq!(shown.sections[0].nodes[1].status, Status::Good);
    }
}
