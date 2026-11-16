//! Builder tests against fixture snapshots of fictional machines.
//!
//! Golden files in `tests/golden/` hold the expected text and JSON output.
//! After an intended change, regenerate them with
//! `UPDATE_GOLDEN=1 cargo test` and review the diff.

use super::*;
use crate::model::Status;
use crate::render::{Format, render};

fn fixture(name: &str) -> Report {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    build(&Snapshot::load(std::path::Path::new(&path)).unwrap())
}

fn find<'a>(r: &'a Report, section: &str, path: &[&str]) -> &'a Node {
    let mut nodes = &r
        .section(section)
        .unwrap_or_else(|| panic!("no section {section}"))
        .nodes;
    let mut found = None;
    for label in path {
        let n = nodes.iter().find(|n| n.label == *label).unwrap_or_else(|| {
            panic!(
                "no node {label} in {section}: {:?}",
                nodes.iter().map(|n| &n.label).collect::<Vec<_>>()
            )
        });
        nodes = &n.children;
        found = Some(n);
    }
    found.unwrap()
}

fn value<'a>(r: &'a Report, section: &str, path: &[&str]) -> &'a str {
    find(r, section, path).value.as_deref().unwrap_or("")
}

#[test]
fn desktop_summary() {
    let r = fixture("desktop");
    assert_eq!(
        value(&r, "summary", &["Operating system"]),
        "Windows 11 Pro 64-bit (24H2, build 26100.7019)"
    );
    assert_eq!(
        value(&r, "summary", &["CPU"]),
        "AMD Ryzen 5 3600 6-Core Processor (Matisse, 7 nm) \u{2014} 6 cores / 12 threads"
    );
    assert_eq!(
        value(&r, "summary", &["RAM"]),
        "48 GB DDR4 dual-channel @ 2133 MT/s (1066 MHz) (4 of 4 slots used)"
    );
    assert_eq!(
        value(&r, "summary", &["Motherboard"]),
        "Gigabyte Technology Co., Ltd. B550 EAGLE WIFI6 (AM4)"
    );
    let graphics = find(&r, "summary", &["Graphics"]);
    assert_eq!(
        graphics.children[0].value.as_deref(),
        Some("LG HDR WQHD (3840 \u{d7} 1600 @ 60 Hz, 32-bit)")
    );
    assert_eq!(
        graphics.children[1].value.as_deref(),
        Some("NVIDIA GeForce GTX 1070 Ti, 8 GB")
    );
    let storage = find(&r, "summary", &["Storage"]);
    assert_eq!(storage.children.len(), 3);
    assert_eq!(
        storage.children[0].value.as_deref(),
        Some("4 TB WDC WD40EZRZ-00GXCB0 (SATA HDD) \u{2014} Good")
    );
    assert_eq!(
        storage.children[1].status,
        Status::Caution,
        "16 reallocation events on the SSD"
    );
}

#[test]
fn desktop_sections_present() {
    let r = fixture("desktop");
    let ids: Vec<_> = r.sections.iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec![
            "summary",
            "os",
            "cpu",
            "ram",
            "motherboard",
            "graphics",
            "storage",
            "optical",
            "audio",
            "peripherals",
            "usb",
            "network",
            "upgrade",
            "problems",
            "drivers",
            "windows11",
            "collection"
        ]
    );
    assert_eq!(ids.len(), SECTIONS.len());
    for (id, title) in SECTIONS {
        assert_eq!(r.section(id).unwrap().title, title);
    }
    assert!(
        r.notes.is_empty(),
        "admin fixture has no notes: {:?}",
        r.notes
    );
}

