//! Validated ACPI discovery for the xAPIC platform path. Firmware tables are
//! identity mapped by NovaBoot and retained for the kernel lifetime.
use nova_core::NovaBootInfo;
use crate::memory::{paging,VirtAddr};

#[derive(Clone, Copy, Debug)]
pub struct Processor { pub apic_id: u8, pub acpi_id: u8 }
#[derive(Debug)]
pub enum Error { MissingAcpi, InvalidAcpi, MissingIoapic, TooManyProcessors, NoProcessors }
pub struct Topology {
    pub processors: [Option<Processor>; 64],
    pub processor_count: usize,
    pub lapic: u64,
    pub ioapic: u64,
    pub gsi_base: u32,
    pub irq: [u32; 2],
    pub flags: [u16; 2],
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
fn mapped_firmware_range(info:&NovaBootInfo,address:u64,len:usize)->bool{
    let Some(end)=address.checked_add(len as u64)else{return false};
    if address==0||end<=address||info.memory_descriptor_size<40{return false}
    let count=info.memory_map_size/info.memory_descriptor_size;
    let mut covered=false;
    for i in 0..count {
        let descriptor=(info.memory_map+i*info.memory_descriptor_size) as *const u8;
        // SAFETY: BootInfo's descriptor array was validated by physical::init
        // before ACPI discovery; offsets 0, 8 and 24 fit the descriptor stride.
        let (kind,base,pages)=unsafe{(
            core::ptr::read_unaligned(descriptor as *const u32),
            core::ptr::read_unaligned(descriptor.add(8) as *const u64),
            core::ptr::read_unaligned(descriptor.add(24) as *const u64))};
        if (1..=10).contains(&kind)&&address>=base&&base.checked_add(pages.saturating_mul(4096)).is_some_and(|last|end<=last){covered=true;break}
    }
    if !covered{return false}
    let mut cursor=address;
    while cursor<end {
        if paging::translate(VirtAddr(cursor)).map(|p|p.0)!=Some(cursor){return false}
        cursor=cursor.checked_add(4096-(cursor&4095)).unwrap_or(end).min(end);
    }
    true
}
fn sdt(info:&NovaBootInfo,p: *const u8) -> Result<usize, Error> {
    if p.is_null() {
        return Err(Error::InvalidAcpi);
    }
    if !mapped_firmware_range(info,p as u64,36){return Err(Error::InvalidAcpi)}
    let len = u32_at(p, 4) as usize;
    if !(36..=1024 * 1024).contains(&len) || !mapped_firmware_range(info,p as u64,len) || !checksum(p, len) {
        return Err(Error::InvalidAcpi);
    }
    Ok(len)
}
pub fn discover(info: &NovaBootInfo) -> Result<Topology, Error> {
    if info.rsdp == 0 {
        return Err(Error::MissingAcpi);
    }
    let r = info.rsdp as *const u8;
    if !mapped_firmware_range(info,info.rsdp,20){return Err(Error::InvalidAcpi)}
    // SAFETY: NovaBoot passes an ACPI RSDP pointer from the firmware configuration table.
    if unsafe { core::slice::from_raw_parts(r, 8) } != b"RSD PTR " || !checksum(r, 20) {
        return Err(Error::InvalidAcpi);
    }
    let revision = unsafe { r.add(15).read() };
    let (root, entry_size) = if revision >= 2 {
        if !mapped_firmware_range(info,info.rsdp,36){return Err(Error::InvalidAcpi)}
        let length = u32_at(r, 20) as usize;
        if !(36..=4096).contains(&length) || !mapped_firmware_range(info,info.rsdp,length) || !checksum(r, length) {
            return Err(Error::InvalidAcpi);
        }
        (u64_at(r, 24), 8usize)
    } else {
        (u32_at(r, 16) as u64, 4usize)
    };
    let root = root as *const u8;
    let root_len = sdt(info,root)?;
    // SAFETY: `sdt` validated the full common header before reading its signature.
    let signature = unsafe { core::slice::from_raw_parts(root, 4) };
    if signature != if entry_size == 8 { b"XSDT" } else { b"RSDT" }
        || (root_len - 36) % entry_size != 0
    {
        return Err(Error::InvalidAcpi);
    }
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
        } // SAFETY: `sdt` checks the firmware-backed range before the signature read.
        sdt(info,p)?;
        if unsafe { core::slice::from_raw_parts(p, 4) } == b"APIC" {
            madt = p;
            break;
        }
    }
    if madt.is_null() {
        return Err(Error::MissingIoapic);
    }
    let len = sdt(info,madt)?;
    if len < 44 {
        return Err(Error::InvalidAcpi);
    }
    let mut route = Topology {
        processors: [None; 64],
        processor_count: 0,
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
        if kind == 0 && size >= 8 {
            let apic_id = unsafe { madt.add(pos + 3).read() };
            let enabled = u32_at(madt, pos + 4) & 3 != 0;
            if enabled && !route.processors[..route.processor_count].iter().flatten().any(|cpu| cpu.apic_id == apic_id) {
                if route.processor_count == route.processors.len() { return Err(Error::TooManyProcessors); }
                route.processors[route.processor_count] = Some(Processor { apic_id, acpi_id: unsafe { madt.add(pos + 2).read() } });
                route.processor_count += 1;
            }
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
    if route.processor_count == 0 { return Err(Error::NoProcessors); }
    Ok(route)
}
