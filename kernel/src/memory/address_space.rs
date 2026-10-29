use super::{
    paging::{self, PageFlags},
    physical, Page, PhysAddr, PhysFrame, VirtAddr,
};
use crate::arch::x86_64::cpu;
use nova_core::NovaBootInfo;

const PRESENT: u64 = 1;
const USER: u64 = 4;
const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
pub const USER_BASE: u64 = 0x0000_4000_0000_0000;
pub const USER_END: u64 = USER_BASE + (1u64 << 39);
pub const USER_STACK_TOP: u64 = USER_BASE + 0x1000_0000;
const SLOT: usize = (USER_BASE >> 39) as usize;

pub struct AddressSpace {
    pub root: PhysFrame,
}
impl AddressSpace {
    pub fn new() -> Result<Self, &'static str> {
        // Child creation can occur during a syscall while a parent's PML4 is active.
        // Clone the permanent supervisor root, whose user slot is empty.
        let source = paging::kernel_root() & ADDR_MASK;
        // SAFETY: the active PML4 is identity mapped and SLOT is within its 512 entries.
        if unsafe { (source as *const u64).add(SLOT).read() } & PRESENT != 0 {
            return Err("user slot occupied");
        }
        let root = physical::allocate_frame().map_err(|_| "no PML4 frame")?;
        // SAFETY: both frames are identity mapped; destination is exclusively owned.
        unsafe {
            core::ptr::copy_nonoverlapping(source as *const u8, root.address() as *mut u8, 4096);
            let entries = root.address() as *mut u64;
            for i in 0..512 {
                let p = entries.add(i);
                p.write(p.read() & !USER);
            }
        }
        Ok(Self { root })
    }
    pub fn activate(&self) {
        cpu::write_cr3(self.root.address());
    }
    pub fn map_user(&self, va: u64, flags: PageFlags) -> Result<PhysFrame, &'static str> {
        if va < USER_BASE || va >= USER_END || va & 4095 != 0 || flags.0 & USER == 0 {
            return Err("invalid user page");
        }
        let frame = physical::allocate_frame().map_err(|_| "no user frame")?;
        let old = cpu::read_cr3();
        let iflag = cpu::interrupts_enabled();
        cpu::disable_interrupts();
        self.activate();
        let result = paging::map_page(Page(VirtAddr(va)), frame, flags);
        cpu::write_cr3(old);
        if iflag {
            cpu::enable_interrupts();
        }
        if result.is_err() {
            physical::free_allocated_frame(frame).map_err(|_| "user frame rollback failed")?;
            return Err("user mapping failed");
        }
        // SAFETY: frame is freshly allocated, identity mapped and owned by this address space.
        unsafe {
            core::ptr::write_bytes(frame.address() as *mut u8, 0, 4096);
        }
        Ok(frame)
    }
    pub fn translate_user(&self, va: u64, write: bool) -> Option<u64> {
        if !(USER_BASE..USER_END).contains(&va) {
            return None;
        }
        let idx = [
            ((va >> 39) & 511) as usize,
            ((va >> 30) & 511) as usize,
            ((va >> 21) & 511) as usize,
            ((va >> 12) & 511) as usize,
        ];
        let mut table = self.root.address() as *const u64;
        for (level, i) in idx.into_iter().enumerate() {
            // SAFETY: page-table frames are identity mapped conventional RAM.
            let p = unsafe { table.add(i).read_volatile() };
            if p & (PRESENT | USER) != PRESENT | USER || (write && level == 3 && p & 2 == 0) {
                return None;
            }
            if p & (1 << 7) != 0 && level != 3 {
                return None;
            }
            if level == 3 {
                return Some((p & ADDR_MASK) + (va & 4095));
            }
            table = (p & ADDR_MASK) as *const u64;
        }
        None
    }
    pub fn unmap_user(&self,va:u64)->Result<(),&'static str>{
        let frame = self.detach_user(va)?;
        physical::free_allocated_frame(frame).map_err(|_|"user frame release failed")
    }
    /// Remove a PTE without returning its frame to the allocator. The caller
    /// must invalidate remote TLBs before freeing the returned frame.
    pub fn detach_user(&self,va:u64)->Result<PhysFrame,&'static str>{
        if va<USER_BASE||va>=USER_END||va&4095!=0{return Err("invalid user unmap")}
        let old=cpu::read_cr3();let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
        self.activate();let result=paging::unmap_page(Page(VirtAddr(va)));
        cpu::write_cr3(old);if enabled{cpu::enable_interrupts()}
        result.map_err(|_|"user page not mapped")
    }
    pub fn destroy(self, info: &NovaBootInfo) -> Result<(), &'static str> {
        if cpu::read_cr3() & ADDR_MASK == self.root.address() {
            return Err("active address space");
        }
        // SAFETY: the stopped process exclusively owns the user slot. Kernel slots are shared.
        unsafe {
            let root = self.root.address() as *mut u64;
            let p = root.add(SLOT).read();
            if p & PRESENT != 0 {
                free_tree(p & ADDR_MASK, 3, info)?;
            }
        }
        physical::free_frame(info, self.root).map_err(|_| "PML4 free failed")
    }
}
unsafe fn free_tree(table: u64, levels: usize, info: &NovaBootInfo) -> Result<(), &'static str> {
    for i in 0..512 {
        // SAFETY: this page table belongs exclusively to the stopped process.
        let p = unsafe { (table as *const u64).add(i).read() };
        if p & PRESENT == 0 {
            continue;
        }
        let frame = PhysFrame(PhysAddr(p & ADDR_MASK));
        if levels > 1 {
            unsafe {
                free_tree(frame.address(), levels - 1, info)?;
            }
        } else {
            physical::free_frame(info, frame).map_err(|e| {
                use core::fmt::Write;
                let _ = writeln!(
                    crate::Diagnostic,
                    "[NOVA][MEM] user free {:#x} depth {}: {:?}",
                    frame.address(),
                    levels,
                    e
                );
                "user frame free failed"
            })?;
        }
    }
    physical::free_frame(info, PhysFrame(PhysAddr(table))).map_err(|_| "user table free failed")
}

#[cfg(feature = "selftest")]
pub fn selftest(info: &NovaBootInfo) {
    let before = physical::stats().free_bytes;
    let space = AddressSpace::new().expect("new address space");
    let frame = space
        .map_user(
            USER_BASE + 0x1000,
            PageFlags(PageFlags::USER.0 | PageFlags::WRITABLE.0 | PageFlags::NO_EXECUTE.0),
        )
        .expect("map user page");
    assert_eq!(
        space.translate_user(USER_BASE + 0x1000, true),
        Some(frame.address())
    );
    assert_eq!(space.translate_user(USER_BASE + 0x2000, false), None);
    assert_eq!(paging::translate(VirtAddr(USER_BASE + 0x1000)), None);
    space.destroy(info).expect("destroy address space");
    assert_eq!(physical::stats().free_bytes, before);
    crate::log("[NOVA][TEST] user address space isolation and cleanup OK");
}
