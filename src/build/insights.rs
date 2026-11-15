//! Sections that answer questions rather than list hardware: the upgrade
//! advisor, device problems, driver and firmware age, and Windows 11
//! readiness.

use super::devices::{bus_type, disk_health, media_type};
use super::system::{bios_date, memory_facts};
use super::{Ctx, join, num, text};
use crate::model::{Node, Section, Status, opt};
use crate::smart::Verdict;
use crate::smbios::SlotUsage;
use crate::util::{self, Date, bytes_binary};

/// Drivers and firmware older than this many years are flagged.
pub const OLD_YEARS: i64 = 3;

/// Builds the Upgrade Advisor section.
pub fn upgrade(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("upgrade", "Upgrade Advisor");
    s.push(memory_advice(ctx));
    s.push(slot_advice(ctx));
    s.push(storage_advice(ctx));
    let bb = ctx.first("baseBoard");
    let board = join(
        &[
            bb.and_then(|b| text(b, "Manufacturer")).or_else(|| {
                ctx.smbios
                    .baseboard
                    .as_ref()
                    .and_then(|b| b.manufacturer.clone())
            }),
            bb.and_then(|b| text(b, "Product")).or_else(|| {
                ctx.smbios
                    .baseboard
                    .as_ref()
                    .and_then(|b| b.product.clone())
            }),
        ],
        " ",
    );
    s.push(board.map(|b| {
        Node::item(
            "Board manual",
            format!("Search for \u{201c}{b} manual\u{201d} for exact slot, lane-sharing and memory support rules."),
        )
        .status(Status::Note)
    }));
    s
}

fn memory_advice(ctx: &Ctx<'_>) -> Option<Node> {
    let f = memory_facts(ctx);
    if f.installed == 0 {
        return None;
    }
    let mut g = Node::group("Memory");
    let kind = f.kind.clone().unwrap_or_else(|| "matching".to_owned());
    g.push(Node::item(
        "Installed",
        match f.max_bytes {
            Some(max) => format!(
                "{} of {} supported",
                bytes_binary(f.installed),
                bytes_binary(max)
            ),
            None => bytes_binary(f.installed),
        },
    ));
    if let Some(total) = f.total_slots {
        let free = total.saturating_sub(f.used_slots);
        g.push(Node::item(
            "Slots",
            format!("{} of {total} used, {free} free", f.used_slots),
        ));
        let advice = if free > 0 {
            Node::item(
                "You can",
                format!(
                    "Add up to {free} more {kind} module{}. Match the existing speed{} so every module runs at full speed.",
                    if free == 1 { "" } else { "s" },
                    f.rated_mts.map_or_else(String::new, |r| format!(" ({r} MT/s)"))
                ),
            )
            .status(Status::Good)
        } else if f.max_bytes.is_some_and(|m| f.installed < m) {
            Node::item(
                "You can",
                format!(
                    "All slots are full. To go past {}, replace modules with larger {kind} ones (up to {} total).",
                    bytes_binary(f.installed),
                    f.max_bytes.map_or_else(String::new, bytes_binary)
                ),
            )
            .status(Status::Note)
        } else {
            Node::item("You can", "Memory is at the board's maximum.").status(Status::Note)
        };
        g.push(advice);
    }
    if let (Some(r), Some(c)) = (f.rated_mts, f.configured_mts)
        && c < r
    {
        g.push(
            Node::item(
                "Free speed",
                format!(
                    "Modules are rated for {r} MT/s but run at {c} MT/s. Enabling the XMP or EXPO profile in the BIOS usually fixes this."
                ),
            )
            .status(Status::Caution),
        );
    }
    Some(g)
}

