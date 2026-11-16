//! Graphics, storage, optical, audio, peripherals, USB and network sections.

use super::{Ctx, join, num, text};
use crate::json::Value;
use crate::model::{Node, Section, Status, opt};
use crate::smart::{self, Verdict};
use crate::util::{self, bytes_binary, bytes_decimal};

/// Splits a PCI hardware ID such as
/// `PCI\VEN_10DE&DEV_1B82&SUBSYS_66733842&REV_A1\4&...` into its parts.
#[must_use]
pub fn pci_ids(pnp: &str) -> Option<(String, String, Option<String>, Option<String>)> {
    let upper = pnp.to_ascii_uppercase();
    let field = |name: &str| -> Option<String> {
        let start = upper.find(name)? + name.len();
        let rest = upper.get(start..)?;
        let end = rest.find(['&', '\\']).unwrap_or(rest.len());
        rest.get(..end).map(ToOwned::to_owned)
    };
    Some((
        field("VEN_")?,
        field("DEV_")?,
        field("SUBSYS_"),
        field("REV_"),
    ))
}

/// Splits a USB hardware ID such as `USB\VID_046D&PID_C52B\...` into
/// `046D:C52B`.
#[must_use]
pub fn usb_ids(pnp: &str) -> Option<String> {
    let upper = pnp.to_ascii_uppercase();
    let get = |name: &str| -> Option<String> {
        let start = upper.find(name)? + name.len();
        upper.get(start..start + 4).map(ToOwned::to_owned)
    };
    Some(format!("{}:{}", get("VID_")?, get("PID_")?))
}

/// Maps a PCI vendor ID (four hex digits) to a company name. Covers GPU
/// chip makers and common graphics card brands.
#[must_use]
pub fn pci_vendor(id: &str) -> Option<&'static str> {
    Some(match id.to_ascii_uppercase().as_str() {
        "10DE" => "NVIDIA",
        "1002" | "1022" => "AMD",
        "8086" => "Intel",
        "3842" => "EVGA",
        "1043" => "ASUS",
        "1462" => "MSI",
        "1458" => "Gigabyte",
        "19DA" => "Zotac",
        "1569" => "Palit",
        "1DA2" => "Sapphire",
        "1682" => "XFX",
        "148C" => "PowerColor",
        "1849" => "ASRock",
        "196E" => "PNY",
        "1B4C" => "Galax",
        "7377" => "Colorful",
        "1028" => "Dell",
        "103C" => "HP",
        "17AA" => "Lenovo",
        "1025" => "Acer",
        "144D" => "Samsung",
        _ => return None,
    })
}

/// Maps a three-letter EDID manufacturer code to a brand name.
#[must_use]
pub fn monitor_vendor(code: &str) -> Option<&'static str> {
    Some(match code.trim().to_ascii_uppercase().as_str() {
        "ACR" => "Acer",
        "AOC" => "AOC",
        "APP" => "Apple",
        "AUS" => "ASUS",
        "BNQ" => "BenQ",
        "DEL" => "Dell",
        "GBT" => "Gigabyte",
        "GSM" => "LG",
        "HWP" => "HP",
        "LEN" => "Lenovo",
        "MSI" => "MSI",
        "NEC" => "NEC",
        "PHL" => "Philips",
        "SAM" => "Samsung",
        "SNY" => "Sony",
        "VSC" => "ViewSonic",
        "AUO" => "AU Optronics",
        "BOE" => "BOE",
        "CMN" => "Innolux",
        "LGD" => "LG Display",
        "SDC" => "Samsung Display",
        "SHP" => "Sharp",
        _ => return None,
    })
}

/// The real video memory size: the registry's 64-bit value when present,
/// because `AdapterRAM` is a 32-bit field that caps at 4 GB.
fn video_memory(ctx: &Ctx<'_>, adapter: &Value) -> Option<u64> {
    let name = text(adapter, "Name")?;
    ctx.list("gpuMemory")
        .into_iter()
        .find(|g| text(g, "driverDesc").as_deref() == Some(name.as_str()))
        .and_then(|g| num(g, "memorySize"))
        .filter(|&m| m > 0)
        .or_else(|| num(adapter, "AdapterRAM").filter(|&m| m > 0))
}

