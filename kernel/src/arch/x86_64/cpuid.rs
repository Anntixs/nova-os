use core::arch::x86_64::__cpuid_count;

pub struct CpuInfo {
    pub vendor: [u8; 12],
    pub brand: [u8; 48],
    pub family: u8,
    pub model: u8,
    pub stepping: u8,
    pub features: u64,
}
pub mod feature {
    pub const SSE: u64 = 1 << 0;
    pub const SSE2: u64 = 1 << 1;
    pub const SSE3: u64 = 1 << 2;
    pub const SSSE3: u64 = 1 << 3;
    pub const SSE41: u64 = 1 << 4;
    pub const SSE42: u64 = 1 << 5;
    pub const AVX: u64 = 1 << 6;
    pub const AVX2: u64 = 1 << 7;
    pub const AES: u64 = 1 << 8;
    pub const NX: u64 = 1 << 9;
    pub const APIC: u64 = 1 << 10;
    pub const X2APIC: u64 = 1 << 11;
    pub const TSC: u64 = 1 << 12;
    pub const RDRAND: u64 = 1 << 13;
    pub const SMEP: u64 = 1 << 14;
    pub const SMAP: u64 = 1 << 15;
    pub const FSGSBASE: u64 = 1 << 16;
}
pub fn detect() -> CpuInfo {
    // CPUID is available on all x86_64 CPUs. This only detects features; it enables none.
    {
        let base = __cpuid_count(0, 0);
        let l1 = __cpuid_count(1, 0);
        let ext = __cpuid_count(0x80000000, 0);
        let e1 = if ext.eax >= 0x80000001 {
            __cpuid_count(0x80000001, 0)
        } else {
            __cpuid_count(0, 0)
        };
        let l7 = if base.eax >= 7 {
            __cpuid_count(7, 0)
        } else {
            __cpuid_count(0, 0)
        };
        let mut vendor = [0; 12];
        vendor[..4].copy_from_slice(&base.ebx.to_le_bytes());
        vendor[4..8].copy_from_slice(&base.edx.to_le_bytes());
        vendor[8..].copy_from_slice(&base.ecx.to_le_bytes());
        let mut brand = [0; 48];
        if ext.eax >= 0x80000004 {
            for i in 0..3 {
                let r = __cpuid_count(0x80000002 + i, 0);
                let j = i as usize * 16;
                brand[j..j + 4].copy_from_slice(&r.eax.to_le_bytes());
                brand[j + 4..j + 8].copy_from_slice(&r.ebx.to_le_bytes());
                brand[j + 8..j + 12].copy_from_slice(&r.ecx.to_le_bytes());
                brand[j + 12..j + 16].copy_from_slice(&r.edx.to_le_bytes());
            }
        }
        let mut f = 0;
        let ec = l1.ecx;
        let ed = l1.edx;
        let b = l7.ebx;
        for (yes, bit) in [
            (ed & (1 << 25) != 0, feature::SSE),
            (ed & (1 << 26) != 0, feature::SSE2),
            (ec & 1 != 0, feature::SSE3),
            (ec & (1 << 9) != 0, feature::SSSE3),
            (ec & (1 << 19) != 0, feature::SSE41),
            (ec & (1 << 20) != 0, feature::SSE42),
            (ec & (1 << 28) != 0, feature::AVX),
            (b & (1 << 5) != 0, feature::AVX2),
            (ec & (1 << 25) != 0, feature::AES),
            (e1.edx & (1 << 20) != 0, feature::NX),
            (ed & (1 << 9) != 0, feature::APIC),
            (ec & (1 << 21) != 0, feature::X2APIC),
            (ed & (1 << 4) != 0, feature::TSC),
            (ec & (1 << 30) != 0, feature::RDRAND),
            (b & (1 << 7) != 0, feature::SMEP),
            (b & (1 << 20) != 0, feature::SMAP),
            (b & 1 != 0, feature::FSGSBASE),
        ] {
            if yes {
                f |= bit;
            }
        }
        let mut family = ((l1.eax >> 8) & 0xf) as u8;
        let mut model = ((l1.eax >> 4) & 0xf) as u8;
        if family == 0xf {
            family = family.wrapping_add(((l1.eax >> 20) & 0xff) as u8);
        }
        if family == 6 || family == 0xf {
            model |= (((l1.eax >> 16) & 0xf) as u8) << 4;
        }
        CpuInfo {
            vendor,
            brand,
            family,
            model,
            stepping: (l1.eax & 15) as u8,
            features: f,
        }
    }
}
