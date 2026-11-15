//! A parser for raw SMBIOS structure tables.
//!
//! On Windows the raw table comes from `MSSmBios_RawSMBiosTables`. Only the
//! structure types this tool reports on are decoded: BIOS (0), system (1),
//! baseboard (2), processor (4), system slots (9), physical memory array (16)
//! and memory device (17). Every read is bounds-checked; malformed tables
//! yield fewer structures, never a panic.

/// One SMBIOS structure: its formatted area and its string set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Structure {
    /// The structure type.
    pub kind: u8,
    /// The structure handle.
    pub handle: u16,
    /// The formatted area, including the 4-byte header.
    pub data: Vec<u8>,
    /// The strings that follow the formatted area, in order.
    pub strings: Vec<String>,
}

impl Structure {
    /// A byte at `offset` in the formatted area.
    #[must_use]
    pub fn byte(&self, offset: usize) -> Option<u8> {
        self.data.get(offset).copied()
    }

    /// A little-endian `u16` at `offset`.
    #[must_use]
    pub fn word(&self, offset: usize) -> Option<u16> {
        let b = self.data.get(offset..offset + 2)?;
        Some(u16::from_le_bytes([*b.first()?, *b.get(1)?]))
    }

    /// A little-endian `u32` at `offset`.
    #[must_use]
    pub fn dword(&self, offset: usize) -> Option<u32> {
        let b = self.data.get(offset..offset + 4)?;
        Some(u32::from_le_bytes([
            *b.first()?,
            *b.get(1)?,
            *b.get(2)?,
            *b.get(3)?,
        ]))
    }

    /// A little-endian `u64` at `offset`.
    #[must_use]
    pub fn qword(&self, offset: usize) -> Option<u64> {
        let b = self.data.get(offset..offset + 8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Some(u64::from_le_bytes(a))
    }

    /// The string referenced by the index byte at `offset`, trimmed. Empty
    /// strings and common placeholder values are treated as absent.
    #[must_use]
    pub fn string(&self, offset: usize) -> Option<String> {
        let index = usize::from(self.byte(offset)?);
        let s = self.strings.get(index.checked_sub(1)?)?.trim();
        if is_placeholder(s) {
            None
        } else {
            Some(s.to_owned())
        }
    }
}

/// True for the filler strings firmware vendors leave in unused fields.
#[must_use]
pub fn is_placeholder(s: &str) -> bool {
    let l = s.trim().to_ascii_lowercase();
    l.is_empty()
        || (l.len() >= 4 && l.chars().all(|c| c == '0'))
        || l.chars().all(|c| c == '.' || c == ' ')
        || matches!(
            l.as_str(),
            "default string"
                | "to be filled by o.e.m."
                | "to be filled by oem"
                | "system product name"
                | "system manufacturer"
                | "system version"
                | "system serial number"
                | "not specified"
                | "not applicable"
                | "none"
                | "unknown"
                | "n/a"
                | "oem"
                | "123456789"
        )
}

/// Splits a raw SMBIOS table into structures. Stops at the end-of-table
/// marker (type 127) or at the first malformed structure.
#[must_use]
pub fn parse(table: &[u8]) -> Vec<Structure> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(&kind) = table.get(pos) {
        let Some(&len) = table.get(pos + 1) else {
            break;
        };
        let len = usize::from(len);
        if len < 4 {
            break;
        }
        let Some(data) = table.get(pos..pos + len) else {
            break;
        };
        let handle = u16::from_le_bytes([
            table.get(pos + 2).copied().unwrap_or(0),
            table.get(pos + 3).copied().unwrap_or(0),
        ]);
        let Some((strings, next)) = read_strings(table, pos + len) else {
            break;
        };
        out.push(Structure {
            kind,
            handle,
            data: data.to_vec(),
            strings,
        });
        if kind == 127 {
            break;
        }
        pos = next;
    }
    out
}

/// Reads the string set starting at `start`. Returns the strings and the
/// offset just past the double-NUL terminator.
fn read_strings(table: &[u8], start: usize) -> Option<(Vec<String>, usize)> {
    let rest = table.get(start..)?;
    // An empty string set is two NULs.
    if rest.first() == Some(&0) {
        return if rest.get(1) == Some(&0) {
            Some((Vec::new(), start + 2))
        } else {
            None
        };
    }
    let mut strings = Vec::new();
    let mut pos = start;
    loop {
        let tail = table.get(pos..)?;
        let end = tail.iter().position(|&b| b == 0)?;
        let bytes = tail.get(..end)?;
        strings.push(String::from_utf8_lossy(bytes).into_owned());
        pos += end + 1;
        if table.get(pos) == Some(&0) {
            return Some((strings, pos + 1));
        }
    }
}

