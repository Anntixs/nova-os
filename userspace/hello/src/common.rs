use core::{arch::asm, panic::PanicInfo};
pub fn call(n: u64, a: u64, b: u64, c: u64) -> i64 {
    let ret: i64;
    // SAFETY: Nova ABI 0.1; kernel preserves all registers except RAX, RCX and R11.
    unsafe {
        asm!("syscall",inlateout("rax")n as i64=>ret,in("rdi")a,in("rsi")b,in("rdx")c,lateout("rcx")_,lateout("r11")_);
    }
    ret
}
pub fn write(bytes: &[u8]) {
    let _ = call(1, 1, bytes.as_ptr() as u64, bytes.len() as u64);
}
pub fn exit(code: i32) -> ! {
    let _ = call(2, code as u64, 0, 0);
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