fn display_mode(adapter: &Value) -> Option<String> {
    let w = num(adapter, "CurrentHorizontalResolution")?;
    let h = num(adapter, "CurrentVerticalResolution")?;
    let refresh = num(adapter, "CurrentRefreshRate")
        .filter(|&r| r > 1)
        .map_or_else(String::new, |r| format!(" @ {r} Hz"));
    let depth =
        num(adapter, "CurrentBitsPerPixel").map_or_else(String::new, |b| format!(", {b}-bit"));
    Some(format!("{w} \u{d7} {h}{refresh}{depth}"))
}

fn driver_date(ctx: &Ctx<'_>, raw: Option<&str>) -> Option<String> {
    let d = util::Date::parse(raw?)?;
    Some(ctx.today.map_or_else(
        || d.iso(),
        |t| format!("{} ({})", d.iso(), util::age_label(d, t)),
    ))
}

/// Builds the Graphics section.
pub fn graphics(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("graphics", "Graphics");
    let mut summary = Node::group("Graphics").with_key("summary-graphics");
    let monitors = ctx.list("monitors");
    // With one monitor, the active display mode can be attributed to it.
    let single_mode = if monitors.len() == 1 {
        ctx.list("videoControllers")
            .into_iter()
            .find_map(display_mode)
    } else {
        None
    };
    for monitor in monitors {
        if let Some(n) = monitor_node(ctx, monitor) {
            let name = n.value.clone().unwrap_or_else(|| n.label.clone());
            summary.push(Node::item(
                "Monitor",
                single_mode
                    .as_ref()
                    .map_or_else(|| name.clone(), |m| format!("{name} ({m})")),
            ));
            s.push(n);
        }
    }
    for a in ctx.list("videoControllers") {
        let Some(name) = text(a, "Name") else {
            continue;
        };
        let mem = video_memory(ctx, a);
        summary.push(Node::item(
            "GPU",
            join(&[Some(name.clone()), mem.map(bytes_binary)], ", ").unwrap_or_default(),
        ));
        let mut n = Node::group(name);
        n.push(opt("Vendor", text(a, "AdapterCompatibility")));
        n.push(opt("Video memory", mem.map(bytes_binary)));
        n.push(opt("Processor", text(a, "VideoProcessor")));
        if let Some((ven, dev, sub, rev)) = text(a, "PNPDeviceID").and_then(|p| pci_ids(&p)) {
            n.push(Node::item("Device ID", format!("{ven}-{dev}")));
            // SUBSYS_ddddvvvv: the low four hex digits name the card maker.
            let maker = sub.as_deref().and_then(|s| s.get(4..8)).map(|v| {
                pci_vendor(v).map_or_else(|| v.to_owned(), |name| format!("{name} ({v})"))
            });
            n.push(opt("Card maker", maker));
            n.push(opt("Subsystem ID", sub));
            n.push(opt("Revision", rev));
        }
        n.push(opt("Current mode", display_mode(a)));
        n.push(opt("Driver version", text(a, "DriverVersion")));
        n.push(opt(
            "Driver date",
            driver_date(ctx, text(a, "DriverDate").as_deref()),
        ));
        n.push(opt("Status", text(a, "Status")));
        s.push(n);
    }
    if !summary.children.is_empty() {
        s.nodes.insert(0, summary);
    }
    s
}

fn monitor_node(ctx: &Ctx<'_>, m: &Value) -> Option<Node> {
    let code = text(m, "manufacturer");
    let vendor = code.as_deref().and_then(monitor_vendor);
    let name =
        text(m, "name").or_else(|| text(m, "productCode").map(|p| format!("Display {p}")))?;
    let full = match vendor {
        Some(v)
            if !name
                .to_ascii_lowercase()
                .starts_with(&v.to_ascii_lowercase()) =>
        {
            format!("{v} {name}")
        }
        _ => name,
    };
    let mut n = Node::item("Monitor", full).with_key("monitor");
    n.push(opt(
        "Manufacturer",
        join(
            &[
                vendor.map(ToOwned::to_owned),
                code.map(|c| {
                    if vendor.is_some() {
                        format!("({c})")
                    } else {
                        c
                    }
                }),
            ],
            " ",
        ),
    ));
    n.push(opt("Product code", text(m, "productCode")));
    n.push(opt("Serial number", text(m, "serial")).map(Node::private));
    if let (Some(week), Some(year)) = (num(m, "week"), num(m, "year")) {
        n.push(Node::item(
            "Manufactured",
            if week > 0 && week <= 53 {
                format!("Week {week}, {year}")
            } else {
                year.to_string()
            },
        ));
    }
    let instance = text(m, "instanceName");
    let params = ctx
        .list("monitorParams")
        .into_iter()
        .find(|p| text(p, "InstanceName") == instance);
    if let Some(p) = params {
        if let (Some(w), Some(h)) = (
            num(p, "MaxHorizontalImageSize"),
            num(p, "MaxVerticalImageSize"),
        ) && w > 0
            && h > 0
        {
            #[allow(
                clippy::cast_precision_loss,
                reason = "monitor sizes are tiny integers"
            )]
            let diag = ((w * w + h * h) as f64).sqrt() / 2.54;
            n.push(Node::item(
                "Screen size",
                format!("{diag:.0} in ({w} \u{d7} {h} cm)"),
            ));
        }
        n.push(opt(
            "Input",
            num(p, "VideoInputType").map(|t| if t == 1 { "Digital" } else { "Analog" }),
        ));
    }
    Some(n)
}

