use super::{cpu, cpuid};
use crate::memory::{
    paging::{self, PageFlags},
    Page, PhysAddr, PhysFrame, VirtAddr,
};
use core::arch::asm;
use nova_core::NovaBootInfo;

const LAPIC_VA: u64 = 0xffff_9200_0000_0000;
const IOAPIC_VA: u64 = 0xffff_9200_0000_1000;
static mut LAPIC: *mut u32 = core::ptr::null_mut();
static mut IOAPIC: *mut u32 = core::ptr::null_mut();
#[derive(Debug)]
pub enum Error {
    MissingApic,
    MissingAcpi,
    InvalidAcpi,
    MissingIoapic,
    BadRouting,
    Mapping,
}
struct Routing {
    lapic: u64,
    ioapic: u64,
    gsi_base: u32,
    irq: [u32; 2],
    flags: [u16; 2],
}
fn u16_at(p: *const u8, o: usize) -> u16 {
    // SAFETY: caller validated ACPI object bounds.
    unsafe { core::ptr::read_unaligned(p.add(o) as *const u16) }
}
fn u32_at(p: *const u8, o: usize) -> u32 {
    // SAFETY: caller validated ACPI object bounds.
    unsafe { core::ptr::read_unaligned(p.add(o) as *const u32) }
}
fn u64_at(p: *const u8, o: usize) -> u64 {
    // SAFETY: caller validated ACPI object bounds.
    unsafe { core::ptr::read_unaligned(p.add(o) as *const u64) }
}
fn checksum(p: *const u8, len: usize) -> bool {
    let mut sum = 0u8;
    for i in 0..len {
        // SAFETY: ACPI length is checked against a strict maximum before this read.
        sum = sum.wrapping_add(unsafe { p.add(i).read_volatile() });
    }
    sum == 0
}
fn sdt(p: *const u8) -> Result<usize, Error> {
    if p.is_null() {
        return Err(Error::InvalidAcpi);
    }
    let len = u32_at(p, 4) as usize;
    if !(36..=1024 * 1024).contains(&len) || !checksum(p, len) {
        return Err(Error::InvalidAcpi);
    }
    Ok(len)
}
fn madt(info: &NovaBootInfo) -> Result<Routing, Error> {
    if info.rsdp == 0 {
        return Err(Error::MissingAcpi);
    }
    let r = info.rsdp as *const u8;
    // SAFETY: NovaBoot passes an ACPI RSDP pointer from the firmware configuration table.
    if unsafe { core::slice::from_raw_parts(r, 8) } != b"RSD PTR " || !checksum(r, 20) {
        return Err(Error::InvalidAcpi);
    }
    let revision = unsafe { r.add(15).read() };
    let (root, entry_size) = if revision >= 2 {
        let length = u32_at(r, 20) as usize;
        if !(36..=4096).contains(&length) || !checksum(r, length) {
            return Err(Error::InvalidAcpi);
        }
        (u64_at(r, 24), 8usize)
    } else {
        (u32_at(r, 16) as u64, 4usize)
    };
    let root = root as *const u8;
    let root_len = sdt(root)?;
    let mut madt = core::ptr::null();
    let count = (root_len - 36) / entry_size;
    for i in 0..count {
        let p = if entry_size == 8 {
            u64_at(root, 36 + i * 8)
        } else {
            u32_at(root, 36 + i * 4) as u64
        } as *const u8;
        if p.is_null() {
            continue;
        } // SAFETY: firmware's SDT pointer refers to at least the common header.
        if unsafe { core::slice::from_raw_parts(p, 4) } == b"APIC" {
            sdt(p)?;
            madt = p;
            break;
        }
    }
    if madt.is_null() {
        return Err(Error::MissingIoapic);
    }
    let len = sdt(madt)?;
    if len < 44 {
        return Err(Error::InvalidAcpi);
    }
    let mut route = Routing {
        lapic: u32_at(madt, 36) as u64,
        ioapic: 0,
        gsi_base: 0,
        irq: [0, 1],
        flags: [0, 0],
    };
    let mut pos = 44;
    while pos + 2 <= len {
        let kind = unsafe { madt.add(pos).read() };
        let size = unsafe { madt.add(pos + 1).read() } as usize;
        if size < 2 || pos + size > len {
            return Err(Error::InvalidAcpi);
        }
        if kind == 1 && size >= 12 && route.ioapic == 0 {
            route.ioapic = u32_at(madt, pos + 4) as u64;
            route.gsi_base = u32_at(madt, pos + 8);
        }
        if kind == 2 && size >= 10 && unsafe { madt.add(pos + 2).read() } == 0 {
            let source = unsafe { madt.add(pos + 3).read() } as usize;
            if source < 2 {
                route.irq[source] = u32_at(madt, pos + 4);
                route.flags[source] = u16_at(madt, pos + 8);
            }
        }
        pos += size;
    }
    if route.ioapic == 0 || route.lapic == 0 {
        return Err(Error::MissingIoapic);
    }
    Ok(route)
}
fn msr_read(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32; // SAFETY: IA32_APIC_BASE is supported when CPUID reports APIC.
    unsafe {
        asm!("rdmsr",in("ecx")msr,out("eax")lo,out("edx")hi,options(nostack));
    }
    (lo as u64) | ((hi as u64) << 32)
}
fn msr_write(msr: u32, value: u64) {
    // SAFETY: only the APIC enable bit is changed on an APIC-capable CPU.
    unsafe {
        asm!("wrmsr",in("ecx")msr,in("eax")value as u32,in("edx")(value>>32) as u32,options(nostack));
    }
}
fn lapic_write(offset: usize, val: u32) {
    // SAFETY: LAPIC is mapped uncached in a dedicated page; offset is a documented register.
    unsafe {
        core::ptr::write_volatile((LAPIC as *mut u8).add(offset) as *mut u32, val);
    }
}
fn lapic_read(offset: usize) -> u32 {
    // SAFETY: LAPIC is mapped uncached in a dedicated page.
    unsafe { core::ptr::read_volatile((LAPIC as *mut u8).add(offset) as *const u32) }
}
fn io_write(reg: u32, val: u32) {
    // SAFETY: IOAPIC MMIO index/data registers are mapped and serialized on the bootstrap CPU.
    unsafe {
        core::ptr::write_volatile(IOAPIC, reg);
        core::ptr::write_volatile((IOAPIC as *mut u8).add(0x10) as *mut u32, val);
    }
}
fn io_read(reg: u32) -> u32 {
    // SAFETY: IOAPIC MMIO index/data registers are mapped and serialized on the bootstrap CPU.
    unsafe {
        core::ptr::write_volatile(IOAPIC, reg);
        core::ptr::read_volatile((IOAPIC as *mut u8).add(0x10) as *const u32)
    }
}
pub fn eoi() {
    lapic_write(0xb0, 0);
}
pub fn init(info: &NovaBootInfo, features: u64) -> Result<(), Error> {
    if features & cpuid::feature::APIC == 0 {
        return Err(Error::MissingApic);
    }
    let r = madt(info)?;
    let base = msr_read(0x1b) & 0xffff_f000;
    if base != r.lapic {
        return Err(Error::BadRouting);
    }
    paging::map_page(
        Page(VirtAddr(LAPIC_VA)),
        PhysFrame(PhysAddr(base)),
        PageFlags::DEVICE,
    )
    .map_err(|_| Error::Mapping)?;
    paging::map_page(
        Page(VirtAddr(IOAPIC_VA)),
        PhysFrame(PhysAddr(r.ioapic & !4095)),
        PageFlags::DEVICE,
    )
    .map_err(|_| Error::Mapping)?;
    // SAFETY: mapped APIC pages stay present for the kernel lifetime; no other CPU runs yet.
    unsafe {
        LAPIC = LAPIC_VA as *mut u32;
        IOAPIC = (IOAPIC_VA + (r.ioapic & 4095)) as *mut u32;
    }
    msr_write(0x1b, msr_read(0x1b) | (1 << 11));
    lapic_write(0x80, 0);
    lapic_write(0x320, 1 << 16);
    lapic_write(0xf0, 0x100 | 0xff);
    let version = io_read(1);
    let max = (version >> 16) & 0xff;
    let apic_id = lapic_read(0x20) >> 24;
    for irq in 0..2 {
        let gsi = r.irq[irq];
        if gsi < r.gsi_base || gsi - r.gsi_base > max {
            return Err(Error::BadRouting);
        }
        let pin = gsi - r.gsi_base;
        let mut low = (32 + irq) as u32;
        let polarity = r.flags[irq] & 3;
        let trigger = (r.flags[irq] >> 2) & 3;
        if polarity == 3 {
            low |= 1 << 13;
        } else if polarity == 2 {
            return Err(Error::BadRouting);
        }
        if trigger == 3 {
            low |= 1 << 15;
        } else if trigger == 2 {
            return Err(Error::BadRouting);
        }
        io_write(0x10 + pin * 2 + 1, apic_id << 24);
        io_write(0x10 + pin * 2, low);
    }
    // Mask legacy PIC so only IOAPIC routes hardware interrupts.
    cpu::port_out(0x21, 0xff);
    cpu::port_out(0xa1, 0xff);
    Ok(())
}
