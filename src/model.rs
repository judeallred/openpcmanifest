//! The report tree that every front end and renderer works from.
//!
//! A [`Report`] is a list of [`Section`]s, each a tree of labelled
//! [`Node`]s. Nodes carry stable keys (for JSON output and snapshot diff),
//! a privacy flag (for redaction) and a volatility flag (values such as
//! uptime that change on every run and are ignored by diff).

/// The text that replaces a private value when redaction is on.
pub const REDACTED: &str = "[redacted]";

/// How a value should be presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    /// Plain value.
    #[default]
    Normal,
    /// A good result, such as a healthy drive.
    Good,
    /// Worth attention.
    Caution,
    /// A problem.
    Bad,
    /// Advice or explanation rather than a measurement.
    Note,
}

impl Status {
    /// A short machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Good => "good",
            Self::Caution => "caution",
            Self::Bad => "bad",
            Self::Note => "note",
        }
    }
}

/// One labelled entry, optionally with a value and children.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// Stable machine key, unique among siblings.
    pub key: String,
    /// Display label.
    pub label: String,
    /// Display value.
    pub value: Option<String>,
    /// True for values that identify the machine or its owner.
    pub private: bool,
    /// True for values that change from run to run.
    pub volatile: bool,
    /// Presentation hint.
    pub status: Status,
    /// Child nodes.
    pub children: Vec<Self>,
}

impl Node {
    /// A group with no value of its own.
    #[must_use]
    pub fn group(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            key: slug(&label),
            label,
            ..Self::default()
        }
    }

    /// A labelled value.
    #[must_use]
    pub fn item(label: impl Into<String>, value: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            key: slug(&label),
            label,
            value: Some(value.into()),
            ..Self::default()
        }
    }

    /// Overrides the generated key.
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// Marks the value as private.
    #[must_use]
    pub const fn private(mut self) -> Self {
        self.private = true;
        self
    }

    /// Marks the value as changing between runs.
    #[must_use]
    pub const fn volatile(mut self) -> Self {
        self.volatile = true;
        self
    }

    /// Sets the presentation status.
    #[must_use]
    pub const fn status(mut self, status: Status) -> Self {
        self.status = status;
        self
    }

    /// Adds a child, ignoring `None`.
    pub fn push(&mut self, child: impl Into<Option<Self>>) {
        if let Some(c) = child.into() {
            self.children.push(c);
        }
    }

    /// Adds a child and returns `self`, for chaining.
    #[must_use]
    pub fn with(mut self, child: impl Into<Option<Self>>) -> Self {
        self.push(child);
        self
    }

    /// True if this node has neither a value nor children.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.value.is_none() && self.children.is_empty()
    }

    /// The value as shown, honouring redaction.
    #[must_use]
    pub fn shown_value(&self, redact: bool) -> Option<&str> {
        if redact && self.private {
            self.value.as_ref().map(|_| REDACTED)
        } else {
            self.value.as_deref()
        }
    }
}

/// An optional labelled value: `None` when the value is missing, so callers
/// can push it without checking.
#[must_use]
pub fn opt(label: &str, value: Option<impl Into<String>>) -> Option<Node> {
    value.map(|v| Node::item(label, v))
}

/// One top-level section, such as CPU or Storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Stable section ID, such as `cpu`.
    pub id: &'static str,
    /// Display title.
    pub title: &'static str,
    /// The section's nodes.
    pub nodes: Vec<Node>,
}

impl Section {
    /// An empty section.
    #[must_use]
    pub const fn new(id: &'static str, title: &'static str) -> Self {
        Self {
            id,
            title,
            nodes: Vec::new(),
        }
    }

    /// Adds a node, ignoring `None` and empty nodes.
    pub fn push(&mut self, node: impl Into<Option<Node>>) {
        if let Some(n) = node.into()
            && !n.is_empty()
        {
            self.nodes.push(n);
        }
    }
}

/// A full report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// The tool name and build that produced the report.
    pub generator: String,
    /// When the underlying data was collected (ISO 8601), if known.
    pub collected_at: Option<String>,
    /// Sections in display order.
    pub sections: Vec<Section>,
    /// Notes about the collection, such as missing administrator rights.
    pub notes: Vec<String>,
}

