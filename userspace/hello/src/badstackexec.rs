#![no_std]
#![no_main]
mod common;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let code = [0xc3u8; 8];
    // SAFETY: intentional test of NX on the mapped userspace stack.
    unsafe {
        core::arch::asm!("call {}",in(reg)code.as_ptr(),clobber_abi("sysv64"));
    }
    common::exit(99)
}