fn physical_disk<'a>(ctx: &Ctx<'a>, index: u64) -> Option<&'a Value> {
    ctx.list("physicalDisks")
        .into_iter()
        .find(|p| text(p, "deviceId") == Some(index.to_string()))
}

fn reliability<'a>(ctx: &Ctx<'a>, index: u64) -> Option<&'a Value> {
    ctx.list("reliability")
        .into_iter()
        .find(|p| text(p, "deviceId") == Some(index.to_string()))
}

/// Normalises the `BusType` of `MSFT_PhysicalDisk`, which arrives as a name
/// or a numeric code.
#[must_use]
pub fn bus_type(v: &str) -> String {
    match v.trim() {
        "1" => "SCSI",
        "3" => "ATA",
        "6" => "Fibre Channel",
        "7" => "USB",
        "8" => "RAID",
        "9" => "iSCSI",
        "10" => "SAS",
        "11" => "SATA",
        "12" => "SD",
        "13" => "MMC",
        "15" => "File-backed virtual",
        "16" => "Storage Spaces",
        "17" => "NVMe",
        "0" => "Unknown",
        other => return other.to_owned(),
    }
    .to_owned()
}

/// Normalises the `MediaType` of `MSFT_PhysicalDisk`.
#[must_use]
pub fn media_type(v: &str) -> Option<String> {
    match v.trim() {
        "3" | "HDD" => Some("HDD".to_owned()),
        "4" | "SSD" => Some("SSD".to_owned()),
        "5" | "SCM" => Some("SCM".to_owned()),
        _ => None,
    }
}

fn disk_health_status(v: &str) -> Option<(Verdict, &'static str)> {
    match v.trim() {
        "0" | "Healthy" => Some((Verdict::Good, "Windows reports the drive as healthy")),
        "1" | "Warning" => Some((Verdict::Caution, "Windows reports a warning for this drive")),
        "2" | "Unhealthy" => Some((Verdict::Bad, "Windows reports the drive as unhealthy")),
        _ => None,
    }
}

/// SMART data for one disk, matched by its `PnP` device ID.
struct SmartFor {
    attributes: Vec<smart::Attribute>,
    predicts_failure: bool,
}

fn smart_for(ctx: &Ctx<'_>, pnp: Option<&str>) -> Option<SmartFor> {
    let pnp = pnp?.to_ascii_uppercase();
    let matches = |v: &Value| {
        text(v, "instanceName")
            .or_else(|| text(v, "InstanceName"))
            .is_some_and(|i| {
                let i = i.to_ascii_uppercase();
                let i = i.strip_suffix("_0").unwrap_or(&i);
                !i.is_empty() && (pnp.starts_with(i) || i.starts_with(&pnp))
            })
    };
    let data = ctx.list("smartData").into_iter().find(|v| matches(v))?;
    let page = crate::base64::decode(data.get("data").as_str()?)?;
    let thresholds = ctx
        .list("smartThresholds")
        .into_iter()
        .find(|v| matches(v))
        .and_then(|v| crate::base64::decode(v.get("data").as_str()?));
    let predicts_failure = ctx
        .list("smartStatus")
        .into_iter()
        .find(|v| matches(v))
        .and_then(|v| v.get("PredictFailure").as_bool())
        .unwrap_or(false);
    Some(SmartFor {
        attributes: smart::parse(&page, thresholds.as_deref()),
        predicts_failure,
    })
}