/// BIOS information (type 0).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bios {
    /// BIOS vendor.
    pub vendor: Option<String>,
    /// BIOS version string.
    pub version: Option<String>,
    /// Release date as written by the vendor (usually `MM/DD/YYYY`).
    pub release_date: Option<String>,
}

/// System information (type 1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct System {
    /// System manufacturer.
    pub manufacturer: Option<String>,
    /// Product name.
    pub product: Option<String>,
    /// Serial number.
    pub serial: Option<String>,
    /// SKU number.
    pub sku: Option<String>,
    /// Product family.
    pub family: Option<String>,
}

/// Baseboard information (type 2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseboard {
    /// Board manufacturer.
    pub manufacturer: Option<String>,
    /// Board model.
    pub product: Option<String>,
    /// Board revision.
    pub version: Option<String>,
    /// Board serial number.
    pub serial: Option<String>,
}

/// Processor socket information (type 4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Processor {
    /// Socket label printed on the board, such as `AM4`.
    pub socket: Option<String>,
    /// Processor version string.
    pub version: Option<String>,
    /// External clock in MHz.
    pub external_clock_mhz: Option<u16>,
    /// Maximum speed in MHz the socket supports.
    pub max_speed_mhz: Option<u16>,
}

/// How a system slot is currently used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotUsage {
    /// Nothing installed.
    Available,
    /// A card or drive is installed.
    InUse,
    /// The slot cannot be used, for example because lanes are shared.
    Unavailable,
    /// Firmware did not say.
    Unknown,
}

impl SlotUsage {
    /// A display label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::InUse => "In use",
            Self::Unavailable => "Unavailable",
            Self::Unknown => "Unknown",
        }
    }
}

/// An expansion slot (type 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// Silkscreen designation, such as `PCIEX16_1` or `J3700 M.2 Slot`.
    pub designation: Option<String>,
    /// Decoded slot type, such as `PCIe 4.0 x16` or `M.2 (Key M)`.
    pub kind: String,
    /// Decoded data bus width, such as `x4`.
    pub width: Option<String>,
    /// Current usage.
    pub usage: SlotUsage,
    /// Decoded characteristics, such as `3.3 V`, `PME`, `Hot-plug`.
    pub characteristics: Vec<String>,
    /// Slot ID.
    pub id: Option<u16>,
}

/// A physical memory array (type 16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryArray {
    /// Maximum supported memory in bytes.
    pub max_capacity_bytes: Option<u64>,
    /// Number of memory slots.
    pub slots: u16,
    /// Error correction type label.
    pub ecc: Option<&'static str>,
    /// True if this array is system memory (use = 3).
    pub is_system_memory: bool,
}

/// A memory device, populated or empty (type 17).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryDevice {
    /// Slot label, such as `DIMM_A1`.
    pub locator: Option<String>,
    /// Bank label, such as `BANK 0` or `P0 CHANNEL A`.
    pub bank: Option<String>,
    /// Installed size in bytes; `None` or `Some(0)` means empty.
    pub size_bytes: Option<u64>,
    /// Memory type, such as `DDR4`.
    pub memory_type: Option<&'static str>,
    /// Form factor, such as `DIMM`.
    pub form_factor: Option<&'static str>,
    /// Rated speed in MT/s.
    pub speed_mts: Option<u32>,
    /// Configured speed in MT/s.
    pub configured_speed_mts: Option<u32>,
    /// Manufacturer.
    pub manufacturer: Option<String>,
    /// Serial number.
    pub serial: Option<String>,
    /// Part number.
    pub part_number: Option<String>,
    /// Number of ranks.
    pub ranks: Option<u8>,
    /// Configured voltage in millivolts.
    pub configured_mv: Option<u16>,
}

impl MemoryDevice {
    /// True if a module is installed in this slot.
    #[must_use]
    pub fn is_populated(&self) -> bool {
        self.size_bytes.is_some_and(|s| s > 0)
    }
}