#[test]
fn desktop_details() {
    let r = fixture("desktop");
    assert_eq!(
        value(&r, "cpu", &["Virtualization"]),
        "AMD-V, in use by the Windows hypervisor"
    );
    assert_eq!(
        value(&r, "cpu", &["Caches", "L1 data"]),
        "6 \u{d7} 32 KB, 8-way"
    );
    assert_eq!(
        value(&r, "cpu", &["Caches", "L3 unified"]),
        "2 \u{d7} 16 MB, 16-way"
    );
    assert_eq!(value(&r, "ram", &["Memory slots", "Free"]), "0");
    assert_eq!(find(&r, "ram", &["Rated speed"]).status, Status::Caution);
    assert_eq!(
        value(&r, "motherboard", &["BIOS", "Date"]),
        "2024-11-18 (1 year old)"
    );
    assert_eq!(
        value(
            &r,
            "motherboard",
            &["Expansion slots", "J3708 PCIE x4 slot from Promontory"]
        ),
        "Available"
    );
    assert_eq!(
        value(&r, "graphics", &["NVIDIA GeForce GTX 1070 Ti", "Device ID"]),
        "10DE-1B82"
    );
    assert_eq!(
        value(
            &r,
            "graphics",
            &["NVIDIA GeForce GTX 1070 Ti", "Card maker"]
        ),
        "EVGA (3842)"
    );
    assert_eq!(
        value(
            &r,
            "ram",
            &["Modules", "Slot DIMM 1 (P0 CHANNEL B)", "Size"]
        ),
        "16 GB"
    );
    assert_eq!(
        value(
            &r,
            "ram",
            &["Modules", "Slot DIMM 1 (P0 CHANNEL B)", "Part number"]
        ),
        "F4-3200C16-16GVK"
    );
    assert_eq!(
        value(&r, "graphics", &["Monitor", "Screen size"]),
        "34 in (80 \u{d7} 34 cm)"
    );
    let hdd = find(&r, "storage", &["WDC WD40EZRZ-00GXCB0"]);
    assert_eq!(
        hdd.children
            .iter()
            .find(|n| n.label == "Health")
            .unwrap()
            .value
            .as_deref(),
        Some("Good")
    );
    let ssd_health = find(&r, "storage", &["Crucial_CT512M550SSD1", "Health"]);
    assert_eq!(
        ssd_health.value.as_deref(),
        Some("Caution: 16 reallocation events")
    );
    assert_eq!(
        value(&r, "storage", &["TEAM TM8FP6002T", "Usage", "Temperature"]),
        "38 \u{b0}C"
    );
    assert_eq!(
        value(
            &r,
            "storage",
            &[
                "WDC WD40EZRZ-00GXCB0",
                "Partitions",
                "Partition 2 (G:)",
                "Label"
            ]
        ),
        "Archive"
    );
    assert_eq!(value(&r, "network", &["Wi-Fi", "Radio type"]), "802.11ax");
    assert!(find(&r, "network", &["Wi-Fi", "SSID"]).private);
}

#[test]
fn desktop_insights() {
    let r = fixture("desktop");
    assert!(
        value(&r, "upgrade", &["Memory", "You can"])
            .starts_with("All slots are full. To go past 48 GB")
    );
    assert!(
        value(&r, "upgrade", &["Memory", "Free speed"])
            .contains("rated for 3200 MT/s but run at 2133 MT/s")
    );
    assert_eq!(
        value(
            &r,
            "upgrade",
            &[
                "Expansion slots",
                "Free: J3708 PCIE x4 slot from Promontory"
            ]
        ),
        "PCIe, x4 lanes"
    );
    assert_eq!(
        find(&r, "upgrade", &["Storage", "Replace first"]).status,
        Status::Caution
    );
    assert_eq!(value(&r, "problems", &["Devices with problems"]), "1");
    assert_eq!(
        value(&r, "problems", &["Base System Device"]),
        "Code 28: Drivers are not installed"
    );
    assert_eq!(value(&r, "problems", &["Not connected"]), "1");
    let old = find(&r, "drivers", &["Drivers older than 3 years"]);
    assert_eq!(old.value.as_deref(), Some("2"));
    assert!(old.children.iter().all(|n| n.status == Status::Caution));
    assert!(
        !find(&r, "drivers", &["Third-party drivers"])
            .children
            .iter()
            .any(|g| g.children.iter().any(|n| n.label.contains("AHCI"))),
        "Microsoft inbox drivers are skipped"
    );
    assert_eq!(
        value(&r, "windows11", &["Result"]),
        "Already running Windows 11"
    );
    assert_eq!(find(&r, "windows11", &["TPM"]).status, Status::Good);
    assert_eq!(
        value(&r, "windows11", &["Secure Boot 2023 certificates"]),
        "Updated"
    );
}