/// The health verdict for a disk and the reasons, or why it is unknown.
pub fn disk_health(ctx: &Ctx<'_>, disk: &Value) -> (Option<Verdict>, Vec<String>) {
    let index = num(disk, "Index");
    let smart = smart_for(ctx, text(disk, "PNPDeviceID").as_deref());
    let (mut verdict, mut reasons) = smart.as_ref().map_or((None, Vec::new()), |sm| {
        let h = smart::assess(&sm.attributes, sm.predicts_failure);
        (Some(h.verdict), h.reasons)
    });
    if let Some((v, why)) = index
        .and_then(|i| physical_disk(ctx, i))
        .and_then(|p| text(p, "healthStatus"))
        .and_then(|h| disk_health_status(&h))
        && v != Verdict::Good
    {
        reasons.push(why.to_owned());
        verdict = Some(verdict.map_or(v, |cur| cur.max(v)));
    } else if verdict.is_none()
        && let Some((v, _)) = index
            .and_then(|i| physical_disk(ctx, i))
            .and_then(|p| text(p, "healthStatus"))
            .and_then(|h| disk_health_status(&h))
    {
        verdict = Some(v);
    }
    if let Some(r) = index.and_then(|i| reliability(ctx, i)) {
        let errors = num(r, "readErrorsUncorrected").unwrap_or(0)
            + num(r, "writeErrorsUncorrected").unwrap_or(0);
        if errors > 0 {
            reasons.push(format!("{errors} uncorrected read/write errors"));
            verdict = Some(verdict.map_or(Verdict::Caution, |cur| cur.max(Verdict::Caution)));
        }
        if let Some(wear) = num(r, "wear")
            && wear >= 90
        {
            reasons.push(format!("{wear}% of rated endurance used"));
            verdict = Some(verdict.map_or(Verdict::Caution, |cur| cur.max(Verdict::Caution)));
        }
    }
    (verdict, reasons)
}

const fn verdict_status(v: Verdict) -> Status {
    match v {
        Verdict::Good => Status::Good,
        Verdict::Caution => Status::Caution,
        Verdict::Bad => Status::Bad,
    }
}

/// Builds the Storage section.
pub fn storage(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("storage", "Storage");
    let mut disks = ctx.list("diskDrives");
    disks.sort_by_key(|d| num(d, "Index").unwrap_or(u64::MAX));
    let mut summary = Node::group("Storage").with_key("summary-storage");
    for d in disks {
        let model = text(d, "Model").unwrap_or_else(|| "Disk".to_owned());
        let index = num(d, "Index");
        let pd = index.and_then(|i| physical_disk(ctx, i));
        let bus = pd
            .and_then(|p| text(p, "busType"))
            .map(|b| bus_type(&b))
            .or_else(|| text(d, "InterfaceType"));
        let media = pd
            .and_then(|p| text(p, "mediaType"))
            .and_then(|m| media_type(&m));
        let size = num(d, "Size").map(bytes_decimal);
        let (verdict, reasons) = disk_health(ctx, d);
        let kind = join(&[bus.clone(), media.clone()], " ");

        let mut line = Node::item(
            "Drive",
            join(
                &[
                    size.clone(),
                    Some(model.clone()),
                    kind.clone().map(|k| format!("({k})")),
                    verdict.map(|v| format!("\u{2014} {}", v.label())),
                ],
                " ",
            )
            .unwrap_or_default(),
        );
        if let Some(v) = verdict {
            line = line.status(verdict_status(v));
        }
        summary.push(line);

        let mut n = Node::group(model.clone()).with_key(format!("disk-{}", index.unwrap_or(0)));
        n.push(opt("Capacity", size));
        n.push(opt(
            "Exact size",
            num(d, "Size").map(|b| format!("{b} bytes")),
        ));
        n.push(opt("Interface", bus));
        n.push(opt("Type", media));
        n.push(opt(
            "Speed",
            pd.and_then(|p| num(p, "spindleSpeed"))
                .filter(|&r| r > 0 && r < 100_000)
                .map(|r| format!("{r} RPM")),
        ));
        n.push(opt(
            "Firmware",
            text(d, "FirmwareRevision").or_else(|| pd.and_then(|p| text(p, "firmwareVersion"))),
        ));
        n.push(opt("Serial number", text(d, "SerialNumber")).map(Node::private));
        n.push(health_node(verdict, &reasons, ctx));
        n.push(reliability_node(ctx, index, d));
        n.push(smart_node(ctx, d));
        n.push(partitions_node(ctx, index));
        s.push(n);
    }
    if !summary.children.is_empty() {
        s.nodes.insert(0, summary);
    }
    s
}