/// A decoded view of the structures this tool uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tables {
    /// BIOS information.
    pub bios: Option<Bios>,
    /// System information.
    pub system: Option<System>,
    /// Baseboard information.
    pub baseboard: Option<Baseboard>,
    /// Processor sockets.
    pub processors: Vec<Processor>,
    /// Expansion slots.
    pub slots: Vec<Slot>,
    /// Memory arrays.
    pub memory_arrays: Vec<MemoryArray>,
    /// Memory devices.
    pub memory_devices: Vec<MemoryDevice>,
}

impl Tables {
    /// Decodes a raw SMBIOS table.
    #[must_use]
    pub fn decode(table: &[u8]) -> Self {
        let mut t = Self::default();
        for s in parse(table) {
            match s.kind {
                0 => t.bios = Some(bios(&s)),
                1 => t.system = Some(system(&s)),
                2 => {
                    if t.baseboard.is_none() {
                        t.baseboard = Some(baseboard(&s));
                    }
                }
                4 => t.processors.push(processor(&s)),
                9 => t.slots.push(slot(&s)),
                16 => t.memory_arrays.push(memory_array(&s)),
                17 => t.memory_devices.push(memory_device(&s)),
                _ => {}
            }
        }
        t
    }

    /// Total memory slots across system memory arrays.
    #[must_use]
    pub fn total_memory_slots(&self) -> u16 {
        self.memory_arrays
            .iter()
            .filter(|a| a.is_system_memory)
            .map(|a| a.slots)
            .sum()
    }

    /// Maximum supported system memory in bytes.
    #[must_use]
    pub fn max_memory_bytes(&self) -> Option<u64> {
        let caps: Vec<u64> = self
            .memory_arrays
            .iter()
            .filter(|a| a.is_system_memory)
            .filter_map(|a| a.max_capacity_bytes)
            .collect();
        if caps.is_empty() {
            None
        } else {
            Some(caps.iter().sum())
        }
    }
}

fn bios(s: &Structure) -> Bios {
    Bios {
        vendor: s.string(0x04),
        version: s.string(0x05),
        release_date: s.string(0x08),
    }
}

fn system(s: &Structure) -> System {
    System {
        manufacturer: s.string(0x04),
        product: s.string(0x05),
        serial: s.string(0x07),
        sku: s.string(0x19),
        family: s.string(0x1A),
    }
}

fn baseboard(s: &Structure) -> Baseboard {
    Baseboard {
        manufacturer: s.string(0x04),
        product: s.string(0x05),
        version: s.string(0x06),
        serial: s.string(0x07),
    }
}

fn processor(s: &Structure) -> Processor {
    let nonzero = |v: Option<u16>| v.filter(|&x| x != 0);
    Processor {
        socket: s.string(0x04),
        version: s.string(0x10),
        external_clock_mhz: nonzero(s.word(0x12)),
        max_speed_mhz: nonzero(s.word(0x14)),
    }
}

/// Decodes an SMBIOS slot type code.
#[must_use]
pub fn slot_type_name(code: u8) -> String {
    let widths = ["", " x1", " x2", " x4", " x8", " x16"];
    let pcie = |base: u8, gen_label: &str| {
        let w = widths.get(usize::from(code - base)).copied().unwrap_or("");
        format!("PCIe{gen_label}{w}")
    };
    match code {
        0x06 => "PCI".to_owned(),
        0x0F => "AGP".to_owned(),
        0x13 => "PCIe Mini".to_owned(),
        0x14 => "M.2 (Key A)".to_owned(),
        0x15 => "M.2 (Key E)".to_owned(),
        0x16 => "M.2 (Key B)".to_owned(),
        0x17 => "M.2 (Key M)".to_owned(),
        0x1F..=0x23 => "PCIe U.2".to_owned(),
        0xA5..=0xAA => pcie(0xA5, ""),
        0xAB..=0xB0 => pcie(0xAB, " 2.0"),
        0xB1..=0xB6 => pcie(0xB1, " 3.0"),
        0xB8..=0xBD => pcie(0xB8, " 4.0"),
        0xBE..=0xC3 => pcie(0xBE, " 5.0"),
        0xC4..=0xC9 => pcie(0xC4, " 6.0"),
        _ => format!("Other (type 0x{code:02X})"),
    }
}

