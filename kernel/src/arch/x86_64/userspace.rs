use core::arch::global_asm;
global_asm!(include_str!("userspace.S"));
unsafe extern "C" {
    fn nova_enter_user(entry: u64, stack: u64) -> !;
}
pub unsafe fn enter(entry: u64, stack: u64) -> ! {
    // SAFETY: caller installed a user page table, valid user CS/SS and TSS.RSP0;
    // entry and stack lie in mapped user pages.
    unsafe { nova_enter_user(entry, stack) }
}
