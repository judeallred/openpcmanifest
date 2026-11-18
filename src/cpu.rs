//! CPUID collection and decoding.
//!
//! Collection records raw register values so a saved snapshot can be decoded
//! again later, on any machine. Decoding is pure and tested with synthetic
//! register values.

use crate::json::Value;

/// One CPUID result: `[leaf, subleaf, eax, ebx, ecx, edx]`.
pub type Leaf = [u32; 6];

/// Reads the CPUID leaves this tool decodes from the current processor.
/// Returns an empty list on targets other than x86 and x86-64.
#[must_use]
pub fn read_local() -> Vec<Leaf> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::__cpuid_count;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::__cpuid_count;
        let q = |leaf: u32, sub: u32| {
            let r = __cpuid_count(leaf, sub);
            [leaf, sub, r.eax, r.ebx, r.ecx, r.edx]
        };
        let mut out = Vec::new();
        let base = q(0, 0);
        let max_basic = base[2];
        out.push(base);
        for leaf in [1u32, 7] {
            if leaf <= max_basic {
                out.push(q(leaf, 0));
            }
        }
        if max_basic >= 4 {
            for sub in 0..8 {
                let r = q(4, sub);
                let done = r[2].trailing_zeros() >= 5;
                out.push(r);
                if done {
                    break;
                }
            }
        }
        let ext = q(0x8000_0000, 0);
        let max_ext = ext[2];
        out.push(ext);
        for leaf in [
            0x8000_0001u32,
            0x8000_0002,
            0x8000_0003,
            0x8000_0004,
            0x8000_0008,
        ] {
            if leaf <= max_ext {
                out.push(q(leaf, 0));
            }
        }
        if max_ext >= 0x8000_001D {
            for sub in 0..8 {
                let r = q(0x8000_001D, sub);
                let done = r[2].trailing_zeros() >= 5;
                out.push(r);
                if done {
                    break;
                }
            }
        }
        out
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        Vec::new()
    }
}

/// Converts leaves to JSON for the raw snapshot.
#[must_use]
pub fn to_json(leaves: &[Leaf]) -> Value {
    Value::Array(
        leaves
            .iter()
            .map(|l| Value::Array(l.iter().map(|&v| Value::Number(f64::from(v))).collect()))
            .collect(),
    )
}

/// Reads leaves back from a raw snapshot.
#[must_use]
pub fn from_json(v: &Value) -> Vec<Leaf> {
    v.items()
        .into_iter()
        .filter_map(|row| {
            let items = row.items();
            if items.len() != 6 {
                return None;
            }
            let mut leaf = [0u32; 6];
            for (slot, item) in leaf.iter_mut().zip(items) {
                *slot = u32::try_from(item.as_u64()?).ok()?;
            }
            Some(leaf)
        })
        .collect()
}

/// One cache level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    /// Cache level, 1 to 3.
    pub level: u8,
    /// `Data`, `Instruction` or `Unified`.
    pub kind: &'static str,
    /// Size of one instance in bytes.
    pub size_bytes: u64,
    /// Associativity.
    pub ways: u32,
    /// Number of logical processors sharing one instance.
    pub shared_by: u32,
}

/// Decoded processor identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuInfo {
    /// Vendor ID string, such as `AuthenticAMD`.
    pub vendor: String,
    /// Brand string, such as `AMD Ryzen 5 3600 6-Core Processor`.
    pub brand: Option<String>,
    /// Display family.
    pub family: u32,
    /// Display model.
    pub model: u32,
    /// Stepping.
    pub stepping: u32,
    /// Base family field.
    pub base_family: u32,
    /// Extended family field.
    pub extended_family: u32,
    /// Base model field.
    pub base_model: u32,
    /// Extended model field.
    pub extended_model: u32,
    /// Supported instruction set extensions, in conventional order.
    pub features: Vec<&'static str>,
    /// Hardware virtualisation support in the CPU (VT-x or AMD-V).
    pub virtualization: Option<&'static str>,
    /// True on hybrid (performance + efficiency core) Intel designs.
    pub hybrid: bool,
    /// True when running under a hypervisor.
    pub hypervisor: bool,
    /// Cache levels.
    pub caches: Vec<Cache>,
    /// Microarchitecture code name and process, when known.
    pub code_name: Option<(&'static str, &'static str)>,
}