/// Decodes an SMBIOS data bus width code into a lane label.
#[must_use]
pub const fn bus_width_name(code: u8) -> Option<&'static str> {
    match code {
        0x08 => Some("x1"),
        0x09 => Some("x2"),
        0x0A => Some("x4"),
        0x0B => Some("x8"),
        0x0C => Some("x12"),
        0x0D => Some("x16"),
        0x0E => Some("x32"),
        _ => None,
    }
}

fn slot(s: &Structure) -> Slot {
    let usage = match s.byte(0x07) {
        Some(3) => SlotUsage::Available,
        Some(4) => SlotUsage::InUse,
        Some(5) => SlotUsage::Unavailable,
        _ => SlotUsage::Unknown,
    };
    let mut characteristics = Vec::new();
    if let Some(c1) = s.byte(0x0B) {
        if c1 & 0x02 != 0 {
            characteristics.push("5 V".to_owned());
        }
        if c1 & 0x04 != 0 {
            characteristics.push("3.3 V".to_owned());
        }
        if c1 & 0x08 != 0 {
            characteristics.push("Shared".to_owned());
        }
    }
    if let Some(c2) = s.byte(0x0C) {
        if c2 & 0x01 != 0 {
            characteristics.push("PME".to_owned());
        }
        if c2 & 0x02 != 0 {
            characteristics.push("Hot-plug".to_owned());
        }
        if c2 & 0x04 != 0 {
            characteristics.push("SMBus".to_owned());
        }
    }
    Slot {
        designation: s.string(0x04),
        kind: s
            .byte(0x05)
            .map_or_else(|| "Unknown".to_owned(), slot_type_name),
        width: s.byte(0x06).and_then(bus_width_name).map(ToOwned::to_owned),
        usage,
        characteristics,
        id: s.word(0x09),
    }
}

fn memory_array(s: &Structure) -> MemoryArray {
    let max_kib = s.dword(0x07);
    let max_capacity_bytes = match max_kib {
        Some(0x8000_0000) => s.qword(0x0F),
        Some(kib) if kib > 0 => Some(u64::from(kib) * 1024),
        _ => None,
    };
    let ecc = match s.byte(0x06) {
        Some(3) => Some("None"),
        Some(4) => Some("Parity"),
        Some(5) => Some("Single-bit ECC"),
        Some(6) => Some("Multi-bit ECC"),
        Some(7) => Some("CRC"),
        _ => None,
    };
    MemoryArray {
        max_capacity_bytes,
        slots: s.word(0x0D).unwrap_or(0),
        ecc,
        is_system_memory: s.byte(0x05) == Some(3),
    }
}

/// Decodes an SMBIOS memory type code.
#[must_use]
pub const fn memory_type_name(code: u8) -> Option<&'static str> {
    match code {
        0x0F => Some("SDRAM"),
        0x12 => Some("DDR"),
        0x13 => Some("DDR2"),
        0x18 => Some("DDR3"),
        0x1A => Some("DDR4"),
        0x1B => Some("LPDDR"),
        0x1C => Some("LPDDR2"),
        0x1D => Some("LPDDR3"),
        0x1E => Some("LPDDR4"),
        0x20 => Some("HBM"),
        0x21 => Some("HBM2"),
        0x22 => Some("DDR5"),
        0x23 => Some("LPDDR5"),
        0x24 => Some("HBM3"),
        _ => None,
    }
}

const fn form_factor_name(code: u8) -> Option<&'static str> {
    match code {
        0x08 => Some("Row of chips"),
        0x09 => Some("DIMM"),
        0x0B => Some("Chip"),
        0x0D => Some("SODIMM"),
        0x0F => Some("FB-DIMM"),
        0x10 => Some("Die"),
        0x11 => Some("CAMM"),
        _ => None,
    }
}

