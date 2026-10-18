pub mod elf;
pub mod stack;
use crate::{arch::x86_64::cpu, process};
pub extern "C" fn thread_entry() -> ! {
    let root=process::space_root().expect("no process address space");
    let (entry,stack)=crate::scheduler::current_user_entry_stack()
        .or_else(process::current_entry_stack).expect("no process entry");
    cpu::write_cr3(root);
    // SAFETY: the ELF loader mapped the executable entry and user stack. The
    // scheduler installed this thread's trusted kernel stack in TSS and SYSCALL.
    unsafe {crate::arch::x86_64::userspace::enter(entry,stack)}
}