fn health_node(verdict: Option<Verdict>, reasons: &[String], ctx: &Ctx<'_>) -> Node {
    verdict.map_or_else(
        || {
            let why = if ctx.is_admin() {
                "Unknown (the drive does not report SMART data)"
            } else {
                "Unknown (run as administrator to read SMART data)"
            };
            Node::item("Health", why).status(Status::Note)
        },
        |v| {
            let value = if reasons.is_empty() {
                v.label().to_owned()
            } else {
                format!("{}: {}", v.label(), reasons.join("; "))
            };
            Node::item("Health", value).status(verdict_status(v))
        },
    )
}

fn reliability_node(ctx: &Ctx<'_>, index: Option<u64>, disk: &Value) -> Option<Node> {
    let r = index.and_then(|i| reliability(ctx, i));
    let sm = smart_for(ctx, text(disk, "PNPDeviceID").as_deref());
    let attr = |id: u8| {
        sm.as_ref()
            .and_then(|s| s.attributes.iter().find(|a| a.id == id).map(|a| a.raw))
    };
    let mut g = Node::group("Usage");
    let temp = r
        .and_then(|r| num(r, "temperature"))
        .filter(|&t| t > 0)
        .or_else(|| attr(0xC2).map(|v| v & 0xFF));
    g.push(opt("Temperature", temp.map(|t| format!("{t} \u{b0}C"))).map(Node::volatile));
    let hours = r
        .and_then(|r| num(r, "powerOnHours"))
        .or_else(|| attr(0x09).map(|v| v & 0xFFFF_FFFF));
    g.push(
        opt(
            "Power-on time",
            hours.map(|h| format!("{h} h ({:.1} years)", hours_to_years(h))),
        )
        .map(Node::volatile),
    );
    g.push(opt("Power cycles", attr(0x0C).map(|v| v.to_string())).map(Node::volatile));
    g.push(
        opt(
            "Endurance used",
            r.and_then(|r| num(r, "wear")).map(|w| format!("{w}%")),
        )
        .map(Node::volatile),
    );
    (!g.children.is_empty()).then_some(g)
}

#[allow(clippy::cast_precision_loss, reason = "display rounding only")]
fn hours_to_years(h: u64) -> f64 {
    h as f64 / 8766.0
}

fn smart_node(ctx: &Ctx<'_>, disk: &Value) -> Option<Node> {
    let sm = smart_for(ctx, text(disk, "PNPDeviceID").as_deref())?;
    if sm.attributes.is_empty() {
        return None;
    }
    let mut g = Node::item("SMART attributes", sm.attributes.len().to_string());
    for a in &sm.attributes {
        let threshold = a
            .threshold
            .map_or_else(|| "\u{2013}".to_owned(), |t| t.to_string());
        let n = Node::item(
            format!("{:02X} {}", a.id, a.name()),
            format!(
                "raw {} \u{b7} value {} \u{b7} worst {} \u{b7} threshold {threshold}",
                a.display_raw(),
                a.current,
                a.worst
            ),
        )
        .with_key(format!("attr-{:02x}", a.id))
        .volatile()
        .status(if a.is_failing() {
            Status::Bad
        } else {
            Status::Normal
        });
        g.push(n);
    }
    Some(g)
}

fn partitions_node(ctx: &Ctx<'_>, index: Option<u64>) -> Option<Node> {
    let index = index?;
    let mut g = Node::group("Partitions");
    for p in ctx
        .list("partitions")
        .into_iter()
        .filter(|p| num(p, "diskNumber") == Some(index))
    {
        let number = num(p, "partitionNumber").unwrap_or(0);
        let letter = text(p, "driveLetter");
        let label = letter.as_ref().map_or_else(
            || format!("Partition {number}"),
            |l| format!("Partition {number} ({l}:)"),
        );
        let mut n = Node::group(label).with_key(format!("partition-{number}"));
        n.push(opt("Type", text(p, "type")));
        n.push(opt("Size", num(p, "size").map(bytes_decimal)));
        if let Some(l) = &letter
            && let Some(ld) = ctx
                .list("logicalDisks")
                .into_iter()
                .find(|d| text(d, "DeviceID") == Some(format!("{l}:")))
        {
            n.push(opt("File system", text(ld, "FileSystem")));
            n.push(opt("Label", text(ld, "VolumeName")));
            if let (Some(size), Some(free)) = (num(ld, "Size"), num(ld, "FreeSpace")) {
                let pct =
                    util::percent(free, size).map_or_else(String::new, |p| format!(" ({p}%)"));
                n.push(
                    Node::item("Free space", format!("{}{pct}", bytes_decimal(free))).volatile(),
                );
            }
            n.push(opt("Volume serial", text(ld, "VolumeSerialNumber")).map(Node::private));
        }
        g.push(n);
    }
    (!g.children.is_empty()).then_some(g)
}

