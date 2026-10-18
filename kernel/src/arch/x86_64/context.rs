use crate::thread::context::Context;
use core::arch::global_asm;

global_asm!(include_str!("context.S"));
unsafe extern "C" {
    fn nova_switch(old: *mut Context, new: *const Context);
}

/// Switches kernel stacks. The caller must own both contexts, keep them pinned, and
/// ensure no interrupt can observe a half-updated scheduler state.
pub unsafe fn switch(old: *mut Context, new: *const Context) {
    // SAFETY: the caller provides pinned, live contexts; assembly preserves all SysV
    // callee-saved registers and returns on the destination stack.
    unsafe { nova_switch(old, new) }
}
