pub mod context;
use crate::process::ProcessId;
use alloc::boxed::Box;
use context::Context;

pub const KERNEL_STACK_SIZE: usize = 32 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadState {
    Created,
    Ready,
    Running,
    Blocked,
    Exited,
    Faulted,
}

pub struct Thread {
    pub tid: ThreadId,
    pub pid: ProcessId,
    pub state: ThreadState,
    pub context: Context,
    pub kernel_stack: Box<[u8; KERNEL_STACK_SIZE]>,
    pub user_entry_stack: Option<(u64,u64)>,
    pub waiting_for: Option<ThreadId>,
    /// A wakeup delivered while this thread is still running. Consumed by the
    /// next block attempt so an event cannot be lost between enqueue and park.
    pub wake_pending: bool,
}
impl Thread {
    pub fn new(tid: ThreadId, pid: ProcessId, entry: extern "C" fn() -> !) -> Self {
        let mut stack = Box::new([0u8; KERNEL_STACK_SIZE]);
        let top = ((stack.as_mut_ptr() as usize + KERNEL_STACK_SIZE) & !15) as *mut u64;
        // SAFETY: top is inside the owned 32 KiB stack. The saved RSP points to a return
        // address for the first context switch, leaving System V stack alignment at entry.
        unsafe {
            top.sub(2).write(entry as usize as u64);
            top.sub(1).write(0);
        }
        Self {
            tid,
            pid,
            state: ThreadState::Created,
            context: Context::new(top as u64 - 16),
            kernel_stack: stack,
            user_entry_stack: None,
            waiting_for: None,
            wake_pending: false,
        }
    }
    pub fn kernel_stack_top(&self) -> u64 {
        ((self.kernel_stack.as_ptr() as usize + KERNEL_STACK_SIZE) & !15) as u64
    }
}