fn slot_advice(ctx: &Ctx<'_>) -> Option<Node> {
    let slots = &ctx.smbios.slots;
    let mut g = Node::group("Expansion slots");
    if slots.is_empty() {
        let win = ctx.list("systemSlots");
        if win.is_empty() {
            return None;
        }
        let free: Vec<String> = win
            .iter()
            .filter(|s| num(s, "CurrentUsage") == Some(3))
            .filter_map(|s| text(s, "SlotDesignation"))
            .collect();
        g.push(Node::item(
            "Free slots",
            if free.is_empty() {
                "None reported".to_owned()
            } else {
                free.join(", ")
            },
        ));
        return Some(g);
    }
    let free: Vec<_> = slots
        .iter()
        .filter(|s| s.usage == SlotUsage::Available)
        .collect();
    let used = slots.iter().filter(|s| s.usage == SlotUsage::InUse).count();
    g.push(Node::item(
        "Slots",
        format!("{} total, {used} in use, {} free", slots.len(), free.len()),
    ));
    for s in &free {
        let name = s
            .designation
            .clone()
            .unwrap_or_else(|| "Unnamed slot".to_owned());
        g.push(
            Node::item(
                format!("Free: {name}"),
                join(
                    &[
                        Some(s.kind.clone()),
                        s.width.clone().map(|w| format!("{w} lanes")),
                    ],
                    ", ",
                )
                .unwrap_or_default(),
            )
            .status(Status::Good),
        );
    }
    if !slots.iter().any(|s| s.kind.starts_with("M.2")) {
        g.push(
            Node::item(
                "M.2 sockets",
                "Windows cannot see empty M.2 sockets on this board; check the board manual.",
            )
            .status(Status::Note),
        );
    }
    Some(g)
}

