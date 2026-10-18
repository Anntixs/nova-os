#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let result = common::call(999, 0, 0, 0);
    if result == -38 {
        common::write(b"Invalid syscall rejected\n");
        common::exit(0)
    }
    common::exit(99)
}