impl Report {
    /// The section with `id`.
    #[must_use]
    pub fn section(&self, id: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// A copy with private values replaced by [`REDACTED`].
    #[must_use]
    pub fn redacted(&self) -> Self {
        fn walk(n: &mut Node) {
            if n.private && n.value.is_some() {
                n.value = Some(REDACTED.to_owned());
            }
            n.children.iter_mut().for_each(walk);
        }
        let mut r = self.clone();
        for s in &mut r.sections {
            s.nodes.iter_mut().for_each(walk);
        }
        r
    }

    /// A copy keeping only the listed section IDs, in the listed order.
    /// Unknown IDs are ignored.
    #[must_use]
    pub fn only_sections(&self, ids: &[&str]) -> Self {
        let mut r = self.clone();
        r.sections = ids
            .iter()
            .filter_map(|id| self.section(id).cloned())
            .collect();
        r
    }

    /// A copy keeping only nodes whose label or value contains `query`
    /// (case-insensitive), along with their ancestors. A matching group keeps
    /// all its children. Sections with no matches are dropped.
    #[must_use]
    pub fn filtered(&self, query: &str, redact: bool) -> Self {
        fn keep(n: &Node, q: &str, redact: bool) -> Option<Node> {
            let hit = n.label.to_lowercase().contains(q)
                || n.shown_value(redact)
                    .is_some_and(|v| v.to_lowercase().contains(q));
            if hit {
                return Some(n.clone());
            }
            let children: Vec<Node> = n
                .children
                .iter()
                .filter_map(|c| keep(c, q, redact))
                .collect();
            if children.is_empty() {
                None
            } else {
                Some(Node {
                    children,
                    ..n.clone()
                })
            }
        }
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.clone();
        }
        let mut r = self.clone();
        r.sections = self
            .sections
            .iter()
            .filter_map(|s| {
                let nodes: Vec<Node> = s.nodes.iter().filter_map(|n| keep(n, &q, redact)).collect();
                if nodes.is_empty() {
                    None
                } else {
                    Some(Section { nodes, ..s.clone() })
                }
            })
            .collect();
        r
    }

    /// Every valued node as `(path, value)`, skipping volatile values and
    /// optionally private ones. Paths are made of keys joined by `/`, with a
    /// `#n` suffix when siblings share a key.
    #[must_use]
    pub fn flatten(&self, include_private: bool) -> Vec<(String, String)> {
        fn walk(
            nodes: &[Node],
            prefix: &str,
            include_private: bool,
            out: &mut Vec<(String, String)>,
        ) {
            let mut seen: Vec<(&str, usize)> = Vec::new();
            for n in nodes {
                let count = if let Some(entry) = seen.iter_mut().find(|(k, _)| *k == n.key) {
                    entry.1 += 1;
                    entry.1
                } else {
                    seen.push((&n.key, 1));
                    1
                };
                let key = if count > 1 {
                    format!("{}#{count}", n.key)
                } else {
                    n.key.clone()
                };
                let path = format!("{prefix}/{key}");
                if let Some(v) = &n.value
                    && !n.volatile
                    && (include_private || !n.private)
                {
                    out.push((path.clone(), v.clone()));
                }
                walk(&n.children, &path, include_private, out);
            }
        }
        let mut out = Vec::new();
        for s in &self.sections {
            walk(&s.nodes, s.id, include_private, &mut out);
        }
        out
    }
}

/// Makes a lowercase, hyphenated key from a label.
#[must_use]
pub fn slug(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut dash = false;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "item".to_owned()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Report {
        let mut s = Section::new("ram", "RAM");
        s.push(Node::item("Total", "48 GB"));
        s.push(
            Node::group("Slot DIMM 1")
                .with(Node::item("Part number", "F4-3200C16"))
                .with(Node::item("Serial", "ABC123").private()),
        );
        s.push(Node::item("Available", "33 GB").volatile());
        let mut net = Section::new("network", "Network");
        net.push(Node::item("Adapter", "Realtek 8851BE"));
        Report {
            generator: "test".into(),
            collected_at: None,
            sections: vec![s, net],
            notes: vec![],
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Slot DIMM 1"), "slot-dimm-1");
        assert_eq!(slug("  L1 (data) cache "), "l1-data-cache");
        assert_eq!(slug("***"), "item");
    }

    #[test]
    fn redaction_replaces_only_private_values() {
        let r = sample().redacted();
        let slot = &r.sections[0].nodes[1];
        assert_eq!(slot.children[1].value.as_deref(), Some(REDACTED));
        assert_eq!(slot.children[0].value.as_deref(), Some("F4-3200C16"));
    }

    #[test]
    fn filter_keeps_ancestors_and_drops_empty_sections() {
        let r = sample().filtered("f4-3200", false);
        assert_eq!(r.sections.len(), 1);
        assert_eq!(r.sections[0].nodes.len(), 1);
        assert_eq!(r.sections[0].nodes[0].children.len(), 1);
        assert!(
            sample().filtered("abc123", true).sections.is_empty(),
            "redacted values are not searchable"
        );
    }

    #[test]
    fn flatten_skips_volatile_and_private() {
        let flat = sample().flatten(false);
        assert_eq!(
            flat,
            vec![
                ("ram/total".to_owned(), "48 GB".to_owned()),
                (
                    "ram/slot-dimm-1/part-number".to_owned(),
                    "F4-3200C16".to_owned()
                ),
                ("network/adapter".to_owned(), "Realtek 8851BE".to_owned()),
            ]
        );
        assert_eq!(sample().flatten(true).len(), 4);
    }

    #[test]
    fn duplicate_keys_get_suffixes() {
        let mut s = Section::new("x", "X");
        s.push(Node::item("Drive", "A"));
        s.push(Node::item("Drive", "B"));
        let r = Report {
            sections: vec![s],
            ..Report::default()
        };
        let flat = r.flatten(true);
        assert_eq!(flat[1].0, "x/drive#2");
    }

    #[test]
    fn only_sections_orders_and_ignores_unknown() {
        let r = sample().only_sections(&["network", "nope", "ram"]);
        let ids: Vec<_> = r.sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["network", "ram"]);
    }
}
