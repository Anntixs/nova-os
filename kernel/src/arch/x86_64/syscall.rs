use core::arch::{asm, global_asm};
use super::percpu;

global_asm!(include_str!("syscall.S"),
    user_rsp_off=const percpu::USER_RSP_OFFSET,
    kernel_stack_off=const percpu::KERNEL_STACK_OFFSET,
);
unsafe extern "C" {
    static nova_syscall_entry: u8;
}
fn rdmsr(n: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: these architectural MSRs are available in x86_64 long mode.
    unsafe {
        asm!("rdmsr",in("ecx")n,out("eax")lo,out("edx")hi,options(nostack));
    }
    lo as u64 | ((hi as u64) << 32)
}
fn wrmsr(n: u32, v: u64) {
    // SAFETY: caller provides a validated x86_64 syscall MSR and selector layout.
    unsafe {
        asm!("wrmsr",in("ecx")n,in("eax")v as u32,in("edx")(v>>32) as u32,options(nostack));
    }
}
pub fn init() {
    // STAR high selector 0x10: SYSRET CS=(0x10+16)|3=0x23, SS=(0x10+8)|3=0x1b.
    let star = (0x10u64 << 48) | (0x08u64 << 32);
    wrmsr(0xc000_0081, star);
    wrmsr(0xc000_0082, &raw const nova_syscall_entry as u64);
    wrmsr(0xc000_0084, 1 << 9); // clear IF on entry
    wrmsr(0xc000_0080, rdmsr(0xc000_0080) | 1); // EFER.SCE
}
pub fn set_stack(top: u64) {
    percpu::set_kernel_stack(top);
}
