//! ATA SMART attribute and threshold parsing, plus a plain-language health
//! verdict in the spirit of `CrystalDiskInfo`'s Good / Caution / Bad.
//!
//! The input is the 512-byte `VendorSpecific` page from
//! `MSStorageDriver_FailurePredictData` and the matching page from
//! `MSStorageDriver_FailurePredictThresholds`.

/// One SMART attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Attribute ID.
    pub id: u8,
    /// Normalised current value.
    pub current: u8,
    /// Worst normalised value seen.
    pub worst: u8,
    /// Failure threshold, if the threshold page was available.
    pub threshold: Option<u8>,
    /// The 48-bit raw value.
    pub raw: u64,
}

impl Attribute {
    /// The attribute's common name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        attribute_name(self.id)
    }

    /// True if the normalised value has reached its failure threshold.
    #[must_use]
    pub fn is_failing(&self) -> bool {
        self.threshold.is_some_and(|t| t > 0 && self.current <= t)
    }

    /// A human-readable raw value for attributes with known units.
    #[must_use]
    pub fn display_raw(&self) -> String {
        match self.id {
            // Temperature: low byte is degrees Celsius.
            0xC2 | 0xBE => format!("{} \u{b0}C", self.raw & 0xFF),
            0x09 => format!("{} h", self.raw & 0xFFFF_FFFF),
            _ => self.raw.to_string(),
        }
    }
}

/// The overall verdict for a drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// No warning signs.
    Good,
    /// Warning signs; back up and keep an eye on it.
    Caution,
    /// An attribute is at or past its failure threshold, or the drive
    /// predicts its own failure.
    Bad,
}

impl Verdict {
    /// A display label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Good => "Good",
            Self::Caution => "Caution",
            Self::Bad => "Bad",
        }
    }
}

/// A verdict with the reasons that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    /// The verdict.
    pub verdict: Verdict,
    /// Plain-language reasons, worst first. Empty when `Good`.
    pub reasons: Vec<String>,
}

/// Parses an attribute page and an optional threshold page.
#[must_use]
pub fn parse(attributes: &[u8], thresholds: Option<&[u8]>) -> Vec<Attribute> {
    let threshold_for = |id: u8| -> Option<u8> {
        let page = thresholds?;
        (0..30).find_map(|i| {
            let entry = page.get(2 + i * 12..2 + i * 12 + 12)?;
            if *entry.first()? == id {
                entry.get(1).copied()
            } else {
                None
            }
        })
    };
    (0..30)
        .filter_map(|i| {
            let e = attributes.get(2 + i * 12..2 + i * 12 + 12)?;
            let id = *e.first()?;
            if id == 0 {
                return None;
            }
            let raw_bytes = e.get(5..11)?;
            let raw = raw_bytes
                .iter()
                .rev()
                .fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
            Some(Attribute {
                id,
                current: *e.get(3)?,
                worst: *e.get(4)?,
                threshold: threshold_for(id),
                raw,
            })
        })
        .collect()
}

/// Judges drive health from SMART attributes and the drive's own failure
/// prediction flag.
#[must_use]
pub fn assess(attributes: &[Attribute], predicts_failure: bool) -> Health {
    let mut reasons: Vec<(Verdict, String)> = Vec::new();
    if predicts_failure {
        reasons.push((
            Verdict::Bad,
            "The drive predicts its own failure".to_owned(),
        ));
    }
    for a in attributes {
        if a.is_failing() {
            reasons.push((
                Verdict::Bad,
                format!(
                    "{} is at its failure threshold ({} \u{2264} {})",
                    a.name(),
                    a.current,
                    a.threshold.unwrap_or(0)
                ),
            ));
        }
        let caution = match a.id {
            0x05 => Some("reallocated sectors"),
            0xC4 => Some("reallocation events"),
            0xC5 => Some("sectors pending reallocation"),
            0xC6 => Some("uncorrectable sectors"),
            0xBB => Some("reported uncorrectable errors"),
            _ => None,
        };
        if let Some(what) = caution
            && a.raw > 0
        {
            reasons.push((Verdict::Caution, format!("{} {what}", a.raw)));
        }
    }
    reasons.sort_by_key(|r| std::cmp::Reverse(r.0));
    let verdict = reasons.first().map_or(Verdict::Good, |r| r.0);
    Health {
        verdict,
        reasons: reasons.into_iter().map(|r| r.1).collect(),
    }
}

