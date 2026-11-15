//! Operating system, CPU, RAM and motherboard sections.

use super::{Ctx, join, num, text};
use crate::model::{Node, Section, Status, opt};
use crate::smbios::{self, MemoryDevice, SlotUsage};
use crate::util::{self, Date, bytes_binary};

/// Seconds between two ISO 8601 UTC timestamps (`YYYY-MM-DDTHH:MM:SS...`).
fn seconds_between(earlier: &str, later: &str) -> Option<u64> {
    fn secs(s: &str) -> Option<i64> {
        let d = Date::parse(s)?;
        let h: i64 = s.get(11..13)?.parse().ok()?;
        let m: i64 = s.get(14..16)?.parse().ok()?;
        let sec: i64 = s.get(17..19)?.parse().ok()?;
        Some(d.days() * 86_400 + h * 3600 + m * 60 + sec)
    }
    u64::try_from(secs(later)? - secs(earlier)?).ok()
}

fn date_with_age(ctx: &Ctx<'_>, raw: Option<&str>) -> Option<String> {
    let d = Date::parse(raw?)?;
    Some(ctx.today.map_or_else(
        || d.iso(),
        |today| format!("{} ({})", d.iso(), util::age_label(d, today)),
    ))
}

/// The OS edition line, such as `Windows 11 Pro 64-bit`.
pub fn os_name(ctx: &Ctx<'_>) -> Option<String> {
    let os = ctx.first("os")?;
    let caption = text(os, "Caption")?;
    let caption = caption
        .strip_prefix("Microsoft ")
        .unwrap_or(&caption)
        .to_owned();
    Some(join(&[Some(caption), text(os, "OSArchitecture")], " ").unwrap_or_default())
}

/// Builds the Operating System section.
pub fn os(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("os", "Operating System");
    let Some(os) = ctx.first("os") else { return s };
    let wv = ctx.raw.get("windowsVersion");
    let name = os_name(ctx);
    let build = join(&[text(os, "BuildNumber"), text(wv, "ubr")], ".");
    let version = text(wv, "displayVersion");

    s.push(opt(
        "summary-os",
        join(
            &[
                name.clone(),
                join(
                    &[
                        version.clone(),
                        build.as_ref().map(|b| format!("build {b}")),
                    ],
                    ", ",
                )
                .map(|v| format!("({v})")),
            ],
            " ",
        ),
    ));
    if let Some(n) = s.nodes.last_mut() {
        "Operating system".clone_into(&mut n.label);
    }

    s.push(opt("Edition", name));
    s.push(opt("Version", version));
    s.push(opt("Build", build));
    s.push(opt(
        "Installed",
        date_with_age(ctx, text(os, "InstallDate").as_deref()),
    ));
    if let Some(boot) = text(os, "LastBootUpTime") {
        let shown = format!(
            "{} {} UTC",
            boot.get(..10).unwrap_or(&boot),
            boot.get(11..16).unwrap_or("")
        );
        s.push(Node::item("Last boot", shown).volatile());
        if let Some(secs) = ctx
            .raw
            .get("collectedAt")
            .as_str()
            .and_then(|now| seconds_between(&boot, now))
        {
            s.push(Node::item("Uptime", util::duration(secs)).volatile());
        }
    }
    s.push(opt("Computer name", text(os, "CSName")).map(Node::private));
    s.push(opt("Registered user", text(os, "RegisteredUser")).map(Node::private));
    s.push(opt("Product ID", text(os, "SerialNumber")).map(Node::private));
    if let Some(cs) = ctx.first("computerSystem") {
        s.push(opt(
            "System model",
            join(&[text(cs, "Manufacturer"), text(cs, "Model")], " "),
        ));
        let group = if cs.get("PartOfDomain").as_bool() == Some(true) {
            "Domain"
        } else {
            "Workgroup"
        };
        s.push(opt(group, text(cs, group)).map(Node::private));
    }
    s.push(security(ctx));
    s.push(runtimes(ctx));
    s.push(opt(
        "Power plan",
        ctx.raw.get("powerPlan").as_str().and_then(power_plan_name),
    ));
    s.push(opt("Firmware", ctx.raw.get("firmwareType").as_str()));
    s.push(opt("Time zone", ctx.raw.get("timeZone").as_str()));
    s.push(opt("Locale", ctx.raw.get("culture").as_str()));
    s.push(hotfixes(ctx));
    s
}

