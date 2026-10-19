use super::{cpu, cpuid};
use crate::memory::{
    paging::{self, PageFlags},
    Page, PhysAddr, PhysFrame, VirtAddr,
};
use core::{arch::asm,sync::atomic::{AtomicBool,Ordering}};
use crate::platform::acpi;

const LAPIC_VA: u64 = 0xffff_9200_0000_0000;
const IOAPIC_VA: u64 = 0xffff_9200_0000_1000;
static mut LAPIC: *mut u32 = core::ptr::null_mut();
static mut IOAPIC: *mut u32 = core::ptr::null_mut();
static APIC_READY:AtomicBool=AtomicBool::new(false);
#[derive(Debug)]
pub enum Error {
    MissingApic,
    BadRouting,
    Mapping,
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
pub fn local_id() -> u8 { (lapic_read(0x20) >> 24) as u8 }
pub fn try_local_id()->Option<u8>{
    APIC_READY.load(Ordering::Acquire).then(local_id)
}
pub fn enable_local(){
    msr_write(0x1b,msr_read(0x1b)|(1<<11));
    lapic_write(0x80,0);
    lapic_write(0x320,1<<16);
    lapic_write(0xf0,0x100|0xff);
}
/// Send a physical-destination xAPIC IPI and wait for its delivery status.
/// This is used only by the BSP during sequential AP startup.
pub fn send_ipi(apic_id:u8, command:u32)->Result<(),Error>{
    for _ in 0..100_000 {if lapic_read(0x300)&(1<<12)==0 {break}}
    if lapic_read(0x300)&(1<<12)!=0{return Err(Error::BadRouting)}
    lapic_write(0x310,(apic_id as u32)<<24);
    lapic_write(0x300,command);
    for _ in 0..100_000 {if lapic_read(0x300)&(1<<12)==0 {return Ok(())}}
    Err(Error::BadRouting)
}
pub fn init(r: &acpi::Topology, features: u64) -> Result<(), Error> {
    if features & cpuid::feature::APIC == 0 {
        return Err(Error::MissingApic);
    }
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
    enable_local();
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
    APIC_READY.store(true,Ordering::Release);
    Ok(())
}
