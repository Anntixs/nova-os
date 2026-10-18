use core::arch::asm;

pub fn disable_interrupts() {
    // SAFETY: kernel ring 0 may change IF; callers ensure IRQ handlers are installed before re-enabling.
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}
pub fn enable_interrupts() {
    // SAFETY: caller must have installed an IDT and configured interrupt controllers.
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    // SAFETY: pushfq only reads this CPU's flags.
    unsafe {
        asm!("pushfq", "pop {}", out(reg) flags, options(nomem));
    }
    flags & (1 << 9) != 0
}
pub fn halt() {
    // SAFETY: HLT waits for an interrupt; valid in ring 0.
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}
pub fn halt_forever() -> ! {
    disable_interrupts();
    loop {
        halt();
    }
}
pub fn idle() {
    loop {
        halt();
    }
}
pub fn read_cr2() -> u64 {
    let v: u64; // SAFETY: read-only privileged register in ring 0.
    unsafe {
        asm!("mov {}, cr2", out(reg) v, options(nomem,nostack));
    }
    v
}
pub fn read_cr3() -> u64 {
    let v: u64; // SAFETY: read-only privileged register in ring 0.
    unsafe {
        asm!("mov {}, cr3", out(reg) v, options(nomem,nostack));
    }
    v
}
pub fn port_out(p: u16, v: u8) {
    // SAFETY: caller owns the I/O port; Nova runs at CPL0 with I/O permission.
    unsafe {
        asm!("out dx, al", in("dx") p, in("al") v, options(nomem,nostack));
    }
}
pub fn port_in(p: u16) -> u8 {
    let v: u8; // SAFETY: caller owns the I/O port; Nova runs at CPL0 with I/O permission.
    unsafe {
        asm!("in al, dx", in("dx") p, out("al") v, options(nomem,nostack));
    }
    v
}
pub fn port_out16(p: u16, v: u16) {
    // SAFETY: caller owns the I/O port and supplies a valid 16-bit register offset.
    unsafe { asm!("out dx, ax", in("dx") p, in("ax") v, options(nomem, nostack)); }
}
pub fn port_in16(p: u16) -> u16 {
    let v: u16;
    // SAFETY: caller owns the I/O port and supplies a valid 16-bit register offset.
    unsafe { asm!("in ax, dx", in("dx") p, out("ax") v, options(nomem, nostack)); }
    v
}
pub fn port_out32(p: u16, v: u32) {
    // SAFETY: caller owns the I/O port and supplies a valid 32-bit register offset.
    unsafe { asm!("out dx, eax", in("dx") p, in("eax") v, options(nomem, nostack)); }
}
pub fn port_in32(p: u16) -> u32 {
    let v: u32;
    // SAFETY: caller owns the I/O port and supplies a valid 32-bit register offset.
    unsafe { asm!("in eax, dx", in("dx") p, out("eax") v, options(nomem, nostack)); }
    v
}
pub fn write_cr3(value: u64) {
    // SAFETY: caller must pass a 4 KiB aligned physical address of a valid PML4.
    unsafe {
        asm!("mov cr3, {}",in(reg)value,options(nostack,preserves_flags));
    }
}
