#![no_std]
#![no_main]
use core::{arch::asm, panic::PanicInfo};

#[inline(always)]
fn call(n: u64, a: u64, b: u64, c: u64) -> i64 {
    let ret: i64;
    // SAFETY: Nova ABI 0.1 uses SYSCALL with RAX number and RDI/RSI/RDX args.
    unsafe {
        asm!("syscall",inlateout("rax")n as i64=>ret,in("rdi")a,in("rsi")b,in("rdx")c,lateout("rcx")_,lateout("r11")_);
    }
    ret
}
fn write(bytes: &[u8]) {
    let _ = call(1, 1, bytes.as_ptr() as u64, bytes.len() as u64);
}
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let cs: u16;
    // SAFETY: reading CS is unprivileged and verifies execution at CPL 3.
    unsafe {
        asm!("mov {0:x}, cs",out(reg)cs,options(nomem,nostack));
    }
    if cs & 3 != 3 {
        loop {}
    }
    write(b"Hello from Nova userspace!\nPID: ");
    let pid = call(3, 0, 0, 0) as u64;
    let mut digits = [0u8; 20];
    let mut n = pid;
    let mut i = 20;
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    write(&digits[i..]);
    write(b"\n");
    let _ = call(2, 0, 0, 0);
    loop {}
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