fn storage_advice(ctx: &Ctx<'_>) -> Option<Node> {
    let disks = ctx.list("diskDrives");
    if disks.is_empty() {
        return None;
    }
    let mut kinds: Vec<(String, usize)> = Vec::new();
    for d in &disks {
        let index = num(d, "Index").map(|i| i.to_string());
        let pd = ctx
            .list("physicalDisks")
            .into_iter()
            .find(|p| text(p, "deviceId") == index);
        let bus = pd
            .and_then(|p| text(p, "busType"))
            .map(|b| bus_type(&b))
            .or_else(|| text(d, "InterfaceType"))
            .unwrap_or_else(|| "Other".to_owned());
        let media = pd
            .and_then(|p| text(p, "mediaType"))
            .and_then(|m| media_type(&m));
        let label = join(&[Some(bus), media], " ").unwrap_or_default();
        if let Some(e) = kinds.iter_mut().find(|(k, _)| *k == label) {
            e.1 += 1;
        } else {
            kinds.push((label, 1));
        }
    }
    let summary = kinds
        .iter()
        .map(|(k, n)| format!("{n} \u{d7} {k}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut g = Node::group("Storage").with(Node::item("Drives", summary));
    let worst = disks.iter().filter_map(|d| disk_health(ctx, d).0).max();
    if let Some(v) = worst
        && v != Verdict::Good
    {
        g.push(
            Node::item(
                "Replace first",
                "A drive shows warning signs; see Storage. Back up before upgrading anything else.",
            )
            .status(if v == Verdict::Bad {
                Status::Bad
            } else {
                Status::Caution
            }),
        );
    }
    g.push(
        Node::item(
            "Free ports",
            "Windows does not report empty SATA ports or M.2 sockets; check the board manual.",
        )
        .status(Status::Note),
    );
    Some(g)
}

/// Explains a Device Manager problem code.
#[must_use]
pub const fn problem_text(code: u64) -> &'static str {
    match code {
        1 => "Not configured correctly",
        3 => "Driver may be corrupted, or the system is low on memory",
        10 => "Cannot start",
        12 => "Not enough free resources",
        14 => "Restart required",
        18 => "Reinstall the drivers",
        19 => "Configuration in the registry is damaged",
        21 => "Windows is removing this device",
        22 => "Disabled",
        24 => "Not present, not working, or missing drivers",
        28 => "Drivers are not installed",
        29 => "Disabled by firmware",
        31 => "Not working properly",
        32 => "Driver service is disabled",
        37 => "Driver failed to initialise",
        39 => "Driver is corrupted or missing",
        41 => "Driver loaded but the hardware was not found",
        43 => "Windows stopped this device because it reported problems",
        45 => "Not currently connected",
        47 => "Prepared for safe removal",
        48 => "Driver blocked because of known problems",
        52 => "Driver signature cannot be verified",
        _ => "Problem reported",
    }
}

/// Builds the Device Problems section.
pub fn problems(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("problems", "Device Problems");
    let list = ctx.list("deviceProblems");
    if list.is_empty() {
        if !ctx.failed("deviceProblems")
            && ctx.raw.get("deviceProblems").items().is_empty()
            && !ctx.raw.get("deviceProblems").is_null()
        {
            s.push(Node::item("Result", "No devices report problems").status(Status::Good));
        }
        return s;
    }
    let mut present = Vec::new();
    let mut absent = Vec::new();
    for d in list {
        let name = text(d, "Name").unwrap_or_else(|| "Unnamed device".to_owned());
        let code = num(d, "ConfigManagerErrorCode").unwrap_or(0);
        let class = text(d, "PNPClass");
        let mut n = Node::item(name, format!("Code {code}: {}", problem_text(code)));
        n.push(opt("Class", class));
        n.push(opt("Device ID", text(d, "PNPDeviceID")));
        if d.get("Present").as_bool() == Some(false) || code == 45 {
            absent.push(n.status(Status::Note));
        } else {
            let status = if matches!(code, 22 | 45 | 47) {
                Status::Note
            } else {
                Status::Caution
            };
            present.push(n.status(status));
        }
    }
    s.push(
        Node::item("Devices with problems", present.len().to_string()).status(
            if present.is_empty() {
                Status::Good
            } else {
                Status::Caution
            },
        ),
    );
    for n in present {
        s.push(n);
    }
    if !absent.is_empty() {
        let mut g = Node::item("Not connected", absent.len().to_string());
        for n in absent {
            g.push(n);
        }
        s.push(g);
    }
    s
}

/// Builds the Drivers & Firmware Age section.
pub fn drivers(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("drivers", "Drivers & Firmware Age");
    let Some(today) = ctx.today else { return s };
    if let Some(d) = bios_date(ctx) {
        let old = d.years_until(today) >= OLD_YEARS;
        s.push(
            Node::item(
                "BIOS",
                format!("{} ({})", d.iso(), util::age_label(d, today)),
            )
            .status(if old { Status::Caution } else { Status::Normal }),
        );
    }
    let mut old = Vec::new();
    let mut all: Vec<(String, Node)> = Vec::new();
    for drv in ctx.list("drivers") {
        let Some(name) = text(drv, "DeviceName") else {
            continue;
        };
        let provider = text(drv, "DriverProviderName");
        // Inbox Microsoft drivers carry placeholder dates (often 2006).
        if provider.as_deref() == Some("Microsoft") {
            continue;
        }
        let date = text(drv, "DriverDate").and_then(|d| Date::parse(&d));
        let value = join(
            &[
                text(drv, "DriverVersion"),
                date.map(|d| format!("{} ({})", d.iso(), util::age_label(d, today))),
                provider,
            ],
            ", ",
        )
        .unwrap_or_default();
        let is_old = date.is_some_and(|d| d.years_until(today) >= OLD_YEARS);
        let node = Node::item(name, value).status(if is_old {
            Status::Caution
        } else {
            Status::Normal
        });
        if is_old {
            old.push(node.clone());
        }
        let class = text(drv, "DeviceClass").unwrap_or_else(|| "OTHER".to_owned());
        if let Some((_, g)) = all.iter_mut().find(|(c, _)| *c == class) {
            g.push(node);
        } else {
            all.push((class.clone(), Node::group(class.to_lowercase()).with(node)));
        }
    }
    if !all.is_empty() {
        let mut flagged = Node::item(
            format!("Drivers older than {OLD_YEARS} years"),
            old.len().to_string(),
        )
        .status(if old.is_empty() {
            Status::Good
        } else {
            Status::Caution
        });
        flagged.children = old;
        s.push(flagged);
        all.sort_by(|a, b| a.0.cmp(&b.0));
        let count: usize = all.iter().map(|(_, g)| g.children.len()).sum();
        let mut g = Node::item("Third-party drivers", count.to_string());
        g.children = all.into_iter().map(|(_, n)| n).collect();
        s.push(g);
        s.push(
            Node::item("Updates", "OpenPCManifest never downloads drivers. Use Windows Update or the device maker's site.")
                .status(Status::Note),
        );
    }
    s
}

/// The individual Windows 11 requirement checks.
fn readiness_checks(ctx: &Ctx<'_>, os: &crate::json::Value) -> Vec<Node> {
    let mut checks: Vec<Node> = Vec::new();

    // TPM
    let tpm = ctx.first("tpm");
    checks.push(match tpm {
        Some(t) => {
            let spec = text(t, "SpecVersion").unwrap_or_default();
            let version = spec.split(',').next().unwrap_or("").trim().to_owned();
            let ok = version.starts_with('2');
            Node::item(
                "TPM",
                join(
                    &[
                        Some(format!("TPM {version}")),
                        text(t, "ManufacturerIdTxt").map(|m| format!("({m})")),
                    ],
                    " ",
                )
                .unwrap_or_default(),
            )
            .status(if ok { Status::Good } else { Status::Bad })
        }
        None if ctx.failed("tpm") => {
            Node::item("TPM", "Unknown (run as administrator)").status(Status::Note)
        }
        None => Node::item("TPM", "Not found").status(Status::Bad),
    });

    // Firmware and Secure Boot
    let firmware = ctx.raw.get("firmwareType").as_str();
    checks.push(opt("Firmware", firmware).map_or_else(
        || Node::item("Firmware", "Unknown").status(Status::Note),
        |n| {
            n.status(if firmware == Some("UEFI") {
                Status::Good
            } else {
                Status::Bad
            })
        },
    ));
    let sb = ctx.raw.path(&["secureBoot", "enabled"]).as_u64();
    checks.push(match sb {
        Some(1) => Node::item("Secure Boot", "On").status(Status::Good),
        Some(_) => Node::item(
            "Secure Boot",
            "Off (can usually be turned on in UEFI settings)",
        )
        .status(Status::Caution),
        None => Node::item("Secure Boot", "Not supported or not reported").status(Status::Caution),
    });

    // CPU
    if let Some(info) = &ctx.cpu {
        checks.push(match crate::cpu::likely_windows11_cpu(info) {
            Some(true) => {
                Node::item("Processor", "Likely supported (estimate)").status(Status::Good)
            }
            Some(false) => Node::item(
                "Processor",
                "Likely not on Microsoft's supported list (estimate)",
            )
            .status(Status::Caution),
            None => Node::item("Processor", "Unknown").status(Status::Note),
        });
    }

    // Memory and system drive
    if let Some(kib) = num(os, "TotalVisibleMemorySize") {
        let bytes = kib * 1024;
        let ok = bytes >= 3_800 * 1024 * 1024;
        checks.push(
            Node::item("Memory", format!("{} (4 GB required)", bytes_binary(bytes)))
                .status(if ok { Status::Good } else { Status::Bad }),
        );
    }
    if let Some(c) = ctx
        .list("logicalDisks")
        .into_iter()
        .find(|d| text(d, "DeviceID").as_deref() == Some("C:"))
        && let Some(size) = num(c, "Size")
    {
        let ok = size >= 64_000_000_000;
        checks.push(
            Node::item(
                "System drive",
                format!("{} (64 GB required)", util::bytes_decimal(size)),
            )
            .status(if ok { Status::Good } else { Status::Bad }),
        );
    }
    checks
}

/// Builds the Windows 11 Readiness section.
pub fn windows11(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("windows11", "Windows 11 Readiness");
    let Some(os) = ctx.first("os") else { return s };
    let running = text(os, "Caption").is_some_and(|c| c.contains("Windows 11"));
    let checks = readiness_checks(ctx, os);

    let worst = checks
        .iter()
        .map(|c| c.status)
        .fold(Status::Good, |acc, st| match (acc, st) {
            (Status::Bad, _) | (_, Status::Bad) => Status::Bad,
            (Status::Caution, _) | (_, Status::Caution) => Status::Caution,
            _ => acc,
        });
    let verdict =
        if running {
            Node::item("Result", "Already running Windows 11").status(Status::Good)
        } else {
            match worst {
                Status::Bad => Node::item("Result", "Does not meet Windows 11 requirements")
                    .status(Status::Bad),
                Status::Caution => Node::item("Result", "May need changes before upgrading")
                    .status(Status::Caution),
                _ => Node::item("Result", "Meets the checked requirements").status(Status::Good),
            }
        };
    s.push(verdict);
    for c in checks {
        s.push(c);
    }

    let ca = ctx.raw.path(&["secureBoot", "ca2023Status"]).as_str();
    s.push(match ca {
        Some(v) if v.eq_ignore_ascii_case("Updated") => Node::item("Secure Boot 2023 certificates", "Updated").status(Status::Good),
        Some(v) => Node::item("Secure Boot 2023 certificates", v.to_owned()).status(Status::Caution),
        None => Node::item(
            "Secure Boot 2023 certificates",
            "Not reported. The 2011 Microsoft Secure Boot certificates expired in 2026; Windows Update installs the replacements.",
        )
        .status(Status::Note),
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_codes() {
        assert_eq!(problem_text(28), "Drivers are not installed");
        assert_eq!(problem_text(9999), "Problem reported");
    }
}
