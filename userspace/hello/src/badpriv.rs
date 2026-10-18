#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // SAFETY: intentional Ring 3 privilege fault test.
    unsafe {
        core::arch::asm!("cli");
    }
    common::exit(99)
}
