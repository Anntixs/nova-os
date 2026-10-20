use crate::{
    arch::x86_64::{context, cpu},
    process::ProcessId,
    thread::{context::Context, Thread, ThreadId, ThreadState},
};
use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{cell::UnsafeCell,sync::atomic::{AtomicBool,Ordering}};

struct Scheduler {
    cpus: [CpuRun; 8],
    threads: Vec<Option<Box<Thread>>>,
    ready: VecDeque<usize>,
    next_tid: u64,
}
struct CpuRun {
    host: Context,
    host_cr3: u64,
    current: Option<usize>,
}
impl CpuRun {
    const fn new() -> Self { Self { host: Context::new(0), host_cr3: 0, current: None } }
}
fn cpu_index() -> usize { crate::arch::x86_64::percpu::current_logical() }
struct Cell(UnsafeCell<Option<Scheduler>>,AtomicBool);
// SAFETY: the atomic lock and local IRQ masking serialize all scheduler state.
// Each CPU has its own host/current slot. AP scheduling is still disabled:
// process ownership and the context-switch handoff are not SMP-safe yet.
unsafe impl Sync for Cell {}
static SCHED: Cell = Cell(UnsafeCell::new(None),AtomicBool::new(false));

#[cfg(feature = "selftest")]
static TEST_STEP: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "selftest")]
fn test_step(expected: usize, label: &str) {
    use core::sync::atomic::Ordering;
    let actual = TEST_STEP.fetch_add(1, Ordering::SeqCst);
    assert_eq!(actual, expected, "scheduler order");
    crate::log(label);
}
#[cfg(feature = "selftest")]
extern "C" fn test_a() -> ! {
    // Model an event delivered between wait registration and the park call.
    // A lost notification would block this thread and stall the self-test.
    wake(ProcessId(0));
    block_current();
    test_step(0, "[NOVA][TEST] A1");
    yield_current();
    test_step(2, "[NOVA][TEST] A2");
    yield_current();
    test_step(4, "[NOVA][TEST] A3");
    exit_current()
}
#[cfg(feature = "selftest")]
extern "C" fn test_b() -> ! {
    test_step(1, "[NOVA][TEST] B1");
    yield_current();
    test_step(3, "[NOVA][TEST] B2");
    yield_current();
    test_step(5, "[NOVA][TEST] B3");
    exit_current()
}
#[cfg(feature = "selftest")]
pub fn selftest() {
    spawn(ProcessId(0), test_a);
    spawn(ProcessId(0), test_b);
    run();
    assert_eq!(TEST_STEP.load(core::sync::atomic::Ordering::SeqCst), 6);
    crate::log("[NOVA][TEST] scheduler round robin OK");
}