fn memory_device(s: &Structure) -> MemoryDevice {
    let size_bytes = match s.word(0x0C) {
        Some(0xFFFF) | None => None,
        Some(0x7FFF) => s
            .dword(0x1C)
            .map(|mb| u64::from(mb & 0x7FFF_FFFF) * 1024 * 1024),
        Some(v) if v & 0x8000 != 0 => Some(u64::from(v & 0x7FFF) * 1024),
        Some(v) => Some(u64::from(v) * 1024 * 1024),
    };
    let speed = |base: usize, ext: usize| match s.word(base) {
        Some(0) | None => None,
        Some(0xFFFF) => s.dword(ext).filter(|&v| v != 0),
        Some(v) => Some(u32::from(v)),
    };
    MemoryDevice {
        locator: s.string(0x10),
        bank: s.string(0x11),
        size_bytes,
        memory_type: s.byte(0x12).and_then(memory_type_name),
        form_factor: s.byte(0x0E).and_then(form_factor_name),
        speed_mts: speed(0x15, 0x54),
        configured_speed_mts: speed(0x20, 0x58),
        manufacturer: s.string(0x17),
        serial: s.string(0x18),
        part_number: s.string(0x1A),
        ranks: s.byte(0x1B).map(|a| a & 0x0F).filter(|&r| r != 0),
        configured_mv: s.word(0x26).filter(|&v| v != 0),
    }
}

/// Helpers to build synthetic SMBIOS tables, used by tests and fixtures.
#[cfg(test)]
pub mod testing {
    /// Builds one structure from a formatted area (without header) and strings.
    ///
    /// # Panics
    ///
    /// Panics if the body is longer than 251 bytes.
    #[must_use]
    pub fn structure(kind: u8, handle: u16, body: &[u8], strings: &[&str]) -> Vec<u8> {
        let len = u8::try_from(body.len() + 4).unwrap();
        let mut out = vec![kind, len];
        out.extend_from_slice(&handle.to_le_bytes());
        out.extend_from_slice(body);
        if strings.is_empty() {
            out.extend_from_slice(&[0, 0]);
        } else {
            for s in strings {
                out.extend_from_slice(s.as_bytes());
                out.push(0);
            }
            out.push(0);
        }
        out
    }

