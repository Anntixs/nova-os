#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // SAFETY: intentional invalid-opcode test in CPL 3.
    unsafe {
        core::arch::asm!("ud2", options(noreturn));
    }
}
