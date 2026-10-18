#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    common::exit(42)
}
