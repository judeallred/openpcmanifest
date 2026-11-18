//! Builds a [`Report`] from a raw [`Snapshot`].
//!
//! Each section has its own builder. Builders read the snapshot through the
//! forgiving accessors in [`crate::json`], so a missing class or field simply
//! leaves its line out of the report.

mod devices;
mod insights;
mod system;

use crate::collect::Snapshot;
use crate::cpu::{self, CpuInfo};
use crate::json::Value;
use crate::model::{Node, Report, Section};
use crate::smbios::{self, Tables};
use crate::util::{self, Date};

/// Section IDs and titles, in display order.
pub const SECTIONS: [(&str, &str); 17] = [
    ("summary", "Summary"),
    ("os", "Operating System"),
    ("cpu", "CPU"),
    ("ram", "RAM"),
    ("motherboard", "Motherboard"),
    ("graphics", "Graphics"),
    ("storage", "Storage"),
    ("optical", "Optical Drives"),
    ("audio", "Audio"),
    ("peripherals", "Peripherals"),
    ("usb", "USB"),
    ("network", "Network"),
    ("upgrade", "Upgrade Advisor"),
    ("problems", "Device Problems"),
    ("drivers", "Drivers & Firmware Age"),
    ("windows11", "Windows 11 Readiness"),
    ("collection", "About This Report"),
];

/// Shared, pre-decoded inputs for the section builders.
pub struct Ctx<'a> {
    /// The raw snapshot document.
    pub raw: &'a Value,
    /// Decoded SMBIOS tables (empty if unavailable).
    pub smbios: Tables,
    /// Decoded CPUID information.
    pub cpu: Option<CpuInfo>,
    /// The collection date, used to compute ages.
    pub today: Option<Date>,
}

impl<'a> Ctx<'a> {
    /// Prepares the context for a snapshot.
    #[must_use]
    pub fn new(raw: &'a Value) -> Self {
        let smbios = raw
            .path(&["smbios", "data"])
            .as_str()
            .and_then(crate::base64::decode)
            .map(|bytes| Tables::decode(&bytes))
            .unwrap_or_default();
        let cpu = cpu::decode(&cpu::from_json(raw.get("cpuid")));
        let today = raw.get("collectedAt").as_str().and_then(Date::parse);
        Self {
            raw,
            smbios,
            cpu,
            today,
        }
    }

    /// The items of a top-level list.
    #[must_use]
    pub fn list(&self, key: &str) -> Vec<&'a Value> {
        self.raw.get(key).items()
    }

    /// The first item of a top-level list.
    #[must_use]
    pub fn first(&self, key: &str) -> Option<&'a Value> {
        self.list(key).into_iter().next()
    }

    /// True if the collector recorded a failure for `source`.
    #[must_use]
    pub fn failed(&self, source: &str) -> bool {
        self.list("errors")
            .iter()
            .any(|e| e.get("source").as_str() == Some(source))
    }

    /// True if the collector ran with administrator rights.
    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.raw.get("isAdmin").as_bool().unwrap_or(false)
    }
}

/// A trimmed, non-placeholder text field.
#[must_use]
pub fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .as_text()
        .map(|s| util::tidy(&s))
        .filter(|s| !smbios::is_placeholder(s))
}

/// A non-negative integer field.
#[must_use]
pub fn num(v: &Value, key: &str) -> Option<u64> {
    v.get(key).as_u64()
}

/// Joins the parts that are present with `sep`.
#[must_use]
pub fn join(parts: &[Option<String>], sep: &str) -> Option<String> {
    let present: Vec<&str> = parts.iter().filter_map(|p| p.as_deref()).collect();
    if present.is_empty() {
        None
    } else {
        Some(present.join(sep))
    }
}