/// Builds the Optical Drives section.
pub fn optical(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("optical", "Optical Drives");
    let mut names = Vec::new();
    for d in ctx.list("cdrom") {
        let Some(name) = text(d, "Name") else {
            continue;
        };
        names.push(name.clone());
        let mut n = Node::group(name);
        n.push(opt("Drive letter", text(d, "Drive")));
        n.push(opt("Type", text(d, "MediaType")));
        n.push(
            opt(
                "Media loaded",
                d.get("MediaLoaded")
                    .as_bool()
                    .map(|b| if b { "Yes" } else { "No" }),
            )
            .map(Node::volatile),
        );
        let caps: Vec<String> = d
            .get("CapabilityDescriptions")
            .items()
            .iter()
            .filter_map(|c| c.as_text())
            .collect();
        if !caps.is_empty() {
            n.push(Node::item("Capabilities", caps.join(", ")));
        }
        if let (Some(bus), Some(target), Some(lun)) = (
            num(d, "SCSIBus"),
            num(d, "SCSITargetId"),
            num(d, "SCSILogicalUnit"),
        ) {
            n.push(Node::item(
                "SCSI address",
                format!("bus {bus}, target {target}, LUN {lun}"),
            ));
        }
        n.push(opt("Status", text(d, "Status")));
        s.push(n);
    }
    if !names.is_empty() {
        s.nodes.insert(
            0,
            Node::item("Optical", names.join(", ")).with_key("summary-optical"),
        );
    }
    s
}

/// Builds the Audio section.
pub fn audio(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("audio", "Audio");
    let mut cards = Node::group("Sound devices");
    let mut names = Vec::new();
    for d in ctx.list("soundDevices") {
        if let Some(name) = text(d, "Name") {
            names.push(name.clone());
            cards.push(Node::item(
                name,
                join(&[text(d, "Manufacturer"), text(d, "Status")], ", ").unwrap_or_default(),
            ));
        }
    }
    let mut endpoints = Node::group("Playback and recording endpoints");
    for e in ctx.list("audioEndpoints") {
        if let Some(name) = text(e, "FriendlyName") {
            endpoints.push(Node::item(name, text(e, "Status").unwrap_or_default()));
        }
    }
    if !names.is_empty() {
        s.push(Node::item("Audio", names.join(", ")).with_key("summary-audio"));
    }
    s.push((!cards.children.is_empty()).then_some(cards));
    s.push((!endpoints.children.is_empty()).then_some(endpoints));
    s
}

/// Builds the Peripherals section.
pub fn peripherals(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("peripherals", "Peripherals");
    let mut kb = Node::group("Keyboards");
    for k in ctx.list("keyboards") {
        if let Some(name) = text(k, "Name").or_else(|| text(k, "Description")) {
            kb.push(Node::item(
                name,
                connection(text(k, "PNPDeviceID").as_deref()),
            ));
        }
    }
    let mut mice = Node::group("Pointing devices");
    for m in ctx.list("pointingDevices") {
        if let Some(name) = text(m, "Name") {
            mice.push(Node::item(
                name,
                join(
                    &[
                        text(m, "Manufacturer"),
                        Some(connection(text(m, "PNPDeviceID").as_deref())),
                    ],
                    ", ",
                )
                .unwrap_or_default(),
            ));
        }
    }
    let mut printers = Node::group("Printers");
    for p in ctx.list("printers") {
        let Some(name) = text(p, "Name") else {
            continue;
        };
        let default = p.get("Default").as_bool() == Some(true);
        let mut n = Node::group(if default {
            format!("{name} (default)")
        } else {
            name
        });
        n.push(opt("Driver", text(p, "DriverName")));
        n.push(opt("Port", text(p, "PortName")).map(Node::private));
        n.push(opt(
            "Network printer",
            p.get("Network")
                .as_bool()
                .map(|b| if b { "Yes" } else { "No" }),
        ));
        n.push(opt(
            "Shared",
            p.get("Shared")
                .as_bool()
                .map(|b| if b { "Yes" } else { "No" }),
        ));
        printers.push(n);
    }
    for g in [kb, mice, printers] {
        s.push((!g.children.is_empty()).then_some(g));
    }
    s
}

