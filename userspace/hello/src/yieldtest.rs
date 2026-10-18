#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    common::write(b"Before yield\n");
    let _ = common::call(4, 0, 0, 0);
    common::write(b"After yield\n");
    common::exit(0)
}
