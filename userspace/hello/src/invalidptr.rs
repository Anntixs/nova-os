#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let result = common::call(1, 1, 0x200000, 8);
    if result == -14 {
        common::write(b"Invalid pointer rejected\n");
        common::exit(0)
    }
    common::exit(99)
}