fn with_sched<R>(f: impl FnOnce(&mut Scheduler) -> R) -> R {
    let enabled = cpu::interrupts_enabled();
    cpu::disable_interrupts();
    while SCHED.1.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
    // SAFETY: atomic lock excludes other CPUs and local IRQs cannot reenter.
    let result = f(unsafe {
        (*SCHED.0.get())
            .as_mut()
            .expect("scheduler not initialized")
    });
    SCHED.1.store(false,Ordering::Release);
    if enabled {
        cpu::enable_interrupts();
    }
    result
}
pub fn init() {
    cpu::disable_interrupts();
    // SAFETY: called once on the bootstrap CPU before the scheduler and APs
    // access this cell. CPU 0's host CR3 is captured before it runs threads.
    unsafe {
        let mut cpus = core::array::from_fn(|_| CpuRun::new());
        cpus[0].host_cr3 = cpu::read_cr3();
        *SCHED.0.get() = Some(Scheduler {
            cpus,
            threads: Vec::new(),
            ready: VecDeque::new(),
            next_tid: 1,
        });
    }
}
pub fn spawn(pid: ProcessId, entry: extern "C" fn() -> !) -> ThreadId {
    let tid = with_sched(|s| {
        let tid = ThreadId(s.next_tid);
        s.next_tid += 1;
        let mut t = Box::new(Thread::new(tid, pid, entry));
        t.state = ThreadState::Ready;
        s.threads.push(Some(t));
        s.ready.push_back(s.threads.len() - 1);
        tid
    });
    crate::process::add_thread(pid, tid);
    tid
}
pub fn spawn_user(pid:ProcessId,entry:u64,stack:u64)->ThreadId {
    let tid=with_sched(|s|{
        let tid=ThreadId(s.next_tid);s.next_tid+=1;
        let mut thread=Box::new(Thread::new(tid,pid,crate::userspace::thread_entry));
        thread.user_entry_stack=Some((entry,stack));thread.state=ThreadState::Ready;
        s.threads.push(Some(thread));s.ready.push_back(s.threads.len()-1);tid
    });
    crate::process::add_thread(pid,tid);tid
}
pub fn current_user_entry_stack()->Option<(u64,u64)>{with_sched(|s|{
    let index=s.cpus[cpu_index()].current?;s.threads[index].as_ref()?.user_entry_stack
})}
pub fn current_tid()->Option<ThreadId>{with_sched(|s|{
    let index=s.cpus[cpu_index()].current?;Some(s.threads[index].as_ref()?.tid)
})}
pub fn try_current_tid()->Option<ThreadId>{
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 if SCHED.1.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){
  if enabled{cpu::enable_interrupts()}return None
 }
 // SAFETY: lock excludes writers; scheduler may not exist during bootstrap.
 let tid=unsafe{(*SCHED.0.get()).as_ref().and_then(|s|s.cpus[cpu_index()].current.and_then(|i|s.threads.get(i)?.as_ref().map(|t|t.tid)))};
 SCHED.1.store(false,Ordering::Release);
 if enabled{cpu::enable_interrupts()}tid
}
pub fn other_live_threads(pid:ProcessId)->bool{with_sched(|s|{
 s.threads.iter().enumerate().any(|(i,slot)|!s.cpus.iter().any(|cpu|cpu.current==Some(i))&&slot.as_ref().is_some_and(|t|t.pid==pid&&!matches!(t.state,ThreadState::Exited|ThreadState::Faulted)))
})}
pub fn run() {
    loop {
        let pair = with_sched(|s| {
            let i = s.ready.pop_front()?;
            s.cpus[cpu_index()].current = Some(i);
            let thread=s.threads[i].as_mut().expect("ready thread missing");
            thread.state = ThreadState::Running;
            crate::process::mark_running(thread.pid);
            let top = thread.kernel_stack_top();
            crate::arch::x86_64::gdt::set_rsp0(top);
            crate::arch::x86_64::syscall::set_stack(top);
            Some((&raw mut s.cpus[cpu_index()].host, &raw const thread.context))
        });
        let Some((old, new)) = pair else {
            crate::tty::service();
            crate::input::service_raw();
            crate::process::wake_sleepers(crate::interrupts::timer::uptime_ms());
            if has_ready(){continue}
            let blocked=with_sched(|s|s.threads.iter().any(|t|t.as_ref().is_some_and(|t|t.state==ThreadState::Blocked)));
            if !blocked {break}
            if !has_ready(){cpu::enable_interrupts();cpu::halt();}
            continue;
        };
        let enabled = cpu::interrupts_enabled();
        cpu::disable_interrupts();
        if let Some(root) = crate::process::space_root() {
            cpu::write_cr3(root);
        }
        // SAFETY: boxed thread contexts are pinned until the run loop finishes; the
        // scheduler has no mutable borrow outstanding across this stack switch.
        unsafe {
            context::switch(old, new);
        }
        let host_cr3 = with_sched(|s| s.cpus[cpu_index()].host_cr3);
        cpu::write_cr3(host_cr3);
        if enabled {
            cpu::enable_interrupts();
        }
    }
    with_sched(|s| {
        for slot in &mut s.threads {if slot.as_ref().is_some_and(|t|t.pid.0==0 && t.state==ThreadState::Exited){*slot=None}}
    });
}
pub fn yield_current() {
    let (old, new) = with_sched(|s| {
        let i = s.cpus[cpu_index()].current.take().expect("yield without running thread");
        let thread=s.threads[i].as_mut().expect("current thread missing");
        thread.state = ThreadState::Ready;
        crate::process::mark_ready(thread.pid);
        s.ready.push_back(i);
        (&raw mut thread.context, &raw const s.cpus[cpu_index()].host)
    });
    // SAFETY: both contexts are pinned and live. The scheduler state is committed
    // before the transfer, and no Rust reference spans it.
    unsafe {
        context::switch(old, new);
    }
}
pub fn exit_current() -> ! {
    let (old, new) = with_sched(|s| {
        let i = s.cpus[cpu_index()].current.take().expect("exit without running thread");
        let thread=s.threads[i].as_mut().expect("current thread missing");
        thread.state = ThreadState::Exited;
        (&raw mut thread.context, &raw const s.cpus[cpu_index()].host)
    });
    // SAFETY: the exited stack remains allocated until the run loop returns.
    unsafe {
        context::switch(old, new);
    }
    unreachable!("exited thread resumed")
}

