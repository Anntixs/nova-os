#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // SAFETY: intentional test of ELF PF_X without PF_W in Ring 3.
    unsafe {
        core::ptr::write_volatile(_start as usize as *mut u8, 0x90);
    }
    common::exit(99)
}