fn find(leaves: &[Leaf], leaf: u32, sub: u32) -> Option<&Leaf> {
    leaves.iter().find(|l| l[0] == leaf && l[1] == sub)
}

fn regs_to_string(regs: &[u32]) -> String {
    let bytes: Vec<u8> = regs.iter().flat_map(|r| r.to_le_bytes()).collect();
    String::from_utf8_lossy(&bytes)
        .trim_matches(char::from(0))
        .trim()
        .to_owned()
}

const fn bit(v: u32, n: u32) -> bool {
    v & (1 << n) != 0
}

/// Decodes recorded CPUID leaves. Returns `None` if leaf 0 is missing.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one linear decode of documented bit fields"
)]
pub fn decode(leaves: &[Leaf]) -> Option<CpuInfo> {
    let l0 = find(leaves, 0, 0)?;
    let mut info = CpuInfo {
        vendor: regs_to_string(&[l0[3], l0[5], l0[4]]),
        ..CpuInfo::default()
    };

    let brand: Vec<u32> = (0x8000_0002u32..=0x8000_0004)
        .filter_map(|leaf| find(leaves, leaf, 0))
        .flat_map(|l| [l[2], l[3], l[4], l[5]])
        .collect();
    if brand.len() == 12 {
        let s = regs_to_string(&brand);
        if !s.is_empty() {
            info.brand = Some(s.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }

    if let Some(l1) = find(leaves, 1, 0) {
        let eax = l1[2];
        info.stepping = eax & 0xF;
        info.base_model = (eax >> 4) & 0xF;
        info.base_family = (eax >> 8) & 0xF;
        info.extended_model = (eax >> 16) & 0xF;
        info.extended_family = (eax >> 20) & 0xFF;
        info.family = if info.base_family == 0xF {
            info.base_family + info.extended_family
        } else {
            info.base_family
        };
        info.model = if info.base_family == 0x6 || info.base_family == 0xF {
            (info.extended_model << 4) + info.base_model
        } else {
            info.base_model
        };
        let (ecx, edx) = (l1[4], l1[5]);
        info.hypervisor = bit(ecx, 31);
        let mut f = Vec::new();
        for (on, name) in [
            (bit(edx, 23), "MMX"),
            (bit(edx, 25), "SSE"),
            (bit(edx, 26), "SSE2"),
            (bit(ecx, 0), "SSE3"),
            (bit(ecx, 9), "SSSE3"),
            (bit(ecx, 19), "SSE4.1"),
            (bit(ecx, 20), "SSE4.2"),
        ] {
            if on {
                f.push(name);
            }
        }
        if let Some(e1) = find(leaves, 0x8000_0001, 0)
            && bit(e1[4], 6)
        {
            f.push("SSE4A");
        }
        for (on, name) in [
            (bit(ecx, 25), "AES"),
            (bit(ecx, 1), "PCLMULQDQ"),
            (bit(ecx, 28), "AVX"),
            (bit(ecx, 12), "FMA3"),
            (bit(ecx, 29), "F16C"),
            (bit(ecx, 30), "RDRAND"),
        ] {
            if on {
                f.push(name);
            }
        }
        if bit(ecx, 5) {
            info.virtualization = Some("Intel VT-x");
        }
        info.features = f;
    }

    if let Some(l7) = find(leaves, 7, 0) {
        let (ebx, ecx, edx) = (l7[3], l7[4], l7[5]);
        for (on, name) in [
            (bit(ebx, 5), "AVX2"),
            (bit(ebx, 3), "BMI1"),
            (bit(ebx, 8), "BMI2"),
            (bit(ebx, 19), "ADX"),
            (bit(ebx, 29), "SHA"),
            (bit(ebx, 16), "AVX-512F"),
            (bit(ecx, 8), "GFNI"),
            (bit(ecx, 9), "VAES"),
        ] {
            if on {
                info.features.push(name);
            }
        }
        info.hybrid = bit(edx, 15);
    }

    if let Some(e1) = find(leaves, 0x8000_0001, 0) {
        if bit(e1[4], 2) {
            info.virtualization = Some("AMD-V");
        }
        if bit(e1[5], 29) {
            info.features.push("x86-64");
        }
        if bit(e1[5], 20) {
            info.features.push("NX");
        }
    }

    let cache_leaf = if info.vendor == "AuthenticAMD" || info.vendor == "HygonGenuine" {
        0x8000_001D
    } else {
        4
    };
    info.caches = decode_caches(leaves, cache_leaf);
    info.code_name = code_name(&info.vendor, info.family, info.model);
    Some(info)
}

fn decode_caches(leaves: &[Leaf], leaf: u32) -> Vec<Cache> {
    let mut caches = Vec::new();
    for sub in 0..8 {
        let Some(l) = find(leaves, leaf, sub) else {
            break;
        };
        let (eax, ebx, ecx) = (l[2], l[3], l[4]);
        let kind = match eax & 0x1F {
            1 => "Data",
            2 => "Instruction",
            3 => "Unified",
            _ => break,
        };
        let line = u64::from((ebx & 0xFFF) + 1);
        let partitions = u64::from(((ebx >> 12) & 0x3FF) + 1);
        let ways = ((ebx >> 22) & 0x3FF) + 1;
        let sets = u64::from(ecx) + 1;
        let level = u8::try_from((eax >> 5) & 0x7).unwrap_or(0);
        caches.push(Cache {
            level,
            kind,
            size_bytes: u64::from(ways) * partitions * line * sets,
            ways,
            shared_by: ((eax >> 14) & 0xFFF) + 1,
        });
    }
    caches
}

/// Maps a vendor, family and model to a code name and process node. The
/// table covers common desktop and laptop parts from 2017 on; contributions
/// are welcome.
#[must_use]
pub fn code_name(vendor: &str, family: u32, model: u32) -> Option<(&'static str, &'static str)> {
    match (vendor, family, model) {
        ("AuthenticAMD", 0x17, 0x01) => Some(("Summit Ridge", "14 nm")),
        ("AuthenticAMD", 0x17, 0x08) => Some(("Pinnacle Ridge", "12 nm")),
        ("AuthenticAMD", 0x17, 0x18) => Some(("Picasso", "12 nm")),
        ("AuthenticAMD", 0x17, 0x60) => Some(("Renoir", "7 nm")),
        ("AuthenticAMD", 0x17, 0x71) => Some(("Matisse", "7 nm")),
        ("AuthenticAMD", 0x19, 0x21) => Some(("Vermeer", "7 nm")),
        ("AuthenticAMD", 0x19, 0x44) => Some(("Rembrandt", "6 nm")),
        ("AuthenticAMD", 0x19, 0x50) => Some(("Cezanne", "7 nm")),
        ("AuthenticAMD", 0x19, 0x61) => Some(("Raphael", "5 nm")),
        ("AuthenticAMD", 0x19, 0x74) => Some(("Phoenix", "4 nm")),
        ("AuthenticAMD", 0x1A, 0x44) => Some(("Granite Ridge", "4 nm")),
        ("GenuineIntel", 6, 0x9E) => Some(("Coffee Lake", "14 nm")),
        ("GenuineIntel", 6, 0xA5) => Some(("Comet Lake", "14 nm")),
        ("GenuineIntel", 6, 0xA7) => Some(("Rocket Lake", "14 nm")),
        ("GenuineIntel", 6, 0x97 | 0x9A) => Some(("Alder Lake", "Intel 7")),
        ("GenuineIntel", 6, 0xB7 | 0xBF) => Some(("Raptor Lake", "Intel 7")),
        ("GenuineIntel", 6, 0xAA) => Some(("Meteor Lake", "Intel 4")),
        ("GenuineIntel", 6, 0xBD) => Some(("Lunar Lake", "TSMC N3B")),
        ("GenuineIntel", 6, 0xC6) => Some(("Arrow Lake", "TSMC N3B")),
        _ => None,
    }
}

/// Estimates whether a processor meets the Windows 11 CPU floor.
///
/// The floor is roughly Intel 8th generation or AMD Zen+ and later.
/// Microsoft's published list is the authority; this is labelled as an
/// estimate wherever it is shown.
#[must_use]
pub fn likely_windows11_cpu(info: &CpuInfo) -> Option<bool> {
    match info.vendor.as_str() {
        "AuthenticAMD" => Some(info.family > 0x17 || (info.family == 0x17 && info.model >= 0x08)),
        "GenuineIntel" if info.family == 6 => Some(info.model >= 0x8E && info.model != 0x9D),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::similar_names, reason = "register names such as eax1 and ecx1")]
mod tests {
    use super::*;

    fn str_regs(s: &str) -> Vec<u32> {
        let mut b = s.as_bytes().to_vec();
        b.resize(48, 0);
        b.chunks(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// A Ryzen 5 3600 (family 17h, model 71h, stepping 0).
    fn ryzen_3600() -> Vec<Leaf> {
        let v = str_regs("AuthenticAMD");
        let b = str_regs("AMD Ryzen 5 3600 6-Core Processor             ");
        let eax1 = (0x8 << 20) | (0x7 << 16) | (0xF << 8) | (0x1 << 4);
        let ecx1 = (1 << 0) | (1 << 9) | (1 << 19) | (1 << 20) | (1 << 25) | (1 << 28) | (1 << 12);
        let edx1 = (1 << 23) | (1 << 25) | (1 << 26);
        let mut leaves = vec![
            [0, 0, 0x10, v[0], v[2], v[1]],
            [1, 0, eax1, 0, ecx1, edx1],
            [7, 0, 0, (1 << 5) | (1 << 3) | (1 << 8) | (1 << 29), 0, 0],
            [
                0x8000_0001,
                0,
                0,
                0,
                (1 << 2) | (1 << 6),
                (1 << 29) | (1 << 20),
            ],
            [0x8000_0002, 0, b[0], b[1], b[2], b[3]],
            [0x8000_0003, 0, b[4], b[5], b[6], b[7]],
            [0x8000_0004, 0, b[8], b[9], b[10], b[11]],
        ];
        // L1D 32 KiB 8-way, shared by 2 threads; L3 16 MiB 16-way shared by 6.
        leaves.push([
            0x8000_001D,
            0,
            1 | (1 << 5) | (1 << 14),
            63 | (7 << 22),
            63,
            0,
        ]);
        leaves.push([
            0x8000_001D,
            1,
            3 | (3 << 5) | (5 << 14),
            63 | (15 << 22),
            16383,
            0,
        ]);
        leaves.push([0x8000_001D, 2, 0, 0, 0, 0]);
        leaves
    }

    #[test]
    fn decodes_ryzen() {
        let info = decode(&ryzen_3600()).unwrap();
        assert_eq!(info.vendor, "AuthenticAMD");
        assert_eq!(
            info.brand.as_deref(),
            Some("AMD Ryzen 5 3600 6-Core Processor")
        );
        assert_eq!((info.family, info.model, info.stepping), (0x17, 0x71, 0));
        assert_eq!(info.code_name, Some(("Matisse", "7 nm")));
        assert_eq!(info.virtualization, Some("AMD-V"));
        for f in ["SSE4A", "AVX2", "AES", "SHA", "FMA3", "x86-64"] {
            assert!(info.features.contains(&f), "missing {f}");
        }
        assert_eq!(info.caches.len(), 2);
        assert_eq!(info.caches[0].size_bytes, 32 * 1024);
        assert_eq!(info.caches[0].shared_by, 2);
        assert_eq!(info.caches[1].size_bytes, 16 * 1024 * 1024);
        assert_eq!(likely_windows11_cpu(&info), Some(true));
    }

    #[test]
    fn json_round_trip() {
        let leaves = ryzen_3600();
        assert_eq!(from_json(&to_json(&leaves)), leaves);
    }

    #[test]
    fn missing_leaf_zero_is_none() {
        assert!(decode(&[]).is_none());
    }

    #[test]
    fn reads_local_cpu_without_panicking() {
        let leaves = read_local();
        if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
            let info = decode(&leaves).unwrap();
            assert!(!info.vendor.is_empty());
        }
    }
}