    /// A type 17 body (offsets 0x04..0x5C) with the given fields.
    ///
    /// # Panics
    ///
    /// Never; the offsets are fixed and in range.
    #[must_use]
    pub fn memory_device_body(size_mb: u16, mem_type: u8, speed: u16, configured: u16) -> Vec<u8> {
        let mut b = vec![0u8; 0x5C - 4];
        let mut put = |off: usize, bytes: &[u8]| {
            b[off - 4..off - 4 + bytes.len()].copy_from_slice(bytes);
        };
        put(0x0C, &size_mb.to_le_bytes());
        put(0x0E, &[0x09]);
        put(0x10, &[1]);
        put(0x11, &[2]);
        put(0x12, &[mem_type]);
        put(0x15, &speed.to_le_bytes());
        put(0x17, &[3]);
        put(0x18, &[4]);
        put(0x1A, &[5]);
        put(0x1B, &[2]);
        put(0x20, &configured.to_le_bytes());
        put(0x26, &1200u16.to_le_bytes());
        b
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    fn sample_table() -> Vec<u8> {
        let mut t = Vec::new();
        // BIOS: vendor, version, start segment, release date.
        t.extend(structure(
            0,
            0,
            &[1, 2, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[
                "American Megatrends International, LLC.",
                "F1",
                "11/18/2024",
            ],
        ));
        // Baseboard.
        t.extend(structure(
            2,
            2,
            &[1, 2, 3, 4, 0],
            &[
                "Gigabyte Technology Co., Ltd.",
                "B550 EAGLE WIFI6",
                "x.x",
                "Default string",
            ],
        ));
        // Slots: x16 in use, M.2 key M in use, x4 available.
        let slot = |desig: u8, kind: u8, width: u8, usage: u8, id: u16| {
            let mut b = vec![desig, kind, width, usage, 0x04];
            b.extend_from_slice(&id.to_le_bytes());
            b.extend_from_slice(&[0x04, 0x01, 0, 0, 0, 0]);
            b
        };
        t.extend(structure(9, 9, &slot(1, 0xBD, 0x0D, 4, 0), &["J10"]));
        t.extend(structure(
            9,
            10,
            &slot(1, 0x17, 0x0A, 4, 1),
            &["J3700 M.2 Slot"],
        ));
        t.extend(structure(
            9,
            11,
            &slot(1, 0xA5, 0x0A, 3, 2),
            &["J3708 PCIE x4 slot"],
        ));
        // Memory array: system memory, 128 GiB max, 4 slots.
        let mut arr = vec![3, 3, 3];
        arr.extend_from_slice(&(128u32 * 1024 * 1024).to_le_bytes());
        arr.extend_from_slice(&[0xFE, 0xFF, 4, 0]);
        t.extend(structure(16, 16, &arr, &[]));
        // One populated DDR4 module and one empty slot.
        t.extend(structure(
            17,
            17,
            &memory_device_body(16384, 0x1A, 3200, 2133),
            &[
                "DIMM 1",
                "P0 CHANNEL A",
                "G.Skill",
                "00000000",
                "F4-3200C16-16GVK",
            ],
        ));
        t.extend(structure(
            17,
            18,
            &memory_device_body(0, 0x02, 0, 0),
            &["DIMM 0", "P0 CHANNEL A", "Unknown", "Unknown", "Unknown"],
        ));
        t.extend(structure(127, 0xFFFF, &[], &[]));
        t
    }

    #[test]
    fn splits_structures_and_strings() {
        let s = parse(&sample_table());
        assert_eq!(s.len(), 9);
        assert_eq!(s[0].strings.len(), 3);
        assert_eq!(s.last().unwrap().kind, 127);
    }

    #[test]
    fn decodes_tables() {
        let t = Tables::decode(&sample_table());
        let bios = t.bios.as_ref().unwrap();
        assert_eq!(bios.version.as_deref(), Some("F1"));
        assert_eq!(bios.release_date.as_deref(), Some("11/18/2024"));
        let board = t.baseboard.as_ref().unwrap();
        assert_eq!(board.product.as_deref(), Some("B550 EAGLE WIFI6"));
        assert_eq!(board.serial, None, "placeholder serial is dropped");
        assert_eq!(t.slots.len(), 3);
        assert_eq!(t.slots[0].kind, "PCIe 4.0 x16");
        assert_eq!(t.slots[0].width.as_deref(), Some("x16"));
        assert_eq!(t.slots[0].usage, SlotUsage::InUse);
        assert_eq!(t.slots[1].kind, "M.2 (Key M)");
        assert_eq!(t.slots[2].usage, SlotUsage::Available);
        assert_eq!(t.slots[2].characteristics, vec!["3.3 V", "PME"]);
        assert_eq!(t.total_memory_slots(), 4);
        assert_eq!(t.max_memory_bytes(), Some(128 * 1024 * 1024 * 1024));
        let dimm = &t.memory_devices[0];
        assert!(dimm.is_populated());
        assert_eq!(dimm.size_bytes, Some(16 * 1024 * 1024 * 1024));
        assert_eq!(dimm.memory_type, Some("DDR4"));
        assert_eq!(dimm.speed_mts, Some(3200));
        assert_eq!(dimm.configured_speed_mts, Some(2133));
        assert_eq!(dimm.part_number.as_deref(), Some("F4-3200C16-16GVK"));
        assert_eq!(dimm.serial, None);
        assert_eq!(dimm.ranks, Some(2));
        assert_eq!(dimm.configured_mv, Some(1200));
        assert!(!t.memory_devices[1].is_populated());
        assert_eq!(t.memory_devices[1].manufacturer, None);
    }

    #[test]
    fn survives_truncated_and_garbage_input() {
        let table = sample_table();
        for cut in 0..table.len() {
            let _ = Tables::decode(&table[..cut]);
        }
        let _ = Tables::decode(&[0xFF; 64]);
        let _ = Tables::decode(&[1, 2, 3]);
    }

    #[test]
    fn decodes_slot_types() {
        assert_eq!(slot_type_name(0xAA), "PCIe x16");
        assert_eq!(slot_type_name(0xB4), "PCIe 3.0 x4");
        assert_eq!(slot_type_name(0xC2), "PCIe 5.0 x8");
        assert_eq!(slot_type_name(0x01), "Other (type 0x01)");
    }

    #[test]
    fn extended_memory_size() {
        let mut body = memory_device_body(0x7FFF, 0x22, 0xFFFF, 0);
        body[0x1C - 4..0x20 - 4].copy_from_slice(&(65536u32).to_le_bytes());
        body[0x54 - 4..0x58 - 4].copy_from_slice(&(8000u32).to_le_bytes());
        let t = Tables::decode(&structure(17, 1, &body, &["A", "B", "C", "D", "E"]));
        let d = &t.memory_devices[0];
        assert_eq!(d.size_bytes, Some(64 * 1024 * 1024 * 1024));
        assert_eq!(d.memory_type, Some("DDR5"));
        assert_eq!(d.speed_mts, Some(8000));
    }
}