pub fn block_current() {
 let pair=with_sched(|s|{
  let i=s.cpus[cpu_index()].current.expect("block without running thread");
  if s.threads[i].as_ref().expect("current thread missing").wake_pending {
   s.threads[i].as_mut().unwrap().wake_pending=false;
   return None;
  }
  s.cpus[cpu_index()].current=None;
  let thread=s.threads[i].as_mut().expect("current thread missing");
  thread.state=ThreadState::Blocked;
  thread.waiting_for=None;
  crate::process::mark_blocked(thread.pid);
  Some((&raw mut thread.context,&raw const s.cpus[cpu_index()].host))
 });
 let Some((old,new))=pair else{return};
 // SAFETY: the blocked thread owns its pinned kernel stack. Its context remains
 // in the scheduler until a wake event queues it again.
 unsafe{context::switch(old,new)}
}
pub fn exit_thread()->! {
 let (old,new)=with_sched(|s|{
  let i=s.cpus[cpu_index()].current.take().expect("thread exit without running thread");
  let tid=s.threads[i].as_ref().unwrap().tid;
  s.threads[i].as_mut().unwrap().state=ThreadState::Exited;
  for (j,slot) in s.threads.iter_mut().enumerate(){
   if let Some(thread)=slot.as_mut(){if thread.state==ThreadState::Blocked&&thread.waiting_for==Some(tid){
    thread.waiting_for=None;thread.state=ThreadState::Ready;s.ready.push_back(j);
   }}
  }
  (&raw mut s.threads[i].as_mut().unwrap().context,&raw const s.cpus[cpu_index()].host)
 });
 // SAFETY: the exited thread's context and kernel stack remain pinned until JOIN
 // reaps them. The host scheduler context remains live on the BSP stack.
 unsafe{context::switch(old,new)}
 unreachable!("exited thread resumed")
}
pub fn join(tid:ThreadId)->Result<(),i64>{
 let pid=crate::process::current_pid().ok_or(-3i64)?;
 loop {
  // Inspect the target and register the waiter under the same lock. Otherwise
  // the target can exit between those steps and its wakeup is lost forever.
  let pending=with_sched(|s|{
   let current=s.cpus[cpu_index()].current.ok_or(-3i64)?;
   if s.threads[current].as_ref().unwrap().tid==tid{return Err(-22i64)}
   let index=s.threads.iter().position(|slot|slot.as_ref().is_some_and(|t|t.tid==tid&&t.pid==pid)).ok_or(-3i64)?;
   if s.threads[index].as_ref().unwrap().state==ThreadState::Exited {
    s.threads[index]=None;return Ok(None)
   }
   let thread=s.threads[current].as_mut().expect("join without running thread");
   thread.state=ThreadState::Blocked;
   thread.waiting_for=Some(tid);
   crate::process::mark_blocked(pid);
   s.cpus[cpu_index()].current=None;
   Ok::<Option<(*mut Context,*const Context)>,i64>(Some((&raw mut thread.context,&raw const s.cpus[cpu_index()].host)))
  })?;
  let Some((old,new))=pending else {crate::process::remove_thread(tid);return Ok(())};
  // SAFETY: the blocked thread's pinned context remains owned by the scheduler
  // until the joined thread exits and queues it again.
  unsafe{context::switch(old,new)}
 }
}
pub fn kill_process_threads(pid:ProcessId){with_sched(|s|{
 for (i,slot) in s.threads.iter_mut().enumerate(){
  if !s.cpus.iter().any(|cpu|cpu.current==Some(i)){if let Some(t)=slot.as_mut(){if t.pid==pid{t.state=ThreadState::Exited}}}
 }
 s.ready.retain(|i|s.threads[*i].as_ref().is_some_and(|t|t.state==ThreadState::Ready));
})}
pub fn wake(pid:ProcessId) {
 with_sched(|s|{
  for (i,slot) in s.threads.iter_mut().enumerate(){
   if let Some(t)=slot.as_mut(){if t.pid==pid {
    if t.state==ThreadState::Blocked {
     t.state=ThreadState::Ready;s.ready.push_back(i);crate::process::mark_ready(pid);
    } else if t.state==ThreadState::Running {
     t.wake_pending=true;
    }
   }}
  }
 });
}
pub fn reap(pid:ProcessId) {
 with_sched(|s|{for slot in &mut s.threads{if slot.as_ref().is_some_and(|t|t.pid==pid&&t.state==ThreadState::Exited){*slot=None}}});
}
pub fn has_ready()->bool {with_sched(|s|!s.ready.is_empty())}
/// Called only after the timer's EOI, on the interrupted thread's trusted
/// kernel stack. The IRQ stub preserves volatile GPRs and FPU state; this
/// context resumes inside the same IRQ handler and eventually executes IRETQ.
pub fn preempt_if_running() {
    let should_switch = with_sched(|s| s.cpus[cpu_index()].current.is_some() && !s.ready.is_empty());
    if should_switch { yield_current(); }
}