/// Builds the full report.
#[must_use]
pub fn build(snapshot: &Snapshot) -> Report {
    let ctx = Ctx::new(&snapshot.data);
    let mut sections = vec![
        Section::new("summary", "Summary"),
        system::os(&ctx),
        system::cpu(&ctx),
        system::ram(&ctx),
        system::motherboard(&ctx),
        devices::graphics(&ctx),
        devices::storage(&ctx),
        devices::optical(&ctx),
        devices::audio(&ctx),
        devices::peripherals(&ctx),
        devices::usb(&ctx),
        devices::network(&ctx),
        insights::upgrade(&ctx),
        insights::problems(&ctx),
        insights::drivers(&ctx),
        insights::windows11(&ctx),
        collection(&ctx),
    ];
    let summary = summary(&sections);
    for s in &mut sections {
        s.nodes.retain(|n| !n.key.starts_with("summary-"));
    }
    if let Some(first) = sections.first_mut() {
        *first = summary;
    }
    sections.retain(|s| !s.nodes.is_empty());
    Report {
        generator: snapshot
            .data
            .get("generator")
            .as_str()
            .map_or_else(crate::build_string, ToOwned::to_owned),
        collected_at: snapshot.collected_at().map(ToOwned::to_owned),
        sections,
        notes: notes(&ctx),
    }
}

/// The values of the summary lines that builders tag with `summary-*` keys.
fn summary(sections: &[Section]) -> Section {
    let mut s = Section::new("summary", "Summary");
    for section in sections {
        for n in &section.nodes {
            if let Some(rest) = n.key.strip_prefix("summary-") {
                s.push(Node {
                    key: rest.to_owned(),
                    ..n.clone()
                });
            }
        }
    }
    s
}

fn notes(ctx: &Ctx<'_>) -> Vec<String> {
    let mut notes = Vec::new();
    if ctx.raw.get("isAdmin").as_bool() == Some(false) {
        notes.push(
            "Not running as administrator: SMART attributes, drive reliability counters and TPM \
             details are unavailable. Right-click and choose \u{201c}Run as administrator\u{201d} to include them."
                .to_owned(),
        );
    }
    for e in ctx.list("errors") {
        if e.get("source").as_str() == Some("platform")
            && let Some(m) = e.get("message").as_str()
        {
            notes.push(m.to_owned());
        }
    }
    notes
}

fn collection(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("collection", "About This Report");
    s.push(crate::model::opt(
        "Collected",
        ctx.raw.get("collectedAt").as_str(),
    ));
    s.push(crate::model::opt(
        "Generator",
        ctx.raw.get("generator").as_str(),
    ));
    s.push(crate::model::opt(
        "Windows PowerShell",
        ctx.raw.get("psVersion").as_str(),
    ));
    s.push(
        ctx.raw
            .get("isAdmin")
            .as_bool()
            .map(|a| Node::item("Administrator", if a { "Yes" } else { "No" })),
    );
    s.push(crate::model::opt(
        "SMBIOS version",
        ctx.raw.path(&["smbios", "version"]).as_str(),
    ));
    if let Value::Object(timings) = ctx.raw.get("timings")
        && !timings.is_empty()
    {
        let slowest = timings
            .iter()
            .filter_map(|(_, v)| v.as_u64())
            .max()
            .unwrap_or(0);
        let mut g = Node::item(
            "Collection time",
            format!("{:.1} s", millis_to_secs(slowest)),
        )
        .volatile();
        for (group, ms) in timings {
            if let Some(ms) = ms.as_u64() {
                g.push(
                    Node::item(group.clone(), format!("{:.1} s", millis_to_secs(ms))).volatile(),
                );
            }
        }
        s.push(g);
    }
    let errors = ctx.list("errors");
    if !errors.is_empty() {
        let mut g = Node::item("Queries that failed", errors.len().to_string());
        for e in errors {
            if let Some(src) = e.get("source").as_str() {
                g.push(
                    Node::item(src, e.get("message").as_str().unwrap_or("failed")).with_key(src),
                );
            }
        }
        s.push(g);
    }
    s
}

#[allow(clippy::cast_precision_loss, reason = "display rounding only")]
fn millis_to_secs(ms: u64) -> f64 {
    ms as f64 / 1000.0
}

#[cfg(test)]
mod tests;