/// The common name of a SMART attribute ID.
#[must_use]
pub const fn attribute_name(id: u8) -> &'static str {
    match id {
        0x01 => "Read Error Rate",
        0x02 => "Throughput Performance",
        0x03 => "Spin-Up Time",
        0x04 => "Start/Stop Count",
        0x05 => "Reallocated Sectors Count",
        0x07 => "Seek Error Rate",
        0x08 => "Seek Time Performance",
        0x09 => "Power-On Hours",
        0x0A => "Spin Retry Count",
        0x0B => "Recalibration Retries",
        0x0C => "Power Cycle Count",
        0xA0 => "Uncorrectable Sector Count (vendor)",
        0xA1 => "Valid Spare Blocks",
        0xA5 | 0xA7 => "Vendor Specific",
        0xA9 => "Remaining Life",
        0xAA => "Available Reserved Space",
        0xAB => "Program Fail Count",
        0xAC => "Erase Fail Count",
        0xAD => "Wear Leveling Count",
        0xAE => "Unexpected Power Loss Count",
        0xAF => "Power Loss Protection Failure",
        0xB1 => "Wear Range Delta",
        0xB4 => "Unused Reserved Block Count",
        0xB5 => "Program Fail Count Total",
        0xB6 => "Erase Fail Count Total",
        0xB7 => "SATA Downshift Error Count",
        0xB8 => "End-to-End Error",
        0xBB => "Reported Uncorrectable Errors",
        0xBC => "Command Timeout",
        0xBD => "High Fly Writes",
        0xBE => "Airflow Temperature",
        0xBF => "G-Sense Error Rate",
        0xC0 => "Power-off Retract Count",
        0xC1 => "Load/Unload Cycle Count",
        0xC2 => "Temperature",
        0xC3 => "Hardware ECC Recovered",
        0xC4 => "Reallocation Event Count",
        0xC5 => "Current Pending Sector Count",
        0xC6 => "Uncorrectable Sector Count",
        0xC7 => "UltraDMA CRC Error Count",
        0xC8 => "Write Error Rate",
        0xCA => "Data Address Mark Errors",
        0xCE => "Flying Height",
        0xD2 => "Vibration During Write",
        0xE7 => "SSD Life Left",
        0xE8 => "Endurance Remaining",
        0xE9 => "Media Wearout Indicator",
        0xF1 => "Total LBAs Written",
        0xF2 => "Total LBAs Read",
        _ => "Unknown attribute",
    }
}

/// Builds attribute and threshold pages, for tests and fixtures.
#[cfg(test)]
pub mod testing {
    /// Builds a 512-byte attribute page from `(id, current, worst, raw)`.
    #[must_use]
    pub fn attribute_page(entries: &[(u8, u8, u8, u64)]) -> Vec<u8> {
        let mut page = vec![0u8; 512];
        page[0] = 0x10;
        for (i, &(id, cur, worst, raw)) in entries.iter().enumerate() {
            let base = 2 + i * 12;
            page[base] = id;
            page[base + 1] = 0x0B;
            page[base + 3] = cur;
            page[base + 4] = worst;
            page[base + 5..base + 11].copy_from_slice(&raw.to_le_bytes()[..6]);
        }
        page
    }

    /// Builds a 512-byte threshold page from `(id, threshold)`.
    #[must_use]
    pub fn threshold_page(entries: &[(u8, u8)]) -> Vec<u8> {
        let mut page = vec![0u8; 512];
        for (i, &(id, t)) in entries.iter().enumerate() {
            page[2 + i * 12] = id;
            page[2 + i * 12 + 1] = t;
        }
        page
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn parses_attributes_with_thresholds() {
        let attrs = attribute_page(&[
            (0x05, 200, 200, 0),
            (0x09, 65, 63, 0x65F2),
            (0xC2, 115, 104, 0x23),
        ]);
        let thresholds = threshold_page(&[(0x05, 140), (0x09, 0)]);
        let parsed = parse(&attrs, Some(&thresholds));
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].name(), "Reallocated Sectors Count");
        assert_eq!(parsed[0].threshold, Some(140));
        assert_eq!(parsed[1].raw, 0x65F2);
        assert_eq!(parsed[1].display_raw(), "26098 h");
        assert_eq!(parsed[2].display_raw(), "35 \u{b0}C");
        assert_eq!(parsed[2].threshold, None);
    }

    #[test]
    fn healthy_drive_is_good() {
        let attrs = parse(
            &attribute_page(&[(0x05, 200, 200, 0), (0xC5, 200, 200, 0)]),
            None,
        );
        let h = assess(&attrs, false);
        assert_eq!(h.verdict, Verdict::Good);
        assert!(h.reasons.is_empty());
    }

    #[test]
    fn reallocation_events_are_caution() {
        let attrs = parse(&attribute_page(&[(0xC4, 100, 100, 16)]), None);
        let h = assess(&attrs, false);
        assert_eq!(h.verdict, Verdict::Caution);
        assert_eq!(h.reasons, vec!["16 reallocation events"]);
    }

    #[test]
    fn threshold_crossing_is_bad_and_listed_first() {
        let attrs = parse(
            &attribute_page(&[(0xC5, 100, 100, 3), (0x05, 100, 100, 900)]),
            Some(&threshold_page(&[(0x05, 140)])),
        );
        let h = assess(&attrs, false);
        assert_eq!(h.verdict, Verdict::Bad);
        assert!(h.reasons[0].starts_with("Reallocated Sectors Count is at its failure threshold"));
        assert_eq!(h.reasons.len(), 3);
    }

    #[test]
    fn predicted_failure_is_bad() {
        assert_eq!(assess(&[], true).verdict, Verdict::Bad);
    }

    #[test]
    fn short_pages_do_not_panic() {
        for len in 0..40 {
            let _ = parse(&vec![1u8; len], Some(&vec![1u8; len]));
        }
    }
}