/// Extracts `Balanced` from `powercfg` output such as
/// `Power Scheme GUID: 381b...  (Balanced)`.
fn power_plan_name(output: &str) -> Option<String> {
    let open = output.rfind('(')?;
    let close = output.rfind(')')?;
    let name = output.get(open + 1..close)?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Decodes `AntiVirusProduct.productState` into (enabled, up to date).
#[must_use]
pub const fn antivirus_state(state: u64) -> (bool, bool) {
    let enabled = matches!((state >> 8) & 0xFF, 0x10 | 0x11);
    let up_to_date = state.trailing_zeros() >= 8;
    (enabled, up_to_date)
}

fn security(ctx: &Ctx<'_>) -> Option<Node> {
    let mut g = Node::group("Security");
    let uac = ctx.raw.get("uac");
    if let Some(lua) = uac.get("enableLua").as_u64() {
        g.push(
            Node::item(
                "User Account Control",
                if lua == 1 { "Enabled" } else { "Disabled" },
            )
            .status(if lua == 1 {
                Status::Normal
            } else {
                Status::Caution
            }),
        );
    }
    for fw in ctx.list("firewall") {
        if let Some(name) = text(fw, "Name") {
            let on = fw.get("Enabled").as_str() == Some("True");
            g.push(
                Node::item(format!("Firewall ({name})"), if on { "On" } else { "Off" })
                    .status(if on { Status::Normal } else { Status::Caution }),
            );
        }
    }
    for av in ctx.list("antivirus") {
        if let Some(name) = text(av, "displayName") {
            let detail = av.get("productState").as_u64().map(|st| {
                let (on, current) = antivirus_state(st);
                format!(
                    "{}, {}",
                    if on { "on" } else { "off" },
                    if current {
                        "definitions up to date"
                    } else {
                        "definitions out of date"
                    }
                )
            });
            g.push(Node::item(
                format!("Antivirus: {name}"),
                detail.unwrap_or_else(|| "installed".to_owned()),
            ));
        }
    }
    (!g.children.is_empty()).then_some(g)
}

fn runtimes(ctx: &Ctx<'_>) -> Option<Node> {
    let r = ctx.raw.get("runtimes");
    let mut g = Node::group("Runtimes");
    g.push(opt(".NET Framework", text(r, "dotnetFramework")));
    g.push(opt("Windows PowerShell", text(r, "windowsPowerShell")));
    g.push(opt("PowerShell 7", text(r, "powershell7")));
    g.push(opt("Java", text(r, "java")));
    (!g.children.is_empty()).then_some(g)
}

fn hotfixes(ctx: &Ctx<'_>) -> Option<Node> {
    let list = ctx.list("hotfixes");
    if list.is_empty() {
        return None;
    }
    let mut g = Node::item("Installed updates", list.len().to_string());
    for h in list {
        if let Some(id) = text(h, "HotFixID") {
            let date = text(h, "InstalledOn")
                .and_then(|d| Date::parse(&d))
                .map(Date::iso);
            g.push(
                Node::item(
                    id.clone(),
                    join(&[text(h, "Description"), date], ", ").unwrap_or_default(),
                )
                .with_key(id),
            );
        }
    }
    Some(g)
}

/// Builds the CPU section.
pub fn cpu(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("cpu", "CPU");
    let p = ctx.first("processor");
    let info = ctx.cpu.as_ref();
    let name = info
        .and_then(|i| i.brand.clone())
        .or_else(|| p.and_then(|p| text(p, "Name")));
    let cores = p.and_then(|p| num(p, "NumberOfCores"));
    let threads = p.and_then(|p| num(p, "NumberOfLogicalProcessors"));
    let code = info.and_then(|i| i.code_name);

    let summary = join(
        &[
            name.clone(),
            code.map(|(c, n)| format!("({c}, {n})")),
            match (cores, threads) {
                (Some(c), Some(t)) => Some(format!("\u{2014} {c} cores / {t} threads")),
                _ => None,
            },
        ],
        " ",
    );
    s.push(opt("summary-cpu", summary).map(|n| Node {
        label: "CPU".to_owned(),
        ..n
    }));

    s.push(opt("Name", name));
    if let Some((c, n)) = code {
        s.push(Node::item("Code name", c));
        s.push(Node::item("Technology", n));
    }
    s.push(opt("Cores", cores.map(|c| c.to_string())));
    s.push(opt("Threads", threads.map(|t| t.to_string())));
    let socket = p
        .and_then(|p| text(p, "SocketDesignation"))
        .or_else(|| ctx.smbios.processors.first().and_then(|x| x.socket.clone()));
    s.push(opt("Socket", socket));
    s.push(opt(
        "Base clock",
        p.and_then(|p| num(p, "MaxClockSpeed"))
            .map(|m| format!("{m} MHz")),
    ));
    if let Some(i) = info {
        s.push(Node::item("Vendor", i.vendor.clone()));
        s.push(Node::item(
            "Family / model / stepping",
            format!("{:X}h / {:X}h / {}", i.family, i.model, i.stepping),
        ));
        s.push(Node::item(
            "CPUID fields",
            format!(
                "family {:X}, extended family {:X}, model {:X}, extended model {:X}",
                i.base_family, i.extended_family, i.base_model, i.extended_model
            ),
        ));
        if !i.features.is_empty() {
            s.push(Node::item("Instructions", i.features.join(", ")));
        }
        if i.hybrid {
            s.push(Node::item(
                "Core design",
                "Hybrid (performance and efficiency cores)",
            ));
        }
    }
    s.push(virtualization(ctx));
    if let (Some(c), Some(t)) = (cores, threads) {
        s.push(Node::item(
            "Simultaneous multithreading",
            if t > c {
                "Supported, enabled"
            } else {
                "Not active"
            },
        ));
    }
    if let Some(i) = info
        && !i.caches.is_empty()
    {
        let mut g = Node::group("Caches");
        for c in &i.caches {
            let instances = threads
                .map(|t| t / u64::from(c.shared_by.max(1)))
                .filter(|&n| n > 1);
            let size = bytes_binary(c.size_bytes);
            let value = instances.map_or_else(|| size.clone(), |n| format!("{n} \u{d7} {size}"));
            g.push(Node::item(
                format!("L{} {}", c.level, c.kind.to_lowercase()),
                format!("{value}, {}-way", c.ways),
            ));
        }
        s.push(g);
    }
    s
}

fn virtualization(ctx: &Ctx<'_>) -> Option<Node> {
    let tech = ctx.cpu.as_ref().and_then(|i| i.virtualization);
    let p = ctx.first("processor");
    let firmware = p.and_then(|p| p.get("VirtualizationFirmwareEnabled").as_bool());
    let hypervisor = ctx
        .first("computerSystem")
        .and_then(|c| c.get("HypervisorPresent").as_bool());
    let state = match (hypervisor, firmware) {
        (Some(true), _) => Some("in use by the Windows hypervisor"),
        (_, Some(true)) => Some("enabled in firmware"),
        (_, Some(false)) => Some("disabled in firmware"),
        _ => None,
    };
    let value = join(
        &[tech.map(ToOwned::to_owned), state.map(ToOwned::to_owned)],
        ", ",
    )?;
    let status = if firmware == Some(false) && hypervisor != Some(true) {
        Status::Caution
    } else {
        Status::Normal
    };
    Some(Node::item("Virtualization", value).status(status))
}

/// One installed memory module, merged from Win32 and SMBIOS data.
struct Module {
    locator: String,
    bank: Option<String>,
    size: u64,
    smbios: Option<MemoryDevice>,
    win32: Option<crate::json::Value>,
}

fn modules(ctx: &Ctx<'_>) -> Vec<Module> {
    let win: Vec<_> = ctx.list("physicalMemory");
    if !win.is_empty() {
        return win
            .into_iter()
            .map(|m| {
                let locator = text(m, "DeviceLocator").unwrap_or_else(|| "Unknown slot".to_owned());
                let bank = text(m, "BankLabel");
                // Locators repeat across channels ("DIMM 0" on A and B), so
                // match on the bank label too when both sides have one.
                let smbios = ctx
                    .smbios
                    .memory_devices
                    .iter()
                    .find(|d| {
                        d.is_populated()
                            && d.locator.as_deref() == Some(locator.as_str())
                            && (bank.is_none() || d.bank.is_none() || d.bank == bank)
                    })
                    .cloned();
                Module {
                    bank,
                    size: num(m, "Capacity").unwrap_or(0),
                    locator,
                    smbios,
                    win32: Some(m.clone()),
                }
            })
            .collect();
    }
    ctx.smbios
        .memory_devices
        .iter()
        .filter(|d| d.is_populated())
        .map(|d| Module {
            locator: d
                .locator
                .clone()
                .unwrap_or_else(|| "Unknown slot".to_owned()),
            bank: d.bank.clone(),
            size: d.size_bytes.unwrap_or(0),
            smbios: Some(d.clone()),
            win32: None,
        })
        .collect()
}

/// Memory facts used by the RAM section, the summary and the upgrade advisor.
pub struct MemoryFacts {
    /// Installed bytes.
    pub installed: u64,
    /// Total slots, if known.
    pub total_slots: Option<u64>,
    /// Populated slots.
    pub used_slots: u64,
    /// Memory type, such as `DDR4`.
    pub kind: Option<String>,
    /// Rated speed in MT/s (lowest across modules).
    pub rated_mts: Option<u64>,
    /// Configured speed in MT/s (lowest across modules).
    pub configured_mts: Option<u64>,
    /// Estimated channel count.
    pub channels: Option<usize>,
    /// Maximum supported memory in bytes.
    pub max_bytes: Option<u64>,
}

/// Gathers memory facts.
pub fn memory_facts(ctx: &Ctx<'_>) -> MemoryFacts {
    let mods = modules(ctx);
    let installed = mods.iter().map(|m| m.size).sum();
    let smbios_slots = u64::from(ctx.smbios.total_memory_slots());
    let win_slots: u64 = ctx
        .list("memoryArray")
        .iter()
        .filter_map(|a| num(a, "MemoryDevices"))
        .sum();
    let total_slots = [smbios_slots, win_slots].into_iter().find(|&n| n > 0);
    let kind = mods
        .iter()
        .find_map(|m| {
            m.smbios
                .as_ref()
                .and_then(|d| d.memory_type)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            mods.iter().find_map(|m| {
                let code = m.win32.as_ref().and_then(|w| num(w, "SMBIOSMemoryType"))?;
                smbios::memory_type_name(u8::try_from(code).ok()?).map(ToOwned::to_owned)
            })
        });
    let rated_mts = mods
        .iter()
        .filter_map(|m| {
            m.smbios
                .as_ref()
                .and_then(|d| d.speed_mts.map(u64::from))
                .or_else(|| m.win32.as_ref().and_then(|w| num(w, "Speed")))
        })
        .min();
    let configured_mts = mods
        .iter()
        .filter_map(|m| {
            m.smbios
                .as_ref()
                .and_then(|d| d.configured_speed_mts.map(u64::from))
                .or_else(|| {
                    m.win32
                        .as_ref()
                        .and_then(|w| num(w, "ConfiguredClockSpeed"))
                })
        })
        .min();
    let mut channels: Vec<char> = mods
        .iter()
        .filter_map(|m| {
            channel_letter(m.bank.as_deref()).or_else(|| channel_letter(Some(&m.locator)))
        })
        .collect();
    channels.sort_unstable();
    channels.dedup();
    let max_bytes = ctx.smbios.max_memory_bytes().or_else(|| {
        let a = ctx.first("memoryArray")?;
        let kib = num(a, "MaxCapacityEx")
            .filter(|&v| v > 0)
            .or_else(|| num(a, "MaxCapacity"))?;
        Some(kib * 1024)
    });
    MemoryFacts {
        installed,
        total_slots,
        used_slots: mods.len() as u64,
        kind,
        rated_mts,
        configured_mts,
        channels: (!channels.is_empty()).then_some(channels.len()),
        max_bytes,
    }
}

/// Finds a channel letter in labels such as `P0 CHANNEL A`, `ChannelB-DIMM0`
/// or `DIMM_A1`.
fn channel_letter(label: Option<&str>) -> Option<char> {
    let upper = label?.to_ascii_uppercase();
    if let Some(i) = upper.find("CHANNEL") {
        return upper
            .get(i + 7..)?
            .trim_start_matches([' ', '_', '-'])
            .chars()
            .next()
            .filter(char::is_ascii_alphabetic);
    }
    let rest = upper
        .strip_prefix("DIMM_")
        .or_else(|| upper.strip_prefix("DIMM"))?;
    let mut chars = rest.trim_start_matches([' ', '_']).chars();
    let letter = chars.next()?;
    (letter.is_ascii_alphabetic() && chars.next().is_some_and(|c| c.is_ascii_digit()))
        .then_some(letter)
}

fn channel_label(n: usize) -> String {
    match n {
        1 => "Single".to_owned(),
        2 => "Dual".to_owned(),
        3 => "Triple".to_owned(),
        4 => "Quad".to_owned(),
        n => format!("{n}-channel"),
    }
}

/// Builds the RAM section.
pub fn ram(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("ram", "RAM");
    let f = memory_facts(ctx);
    if f.installed == 0 && f.total_slots.is_none() {
        return s;
    }
    let speed = f
        .configured_mts
        .or(f.rated_mts)
        .map(|m| format!("{m} MT/s ({} MHz)", m / 2));
    let summary = join(
        &[
            Some(bytes_binary(f.installed)),
            f.kind.clone(),
            f.channels
                .map(|c| format!("{}-channel", channel_label(c).to_lowercase())),
            speed.clone().map(|sp| format!("@ {sp}")),
            f.total_slots
                .map(|t| format!("({} of {t} slots used)", f.used_slots)),
        ],
        " ",
    );
    s.push(opt("summary-ram", summary).map(|n| Node {
        label: "RAM".to_owned(),
        ..n
    }));

    if let Some(total) = f.total_slots {
        s.push(
            Node::group("Memory slots")
                .with(Node::item("Total", total.to_string()))
                .with(Node::item("Used", f.used_slots.to_string()))
                .with(Node::item(
                    "Free",
                    total.saturating_sub(f.used_slots).to_string(),
                )),
        );
    }
    s.push(Node::item("Installed", bytes_binary(f.installed)));
    s.push(opt("Type", f.kind.clone()));
    s.push(opt("Channels (estimated)", f.channels.map(channel_label)));
    s.push(opt("Speed", speed));
    if let (Some(r), Some(c)) = (f.rated_mts, f.configured_mts)
        && c < r
    {
        s.push(
            Node::item(
                "Rated speed",
                format!("{r} MT/s \u{2014} modules run slower than rated"),
            )
            .status(Status::Caution),
        );
    }
    s.push(opt("Maximum supported", f.max_bytes.map(bytes_binary)));
    if let Some(os) = ctx.first("os") {
        let total = num(os, "TotalVisibleMemorySize").map(|k| k * 1024);
        let free = num(os, "FreePhysicalMemory").map(|k| k * 1024);
        if let (Some(t), Some(fr)) = (total, free) {
            s.push(
                Node::group("Usage")
                    .with(Node::item("Usable", bytes_binary(t)).volatile())
                    .with(Node::item("Available", bytes_binary(fr)).volatile())
                    .with(
                        opt(
                            "In use",
                            util::percent(t.saturating_sub(fr), t).map(|p| format!("{p}%")),
                        )
                        .map(Node::volatile),
                    ),
            );
        }
    }
    let mut g = Node::group("Modules");
    for m in modules(ctx) {
        g.push(module_node(&m));
    }
    for d in ctx
        .smbios
        .memory_devices
        .iter()
        .filter(|d| !d.is_populated())
    {
        if let Some(loc) = &d.locator {
            g.push(Node::item(format!("Slot {loc}"), "Empty").status(Status::Note));
        }
    }
    s.push((!g.children.is_empty()).then_some(g));
    s
}

fn module_node(m: &Module) -> Node {
    let label = m.bank.as_ref().map_or_else(
        || format!("Slot {}", m.locator),
        |b| format!("Slot {} ({b})", m.locator),
    );
    let d = m.smbios.as_ref();
    let w = m.win32.as_ref();
    let mut n = Node::group(label);
    n.push(Node::item("Size", bytes_binary(m.size)));
    n.push(opt("Type", d.and_then(|d| d.memory_type)));
    n.push(opt(
        "Manufacturer",
        d.and_then(|d| d.manufacturer.clone())
            .or_else(|| w.and_then(|w| text(w, "Manufacturer"))),
    ));
    n.push(opt(
        "Part number",
        d.and_then(|d| d.part_number.clone())
            .or_else(|| w.and_then(|w| text(w, "PartNumber"))),
    ));
    n.push(
        opt(
            "Serial number",
            d.and_then(|d| d.serial.clone())
                .or_else(|| w.and_then(|w| text(w, "SerialNumber"))),
        )
        .map(Node::private),
    );
    n.push(opt(
        "Rated speed",
        d.and_then(|d| d.speed_mts.map(u64::from))
            .or_else(|| w.and_then(|w| num(w, "Speed")))
            .map(|v| format!("{v} MT/s")),
    ));
    n.push(opt(
        "Configured speed",
        d.and_then(|d| d.configured_speed_mts.map(u64::from))
            .or_else(|| w.and_then(|w| num(w, "ConfiguredClockSpeed")))
            .map(|v| format!("{v} MT/s")),
    ));
    n.push(opt(
        "Voltage",
        d.and_then(|d| d.configured_mv.map(u64::from))
            .or_else(|| {
                w.and_then(|w| num(w, "ConfiguredVoltage"))
                    .filter(|&v| v > 0)
            })
            .map(|mv| {
                format!(
                    "{:.2} V",
                    f64::from(u32::try_from(mv).unwrap_or(0)) / 1000.0
                )
            }),
    ));
    n.push(opt("Form factor", d.and_then(|d| d.form_factor)));
    n.push(opt("Ranks", d.and_then(|d| d.ranks).map(|r| r.to_string())));
    n
}

/// Builds the Motherboard section.
pub fn motherboard(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("motherboard", "Motherboard");
    let bb = ctx.first("baseBoard");
    let sb = ctx.smbios.baseboard.as_ref();
    let maker = bb
        .and_then(|b| text(b, "Manufacturer"))
        .or_else(|| sb.and_then(|b| b.manufacturer.clone()));
    let model = bb
        .and_then(|b| text(b, "Product"))
        .or_else(|| sb.and_then(|b| b.product.clone()));
    let socket = ctx
        .first("processor")
        .and_then(|p| text(p, "SocketDesignation"))
        .or_else(|| ctx.smbios.processors.first().and_then(|p| p.socket.clone()));
    s.push(
        opt(
            "summary-motherboard",
            join(
                &[
                    join(&[maker.clone(), model.clone()], " "),
                    socket.map(|x| format!("({x})")),
                ],
                " ",
            ),
        )
        .map(|n| Node {
            label: "Motherboard".to_owned(),
            ..n
        }),
    );
    s.push(opt("Manufacturer", maker));
    s.push(opt("Model", model));
    s.push(opt(
        "Version",
        bb.and_then(|b| text(b, "Version"))
            .or_else(|| sb.and_then(|b| b.version.clone())),
    ));
    s.push(
        opt(
            "Serial number",
            bb.and_then(|b| text(b, "SerialNumber"))
                .or_else(|| sb.and_then(|b| b.serial.clone())),
        )
        .map(Node::private),
    );
    if let Some(sys) = &ctx.smbios.system {
        s.push(opt(
            "System",
            join(&[sys.manufacturer.clone(), sys.product.clone()], " "),
        ));
        s.push(opt("System family", sys.family.clone()));
        s.push(opt("System serial number", sys.serial.clone()).map(Node::private));
    }
    s.push(bios_node(ctx));
    s.push(slots_node(ctx));
    s
}

/// The BIOS release date, from Win32 or SMBIOS.
pub fn bios_date(ctx: &Ctx<'_>) -> Option<Date> {
    ctx.first("bios")
        .and_then(|b| text(b, "ReleaseDate"))
        .or_else(|| {
            ctx.smbios
                .bios
                .as_ref()
                .and_then(|b| b.release_date.clone())
        })
        .and_then(|d| Date::parse(&d))
}

fn bios_node(ctx: &Ctx<'_>) -> Option<Node> {
    let b = ctx.first("bios");
    let sb = ctx.smbios.bios.as_ref();
    let mut g = Node::group("BIOS");
    g.push(opt(
        "Vendor",
        b.and_then(|b| text(b, "Manufacturer"))
            .or_else(|| sb.and_then(|b| b.vendor.clone())),
    ));
    g.push(opt(
        "Version",
        b.and_then(|b| text(b, "SMBIOSBIOSVersion"))
            .or_else(|| sb.and_then(|b| b.version.clone())),
    ));
    g.push(opt(
        "Date",
        date_with_age(ctx, bios_date(ctx).map(Date::iso).as_deref()),
    ));
    (!g.children.is_empty()).then_some(g)
}

fn slots_node(ctx: &Ctx<'_>) -> Option<Node> {
    let mut g = Node::group("Expansion slots");
    if ctx.smbios.slots.is_empty() {
        for (i, slot) in ctx.list("systemSlots").into_iter().enumerate() {
            let usage = match num(slot, "CurrentUsage") {
                Some(3) => SlotUsage::Available,
                Some(4) => SlotUsage::InUse,
                Some(5) => SlotUsage::Unavailable,
                _ => SlotUsage::Unknown,
            };
            let label = text(slot, "SlotDesignation").unwrap_or_else(|| format!("Slot {}", i + 1));
            g.push(Node::item(label, usage.label()).status(usage_status(usage)));
        }
    }
    for (i, slot) in ctx.smbios.slots.iter().enumerate() {
        let label = slot
            .designation
            .clone()
            .unwrap_or_else(|| format!("Slot {}", i + 1));
        let mut n = Node::item(label, slot.usage.label()).status(usage_status(slot.usage));
        n.push(Node::item("Type", slot.kind.clone()));
        n.push(opt("Lanes", slot.width.clone()));
        if !slot.characteristics.is_empty() {
            n.push(Node::item(
                "Characteristics",
                slot.characteristics.join(", "),
            ));
        }
        n.push(opt("Slot number", slot.id.map(|x| x.to_string())));
        g.push(n);
    }
    (!g.children.is_empty()).then_some(g)
}

const fn usage_status(u: SlotUsage) -> Status {
    match u {
        SlotUsage::Available => Status::Good,
        _ => Status::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_letters() {
        assert_eq!(channel_letter(Some("P0 CHANNEL A")), Some('A'));
        assert_eq!(channel_letter(Some("ChannelB-DIMM0")), Some('B'));
        assert_eq!(channel_letter(Some("DIMM_A1")), Some('A'));
        assert_eq!(channel_letter(Some("DIMM 1")), None);
        assert_eq!(channel_letter(Some("BANK 0")), None);
        assert_eq!(channel_letter(None), None);
    }

    #[test]
    fn power_plan_parsing() {
        assert_eq!(
            power_plan_name("Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)")
                .as_deref(),
            Some("Balanced")
        );
        assert_eq!(power_plan_name("nothing"), None);
    }

    #[test]
    fn antivirus_states() {
        assert_eq!(antivirus_state(0x61100), (true, true));
        assert_eq!(antivirus_state(0x60110), (false, false));
    }

    #[test]
    fn uptime_math() {
        assert_eq!(
            seconds_between("2026-11-15T17:27:03.0000000Z", "2026-11-15T19:04:44Z"),
            Some(5861)
        );
        assert_eq!(
            seconds_between("2026-11-15T19:00:00Z", "2026-11-15T18:00:00Z"),
            None
        );
    }
}
