use super::physical;
use super::{Page, PhysAddr, PhysFrame, VirtAddr};
use crate::arch::x86_64::cpu;
use core::arch::asm;
use core::sync::atomic::{AtomicBool,AtomicU64,Ordering};
static KERNEL_ROOT:AtomicU64=AtomicU64::new(0);
pub fn kernel_root()->u64 {KERNEL_ROOT.load(Ordering::Acquire)}
static NX_ENABLED:AtomicBool=AtomicBool::new(false);
pub fn nx_enabled()->bool {NX_ENABLED.load(Ordering::Acquire)}

const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
const PRESENT: u64 = 1;
const HUGE: u64 = 1 << 7;
#[derive(Clone, Copy)]
pub struct PageFlags(pub u64);
impl PageFlags {
    pub const WRITABLE: Self = Self(1 << 1);
    pub const USER: Self = Self(1 << 2);
    pub const NO_EXECUTE: Self = Self(1 << 63);
    pub const DEVICE: Self = Self((1 << 1) | (1 << 3) | (1 << 4));
    pub const KERNEL_RW_NX: Self = Self((1 << 1) | (1 << 63));
}
#[derive(Debug)]
pub enum Error {
    InvalidAddress,
    AlreadyMapped,
    NotMapped,
    HugePageConflict,
    NoIdentityMapping,
    OutOfFrames,
    FrameRollback,
}
fn canonical(a: u64) -> bool {
    let high = a >> 48;
    high == 0 || high == 0xffff
}
fn indexes(a: u64) -> [usize; 4] {
    [
        ((a >> 39) & 511) as usize,
        ((a >> 30) & 511) as usize,
        ((a >> 21) & 511) as usize,
        ((a >> 12) & 511) as usize,
    ]
}
fn table(phys: u64) -> *mut u64 {
    phys as *mut u64
}
fn read(tab: *mut u64, i: usize) -> u64 {
    // SAFETY: caller supplies an identity-mapped 4 KiB page table; i<512.
    unsafe { core::ptr::read_volatile(tab.add(i)) }
}
fn write(tab: *mut u64, i: usize, val: u64) {
    // SAFETY: caller supplies an identity-mapped 4 KiB page table; i<512.
    unsafe {
        core::ptr::write_volatile(tab.add(i), val);
    }
}
fn flush(addr: u64) {
    // SAFETY: invalidating this CPU's translation for the modified page is required after PTE edits.
    unsafe {
        asm!("invlpg [{}]",in(reg)addr,options(nostack,preserves_flags));
    }
}
pub fn enable_nx(supported: bool) -> bool {
    if !supported {
        return false;
    }
    let lo: u32;
    let hi: u32;
    // SAFETY: EFER is supported on x86_64; NXE bit is set only when CPUID reports NX.
    unsafe {
        asm!("rdmsr",in("ecx")0xc000_0080u32,out("eax")lo,out("edx")hi,options(nostack));
        let value = (lo as u64) | ((hi as u64) << 32) | (1 << 11);
        asm!("wrmsr",in("ecx")0xc000_0080u32,in("eax")value as u32,in("edx")(value>>32) as u32,options(nostack));
    }
    NX_ENABLED.store(true,Ordering::Release);
    true
}
pub fn translate(addr: VirtAddr) -> Option<PhysAddr> {
    if !canonical(addr.0) {
        return None;
    }
    let mut tab = table(cpu::read_cr3() & ADDR_MASK);
    let idx = indexes(addr.0);
    for level in 0..4 {
        let p = read(tab, idx[level]);
        if p & PRESENT == 0 {
            return None;
        }
        if p & HUGE != 0 && level == 1 {
            return Some(PhysAddr((p & ADDR_MASK) + (addr.0 & ((1 << 30) - 1))));
        }
        if p & HUGE != 0 && level == 2 {
            return Some(PhysAddr((p & ADDR_MASK) + (addr.0 & ((1 << 21) - 1))));
        }
        if level == 3 {
            return Some(PhysAddr((p & ADDR_MASK) + (addr.0 & 4095)));
        }
        tab = table(p & ADDR_MASK);
    }
    None
}
pub fn map_page(page: Page, frame: PhysFrame, flags: PageFlags) -> Result<(), Error> {
    let va = page.address();
    let pa = frame.address();
    if !canonical(va) || va & 4095 != 0 || pa & 4095 != 0 || pa & !ADDR_MASK != 0 {
        return Err(Error::InvalidAddress);
    }
    let idx = indexes(va);
    let mut tab = table(cpu::read_cr3() & ADDR_MASK);
    for level in 0..3 {
        let p = read(tab, idx[level]);
        if p & PRESENT == 0 {
            let f = physical::allocate_frame().map_err(|_| Error::OutOfFrames)?;
            if translate(VirtAddr(f.address())).map(|p| p.0) != Some(f.address()) {
                physical::free_allocated_frame(f).map_err(|_| Error::FrameRollback)?;
                return Err(Error::NoIdentityMapping);
            }
            // SAFETY: frame was freshly allocated and verified identity-mapped; zeroing it creates an empty page table.
            unsafe {
                core::ptr::write_bytes(f.address() as *mut u8, 0, 4096);
            }
            write(
                tab,
                idx[level],
                f.address() | PRESENT | PageFlags::WRITABLE.0 | (flags.0 & PageFlags::USER.0),
            );
            tab = table(f.address());
        } else {
            if p & HUGE != 0 {
                return Err(Error::HugePageConflict);
            }
            if flags.0 & PageFlags::USER.0 != 0 && p & PageFlags::USER.0 == 0 {
                return Err(Error::InvalidAddress);
            }
            tab = table(p & ADDR_MASK);
        }
    }
    if read(tab, idx[3]) & PRESENT != 0 {
        return Err(Error::AlreadyMapped);
    }
    write(tab, idx[3], pa | flags.0 | PRESENT);
    flush(va);
    Ok(())
}
pub fn unmap_page(page: Page) -> Result<PhysFrame, Error> {
    let va = page.address();
    if !canonical(va) || va & 4095 != 0 {
        return Err(Error::InvalidAddress);
    }
    let idx = indexes(va);
    let mut tab = table(cpu::read_cr3() & ADDR_MASK);
    for level in 0..3 {
        let p = read(tab, idx[level]);
        if p & PRESENT == 0 {
            return Err(Error::NotMapped);
        }
        if p & HUGE != 0 {
            return Err(Error::HugePageConflict);
        }
        tab = table(p & ADDR_MASK);
    }
    let p = read(tab, idx[3]);
    if p & PRESENT == 0 {
        return Err(Error::NotMapped);
    }
    write(tab, idx[3], 0);
    flush(va);
    Ok(PhysFrame(PhysAddr(p & ADDR_MASK)))
}
pub fn set_flags(page: Page, flags: PageFlags) -> Result<(), Error> {
    let va = page.address();
    if !canonical(va) || va & 4095 != 0 {
        return Err(Error::InvalidAddress);
    }
    let idx = indexes(va);
    let mut tab = table(cpu::read_cr3() & ADDR_MASK);
    for level in 0..3 {
        let p = read(tab, idx[level]);
        if p & PRESENT == 0 {
            return Err(Error::NotMapped);
        }
        if p & HUGE != 0 {
            return Err(Error::HugePageConflict);
        }
        tab = table(p & ADDR_MASK);
    }
    let p = read(tab, idx[3]);
    if p & PRESENT == 0 {
        return Err(Error::NotMapped);
    }
    write(tab, idx[3], (p & ADDR_MASK) | flags.0 | PRESENT);
    flush(va);
    Ok(())
}
pub fn init() -> Result<bool, Error> {
    let old = cpu::read_cr3() & ADDR_MASK;
    if translate(VirtAddr(old)).map(|p| p.0) != Some(old) {
        return Err(Error::NoIdentityMapping);
    }
    let fresh = physical::allocate_frame().map_err(|_| Error::OutOfFrames)?;
    {
        use core::fmt::Write;
        let mut d = crate::Diagnostic;
        let _ = writeln!(d, "[NOVA][MEM] New PML4 frame: {:#x}", fresh.address());
    }
    if translate(VirtAddr(fresh.address())).map(|p| p.0) != Some(fresh.address()) {
        return Err(Error::NoIdentityMapping);
    }
    // SAFETY: old and fresh are mapped 4 KiB pages; fresh is exclusively owned by Nova.
    // Copying firmware's PML4 preserves all working identity and framebuffer mappings.
    unsafe {
        core::ptr::copy_nonoverlapping(old as *const u8, fresh.address() as *mut u8, 4096);
    }
    cpu::write_cr3(fresh.address());
    KERNEL_ROOT.store(fresh.address(),Ordering::Release);
    Ok(true)
}