/// Describes how a device connects, from its `PnP` device ID prefix.
#[must_use]
pub fn connection(pnp: Option<&str>) -> String {
    let p = pnp.unwrap_or("").to_ascii_uppercase();
    if p.contains("BTHLE") || p.contains("{00001812") {
        "Bluetooth LE".to_owned()
    } else if p.starts_with("BTH") {
        "Bluetooth".to_owned()
    } else if p.starts_with("HID\\VID_") || p.starts_with("USB") {
        "USB".to_owned()
    } else if p.starts_with("ACPI") {
        "Built-in (PS/2 or ACPI)".to_owned()
    } else if p.starts_with("HID") {
        "HID".to_owned()
    } else {
        "Unknown connection".to_owned()
    }
}

/// Builds the USB section.
pub fn usb(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("usb", "USB");
    let mut controllers = Node::group("Controllers");
    for c in ctx.list("usbControllers") {
        if let Some(name) = text(c, "Name") {
            controllers.push(Node::item(
                name,
                text(c, "Manufacturer").unwrap_or_default(),
            ));
        }
    }
    s.push((!controllers.children.is_empty()).then_some(controllers));
    let mut by_class: Vec<(String, Node)> = Vec::new();
    for d in ctx.list("usbDevices") {
        let Some(name) = text(d, "Name") else {
            continue;
        };
        let class = text(d, "PNPClass").unwrap_or_else(|| "Other".to_owned());
        let ids = text(d, "PNPDeviceID").and_then(|p| usb_ids(&p));
        let value = join(
            &[
                text(d, "Manufacturer"),
                ids.map(|i| format!("ID {i}")),
                text(d, "Status").filter(|s| s != "OK"),
            ],
            ", ",
        )
        .unwrap_or_default();
        let node = Node::item(name, value);
        if let Some((_, g)) = by_class.iter_mut().find(|(c, _)| *c == class) {
            g.push(node);
        } else {
            by_class.push((class.clone(), Node::group(class).with(node)));
        }
    }
    by_class.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, mut g) in by_class {
        g.value = Some(g.children.len().to_string());
        s.push(g);
    }
    s
}

/// Parses `netsh wlan show interfaces` output into known fields.
#[must_use]
pub fn parse_wlan(output: &str) -> Vec<(String, String)> {
    const KEEP: [&str; 9] = [
        "SSID",
        "State",
        "Radio type",
        "Band",
        "Channel",
        "Authentication",
        "Receive rate (Mbps)",
        "Transmit rate (Mbps)",
        "Signal",
    ];
    output
        .lines()
        .filter_map(|line| {
            let (k, v) = line.split_once(" : ")?;
            let k = k.trim();
            let v = v.trim();
            (KEEP.contains(&k) && !v.is_empty()).then(|| (k.to_owned(), v.to_owned()))
        })
        .collect()
}

fn wifi_node(ctx: &Ctx<'_>) -> Option<Node> {
    let wlan = ctx
        .raw
        .get("wlanInterfaces")
        .as_str()
        .map(parse_wlan)
        .unwrap_or_default();
    if wlan.is_empty() {
        return None;
    }
    let mut g = Node::group("Wi-Fi");
    for (k, v) in wlan {
        let mut n = Node::item(k.clone(), v);
        if k == "SSID" {
            n = n.private();
        }
        if matches!(
            k.as_str(),
            "Signal" | "Receive rate (Mbps)" | "Transmit rate (Mbps)" | "State" | "Channel"
        ) {
            n = n.volatile();
        }
        g.push(n);
    }
    Some(g)
}

