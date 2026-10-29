use crate::{
    arch::x86_64::{context, cpu},
    process::ProcessId,
    thread::{context::Context, Thread, ThreadId, ThreadState},
};
use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{cell::UnsafeCell,sync::atomic::{AtomicBool,AtomicU64,Ordering}};

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
    pending: Option<ThreadState>,
}
impl CpuRun {
    const fn new() -> Self { Self { host: Context::new(0), host_cr3: 0, current: None, pending: None } }
}
fn cpu_index() -> usize { crate::arch::x86_64::percpu::current_logical() }
struct Cell(UnsafeCell<Option<Scheduler>>,AtomicBool);
// SAFETY: the atomic lock and local IRQ masking serialize all scheduler state.
// Each CPU has its own host/current slot. Only diagnostic PID 0 kernel
// workers may run on APs; user threads remain pinned to the BSP.
unsafe impl Sync for Cell {}
static SCHED: Cell = Cell(UnsafeCell::new(None),AtomicBool::new(false));
static KERNEL_ONLINE:AtomicU64=AtomicU64::new(1);
pub fn kernel_online_cpus()->u32{KERNEL_ONLINE.load(Ordering::Acquire).count_ones()}
pub fn kernel_online_mask() -> u64 { KERNEL_ONLINE.load(Ordering::Acquire) }

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
/// Complete a thread's transition only after the CPU is back on its host
/// stack. Until this point the old thread must remain unclaimable by APs.
fn finish_switch() {
    with_sched(|s| {
        let local = &mut s.cpus[cpu_index()];
        let Some(next) = local.pending.take() else { return };
        let index = local.current.take().expect("switch without current thread");
        debug_assert!(!s.ready.contains(&index), "active stack already queued");
        let thread = s.threads[index].as_mut().expect("switched thread missing");
        debug_assert_eq!(thread.state, ThreadState::Running);
        let pid = thread.pid;
        let tid = thread.tid;
        match next {
            ThreadState::Ready => {
                thread.state = ThreadState::Ready;
                crate::process::mark_ready(pid);
                s.ready.push_back(index);
            }
            ThreadState::Blocked => {
                if thread.wake_pending {
                    thread.wake_pending = false;
                    thread.waiting_for = None;
                    thread.state = ThreadState::Ready;
                    crate::process::mark_ready(pid);
                    s.ready.push_back(index);
                } else {
                    thread.state = ThreadState::Blocked;
                    crate::process::mark_blocked(pid);
                }
            }
            ThreadState::Exited => {
                thread.state = ThreadState::Exited;
                for (waiter_index, slot) in s.threads.iter_mut().enumerate() {
                    if let Some(waiter) = slot.as_mut() {
                        if waiter.waiting_for == Some(tid) {
                            waiter.waiting_for = None;
                            if waiter.state == ThreadState::Blocked {
                                waiter.state = ThreadState::Ready;
                                crate::process::mark_ready(waiter.pid);
                                s.ready.push_back(waiter_index);
                            } else if waiter.state == ThreadState::Running {
                                waiter.wake_pending = true;
                            }
                        }
                    }
                }
            }
            _ => panic!("invalid scheduler switch target"),
        }
    });
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
/// Pin a diagnostic kernel worker to one AP. User processes cannot use this
/// path until address-space shootdown and process lifetime are SMP-safe.
#[cfg(feature = "smptest")]
pub fn spawn_ap_worker(logical:usize,entry:extern "C" fn()->!)->ThreadId{
    assert!((1..8).contains(&logical));
    with_sched(|s|{
        let tid=ThreadId(s.next_tid);s.next_tid+=1;
        let mut thread=Box::new(Thread::new(tid,ProcessId(0),entry));
        thread.affinity=logical;thread.state=ThreadState::Ready;
        s.threads.push(Some(thread));s.ready.push_back(s.threads.len()-1);tid
    })
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
 let tid=unsafe{(*SCHED.0.get()).as_ref().and_then(|s|{
  let cpu=crate::arch::x86_64::percpu::try_current_logical()?;
  s.cpus[cpu].current.and_then(|i|s.threads.get(i)?.as_ref().map(|t|t.tid))
 })};
 SCHED.1.store(false,Ordering::Release);
 if enabled{cpu::enable_interrupts()}tid
}
pub fn other_live_threads(pid:ProcessId)->bool{with_sched(|s|{
 let own=s.cpus[cpu_index()].current;
 s.threads.iter().enumerate().any(|(i,slot)|Some(i)!=own&&slot.as_ref().is_some_and(|t|t.pid==pid&&!matches!(t.state,ThreadState::Exited|ThreadState::Faulted)))
})}
pub fn run() {
    loop {
        // Keep the CPU on its host stack with IF clear from claiming a thread
        // through the actual switch. A timer IRQ in this interval would see
        // current=Some and could otherwise preempt the host as that thread.
        let enabled = cpu::interrupts_enabled();
        cpu::disable_interrupts();
        let pair = with_sched(|s| {
            let position=s.ready.iter().position(|&i|s.threads[i].as_ref().is_some_and(|t|t.affinity==cpu_index()))?;
            let i = s.ready.remove(position).expect("ready queue position vanished");
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
            if enabled { cpu::enable_interrupts(); }
            crate::tty::service();
            crate::input::service_raw();
            crate::process::wake_sleepers(crate::interrupts::timer::uptime_ms());
            if has_ready(){continue}
            let blocked=with_sched(|s|s.threads.iter().any(|t|t.as_ref().is_some_and(|t|t.affinity==cpu_index()&&t.state==ThreadState::Blocked)));
            if !blocked {break}
            if !has_ready(){cpu::enable_interrupts();cpu::halt();}
            continue;
        };
        if let Some(root) = crate::process::space_root() {
            cpu::write_cr3(root);
        }
        // SAFETY: boxed thread contexts are pinned until the run loop finishes; the
        // scheduler has no mutable borrow outstanding across this stack switch.
        unsafe {
            context::switch(old, new);
        }
        finish_switch();
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
/// AP host loop for diagnostic PID 0 workers only. Every user thread and
/// ordinary kernel thread remains pinned to CPU 0 by Thread::new.
pub fn run_ap() -> ! {
    let logical=cpu_index();
    assert!(logical>0 && logical<8);
    with_sched(|s|s.cpus[logical].host_cr3=cpu::read_cr3());
    KERNEL_ONLINE.fetch_or(1u64<<logical,Ordering::Release);
    loop {
        cpu::disable_interrupts();
        let pair=with_sched(|s|{
            let position=s.ready.iter().position(|&i|s.threads[i].as_ref().is_some_and(|t|t.affinity==logical))?;
            let i=s.ready.remove(position).expect("AP ready queue position vanished");
            s.cpus[logical].current=Some(i);
            let thread=s.threads[i].as_mut().expect("AP ready thread missing");
            assert_eq!(thread.pid,ProcessId(0),"user thread assigned to AP");
            thread.state=ThreadState::Running;
            let top=thread.kernel_stack_top();
            crate::arch::x86_64::gdt::set_rsp0(top);
            crate::arch::x86_64::syscall::set_stack(top);
            Some((&raw mut s.cpus[logical].host,&raw const thread.context))
        });
        let Some((host,thread))=pair else {
            // SAFETY: STI followed immediately by HLT avoids losing a wakeup
            // IPI between enabling interrupts and entering the idle state.
            unsafe{core::arch::asm!("sti; hlt",options(nomem,nostack));}
            continue
        };
        // SAFETY: the selected boxed thread is pinned and cannot be selected
        // by another CPU while its state is Running. Local IRQs are masked.
        unsafe{context::switch(host,thread)}
        finish_switch();
        let host_cr3=with_sched(|s|s.cpus[logical].host_cr3);
        cpu::write_cr3(host_cr3);
        cpu::enable_interrupts();
    }
}
#[cfg(feature="smptest")]
pub fn ap_workers_exited(tids:&[ThreadId])->bool{with_sched(|s|{
    tids.iter().all(|tid|s.threads.iter().flatten().any(|t|t.tid==*tid&&t.state==ThreadState::Exited))
})}
#[cfg(feature="smptest")]
pub fn reap_ap_workers(tids:&[ThreadId]){with_sched(|s|{
    for slot in &mut s.threads{
        if slot.as_ref().is_some_and(|t|tids.contains(&t.tid)&&t.state==ThreadState::Exited){*slot=None}
    }
})}
#[cfg(feature="smptest")]
static AP_WORK_DONE:core::sync::atomic::AtomicU64=core::sync::atomic::AtomicU64::new(0);
#[cfg(feature="smptest")]
static AP_WORK_STARTED:AtomicU64=AtomicU64::new(0);
#[cfg(feature="smptest")]
static AP_WORK_EXPECTED:AtomicU64=AtomicU64::new(0);
#[cfg(feature="smptest")]
static AP_WORK_TOTAL:AtomicU64=AtomicU64::new(0);
#[cfg(feature="smptest")]
static AP_WORK_FLUSHED:AtomicU64=AtomicU64::new(0);
#[cfg(feature="smptest")]
static AP_WORK_FLUSH_COUNT:AtomicU64=AtomicU64::new(0);
#[cfg(feature="smptest")]
extern "C" fn ap_worker()->!{
    let logical=cpu_index();
    assert!(logical>0,"AP worker ran on BSP");
    AP_WORK_STARTED.fetch_or(1u64<<logical,Ordering::Release);
    let barrier_deadline=crate::interrupts::timer::uptime_ms().saturating_add(2000);
    while AP_WORK_STARTED.load(Ordering::Acquire)&AP_WORK_EXPECTED.load(Ordering::Acquire)
        !=AP_WORK_EXPECTED.load(Ordering::Acquire)
    {
        if crate::interrupts::timer::uptime_ms()>=barrier_deadline{exit_current()}
        core::hint::spin_loop();
    }
    cpu::enable_interrupts();
    let expected_targets=AP_WORK_EXPECTED.load(Ordering::Acquire).count_ones() as usize;
    for _ in 0..16 {
        if crate::arch::x86_64::tlb::broadcast()!=Ok(expected_targets){exit_current()}
        AP_WORK_FLUSH_COUNT.fetch_add(1,Ordering::AcqRel);
        yield_current();
    }
    AP_WORK_FLUSHED.fetch_or(1u64<<logical,Ordering::Release);
    for _ in 0..64 {
        assert_eq!(cpu_index(),logical,"AP worker migrated unexpectedly");
        AP_WORK_TOTAL.fetch_add(1,Ordering::AcqRel);
        yield_current();
    }
    AP_WORK_DONE.fetch_or(1u64<<logical,Ordering::Release);
    exit_current()
}
#[cfg(feature="smptest")]
pub fn ap_selftest(){
    let mut tids=Vec::new();
    let mut expected=0u64;
    for logical in 1..8 {
        if matches!(crate::arch::x86_64::percpu::state(logical),Some(crate::arch::x86_64::percpu::State::Idle)) {
            tids.push(spawn_ap_worker(logical,ap_worker));
            expected|=1u64<<logical;
        }
    }
    AP_WORK_EXPECTED.store(expected,Ordering::Release);
    let started=crate::arch::x86_64::smp::start_scheduler().expect("AP scheduler start failed");
    assert_eq!(started,expected.count_ones() as usize);
    let deadline=crate::interrupts::timer::uptime_ms().saturating_add(3000);
    while !ap_workers_exited(&tids) && crate::interrupts::timer::uptime_ms()<deadline {cpu::halt()}
    assert!(ap_workers_exited(&tids),"AP kernel worker test timed out");
    assert_eq!(AP_WORK_DONE.load(Ordering::Acquire)&expected,expected);
    assert_eq!(AP_WORK_STARTED.load(Ordering::Acquire)&expected,expected);
    assert_eq!(AP_WORK_FLUSHED.load(Ordering::Acquire)&expected,expected);
    assert_eq!(AP_WORK_FLUSH_COUNT.load(Ordering::Acquire),started as u64*16);
    assert_eq!(AP_WORK_TOTAL.load(Ordering::Acquire),started as u64*64);
    reap_ap_workers(&tids);
    crate::nova_log!(Info,"TEST","concurrent TLB flush passed on {} APs (16 rounds each)",started);
    crate::nova_log!(Info,"TEST","AP scheduler kernel workers passed on {} APs",started);
}
pub fn yield_current() {
    switch_to_host(ThreadState::Ready, None);
}
pub fn exit_current() -> ! {
    switch_to_host(ThreadState::Exited, None);
    unreachable!("exited thread resumed")
}

pub fn block_current() {
 switch_to_host(ThreadState::Blocked, None);
}
pub fn exit_thread()->! {
 switch_to_host(ThreadState::Exited, None);
 unreachable!("exited thread resumed")
}
fn switch_to_host(next:ThreadState,waiting_for:Option<ThreadId>) {
 let enabled=cpu::interrupts_enabled();
 cpu::disable_interrupts();
 let pair=with_sched(|s|{
  let local=&mut s.cpus[cpu_index()];
  let i=local.current.expect("switch without running thread");
  let thread=s.threads[i].as_mut().expect("current thread missing");
  if next==ThreadState::Blocked&&thread.wake_pending {
   thread.wake_pending=false;
   return None;
  }
  assert!(local.pending.is_none(),"nested scheduler switch");
  thread.waiting_for=waiting_for;
  local.pending=Some(next);
  Some((&raw mut thread.context,&raw const local.host))
 });
 if let Some((old,new))=pair {
  // SAFETY: both contexts are pinned. The thread remains Running until its
  // original CPU has returned to the host stack and publishes the transition.
  unsafe{context::switch(old,new)}
 }
 if enabled {cpu::enable_interrupts()}
}
pub fn join(tid:ThreadId)->Result<(),i64>{
 let pid=crate::process::current_pid().ok_or(-3i64)?;
 loop {
  let enabled=cpu::interrupts_enabled();
  cpu::disable_interrupts();
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
   thread.waiting_for=Some(tid);
   assert!(s.cpus[cpu_index()].pending.is_none(),"nested join switch");
   s.cpus[cpu_index()].pending=Some(ThreadState::Blocked);
   Ok::<Option<(*mut Context,*const Context)>,i64>(Some((&raw mut thread.context,&raw const s.cpus[cpu_index()].host)))
  });
  let pending=match pending{Ok(value)=>value,Err(error)=>{if enabled{cpu::enable_interrupts()}return Err(error)}};
  let Some((old,new))=pending else {if enabled{cpu::enable_interrupts()}crate::process::remove_thread(tid);return Ok(())};
  // SAFETY: the thread remains Running and unavailable to other CPUs until
  // its host finalizes this transition after the stack switch.
  unsafe{context::switch(old,new)}
  if enabled{cpu::enable_interrupts()}
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
pub fn has_ready()->bool {with_sched(|s|s.ready.iter().any(|&i|s.threads[i].as_ref().is_some_and(|t|t.affinity==cpu_index())))}
/// Called only after the timer's EOI, on the interrupted thread's trusted
/// kernel stack. The IRQ stub preserves volatile GPRs and FPU state; this
/// context resumes inside the same IRQ handler and eventually executes IRETQ.
pub fn preempt_if_running() {
    let should_switch = with_sched(|s| s.cpus[cpu_index()].current.is_some()
        && s.ready.iter().any(|&i|s.threads[i].as_ref().is_some_and(|t|t.affinity==cpu_index())));
    if should_switch { yield_current(); }
}
