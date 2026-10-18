#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // SAFETY: intentional test that the identity-mapped kernel image is supervisor-only.
    unsafe {
        core::ptr::read_volatile(0x200000 as *const u64);
    }
    common::exit(99)
}