/// Builds the Network section.
pub fn network(ctx: &Ctx<'_>) -> Section {
    let mut s = Section::new("network", "Network");
    let mut connected = Vec::new();
    for ip in ctx.list("ipConfig") {
        let Some(desc) = text(ip, "Description") else {
            continue;
        };
        connected.push(desc.clone());
        let list = |key: &str| {
            let items: Vec<String> = ip
                .get(key)
                .items()
                .iter()
                .filter_map(|v| v.as_text())
                .collect();
            (!items.is_empty()).then(|| items.join(", "))
        };
        let mut n = Node::group(format!("Connection: {desc}"));
        n.push(opt("IP addresses", list("IPAddress")).map(Node::private));
        n.push(opt("Subnet masks", list("IPSubnet")));
        n.push(opt("Gateway", list("DefaultIPGateway")).map(Node::private));
        n.push(opt("DNS servers", list("DNSServerSearchOrder")).map(Node::private));
        n.push(opt(
            "DHCP",
            ip.get("DHCPEnabled")
                .as_bool()
                .map(|b| if b { "Enabled" } else { "Disabled" }),
        ));
        n.push(opt("DHCP server", text(ip, "DHCPServer")).map(Node::private));
        s.push(n);
    }
    if let Some(first) = connected.first() {
        s.nodes.insert(
            0,
            Node::item("Network", first.clone()).with_key("summary-network"),
        );
    }
    let mut adapters = Node::group("Adapters");
    for a in ctx.list("netAdapters") {
        let Some(desc) = text(a, "InterfaceDescription") else {
            continue;
        };
        let mut n = Node::group(desc);
        n.push(opt("Connection name", text(a, "Name")));
        n.push(opt("Status", text(a, "Status")).map(Node::volatile));
        n.push(opt("Link speed", text(a, "LinkSpeed")).map(Node::volatile));
        n.push(opt(
            "Media",
            text(a, "PhysicalMediaType").or_else(|| text(a, "MediaType")),
        ));
        n.push(opt("MAC address", text(a, "MacAddress")).map(Node::private));
        n.push(opt("Driver version", text(a, "DriverVersion")));
        n.push(opt(
            "Virtual",
            a.get("Virtual")
                .as_bool()
                .map(|b| if b { "Yes" } else { "No" }),
        ));
        adapters.push(n);
    }
    s.push((!adapters.children.is_empty()).then_some(adapters));
    s.push(wifi_node(ctx));
    let rdp = ctx.raw.path(&["remoteDesktop", "denyConnections"]).as_u64();
    s.push(opt(
        "Remote Desktop",
        rdp.map(|d| if d == 0 { "Enabled" } else { "Disabled" }),
    ));
    let proxy = ctx.raw.get("proxy");
    if proxy.get("enabled").as_u64() == Some(1) {
        s.push(opt("Proxy", text(proxy, "server")).map(Node::private));
    } else if proxy.get("enabled").as_u64() == Some(0) {
        s.push(Node::item("Proxy", "None"));
    }
    s.push(opt("Proxy auto-config", text(proxy, "autoConfigUrl")).map(Node::private));
    let mut shares = Node::group("Shared folders");
    for sh in ctx.list("shares") {
        if let Some(name) = text(sh, "Name")
            && !name.ends_with('$')
        {
            shares.push(Node::item(name, text(sh, "Path").unwrap_or_default()).private());
        }
    }
    s.push((!shares.children.is_empty()).then_some(shares));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_and_usb_ids() {
        let (v, d, s, r) =
            pci_ids("PCI\\VEN_10DE&DEV_1B82&SUBSYS_66733842&REV_A1\\4&2283F625&0&0019").unwrap();
        assert_eq!((v.as_str(), d.as_str()), ("10DE", "1B82"));
        assert_eq!(s.as_deref(), Some("66733842"));
        assert_eq!(r.as_deref(), Some("A1"));
        assert!(pci_ids("ROOT\\BasicDisplay\\0000").is_none());
        assert_eq!(
            usb_ids("USB\\VID_046D&PID_C52B\\5&1").as_deref(),
            Some("046D:C52B")
        );
    }

    #[test]
    fn bus_and_media_types() {
        assert_eq!(bus_type("17"), "NVMe");
        assert_eq!(bus_type("SATA"), "SATA");
        assert_eq!(media_type("4").as_deref(), Some("SSD"));
        assert_eq!(media_type("Unspecified"), None);
    }

    #[test]
    fn wlan_parsing() {
        let out = "    Name                   : Wi-Fi 2\r\n    SSID                   : Home\r\n    BSSID                  : aa:bb\r\n    Signal                 : 98%\r\n";
        assert_eq!(
            parse_wlan(out),
            vec![
                ("SSID".into(), "Home".into()),
                ("Signal".into(), "98%".into())
            ]
        );
    }

    #[test]
    fn connections() {
        assert_eq!(
            connection(Some("HID\\{00001812-0000-1000-8000-00805F9B34FB}_DEV")),
            "Bluetooth LE"
        );
        assert_eq!(connection(Some("HID\\VID_046D&PID_C52B")), "USB");
        assert_eq!(connection(None), "Unknown connection");
    }

    #[test]
    fn monitor_vendors() {
        assert_eq!(monitor_vendor("GSM"), Some("LG"));
        assert_eq!(monitor_vendor("ZZZ"), None);
        assert_eq!(pci_vendor("3842"), Some("EVGA"));
    }
}
