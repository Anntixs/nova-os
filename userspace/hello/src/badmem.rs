#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // SAFETY: intentional user fault test; address zero is outside the user slot.
    unsafe {
        core::ptr::read_volatile(0x1000 as *const u64);
    }
    common::exit(99)
}