#[test]
fn private_values_are_flagged() {
    let r = fixture("desktop");
    let red = render(&r.redacted(), Format::Json);
    for secret in [
        "WD-WCC7K0000001",
        "EXAMPLE-PC",
        "192.168.0.197",
        "AC-36-1B-00-00-01",
        "ExampleNet",
        "112NTAB12345",
        "00330-80000",
    ] {
        assert!(
            !red.contains(secret),
            "{secret} leaked into redacted output"
        );
        assert!(
            render(&r, Format::Json).contains(secret),
            "{secret} missing from full output"
        );
    }
}

#[test]
fn laptop_without_admin() {
    let r = fixture("laptop");
    assert!(
        r.notes
            .iter()
            .any(|n| n.contains("Not running as administrator"))
    );
    assert_eq!(
        value(&r, "storage", &["SAMSUNG MZVLW256", "Interface"]),
        "NVMe"
    );
    assert_eq!(
        value(&r, "storage", &["SAMSUNG MZVLW256", "Health"]),
        "Good"
    );
    assert_eq!(value(&r, "ram", &["Memory slots", "Free"]), "1");
    assert_eq!(value(&r, "ram", &["Channels (estimated)"]), "Single");
    assert!(
        value(&r, "upgrade", &["Memory", "You can"]).starts_with("Add up to 1 more DDR4 module.")
    );
    assert_eq!(
        value(&r, "windows11", &["TPM"]),
        "Unknown (run as administrator)"
    );
    assert_eq!(find(&r, "windows11", &["Firmware"]).status, Status::Bad);
    assert_eq!(
        value(&r, "windows11", &["Result"]),
        "Does not meet Windows 11 requirements"
    );
    assert_eq!(
        value(&r, "problems", &["Result"]),
        "No devices report problems"
    );
    assert_eq!(value(&r, "collection", &["Queries that failed"]), "3");
}

#[test]
fn empty_snapshot_builds_without_panicking() {
    let r = build(&Snapshot::from_json(r#"{"schema":1}"#).unwrap());
    assert!(r.section("summary").is_none());
    let junk = build(
        &Snapshot::from_json(
            r#"{"schema":1,"os":"x","diskDrives":[1,2],"smbios":{"data":"!!"},"cpuid":[[1]]}"#,
        )
        .unwrap(),
    );
    let _ = render(&junk, Format::Html);
}

fn golden(name: &str, format: Format, ext: &str) {
    let report = fixture(name);
    let actual = render(&report, format);
    let path = format!("{}/tests/golden/{name}.{ext}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing {path}; run UPDATE_GOLDEN=1 cargo test"))
        .replace("\r\n", "\n");
    assert!(
        expected == actual,
        "{path} is out of date; run UPDATE_GOLDEN=1 cargo test and review the diff"
    );
}

#[test]
fn golden_desktop_text() {
    golden("desktop", Format::Text, "txt");
}

#[test]
fn golden_desktop_json() {
    golden("desktop", Format::Json, "json");
}

#[test]
fn golden_laptop_text() {
    golden("laptop", Format::Text, "txt");
}

#[test]
fn virtual_machine_placeholders_are_hidden() {
    let snap = Snapshot::from_json(
        r#"{"schema":1,
            "physicalMemory":[{"DeviceLocator":"M0001","Capacity":8589934592,"Speed":0,"ConfiguredClockSpeed":0}],
            "videoControllers":[{"Name":"Microsoft Hyper-V Video","CurrentHorizontalResolution":1024,
                                 "CurrentVerticalResolution":768,"CurrentRefreshRate":1}]}"#,
    )
    .unwrap();
    let r = build(&snap);
    assert_eq!(value(&r, "summary", &["RAM"]), "8 GB");
    assert_eq!(
        value(&r, "graphics", &["Microsoft Hyper-V Video", "Current mode"]),
        "1024 \u{d7} 768"
    );
}
