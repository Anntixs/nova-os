//! CPU-local identity and state. GS_BASE points to a pinned record. Scheduling
//! user code on APs remains disabled until syscall and process state are local.
use core::{arch::asm,mem::offset_of,sync::atomic::{AtomicU8,AtomicU32,AtomicU64,Ordering}};

#[repr(u8)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum State {Offline=0,Starting=1,Idle=2,Running=3,Failed=4}
#[repr(C)]
pub struct PerCpu {
    logical_id:u64,
    apic_id:AtomicU32,
    state:AtomicU8,
    pub current_tid:AtomicU64,
    pub kernel_stack:AtomicU64,
    pub preempt_depth:AtomicU32,
    pub interrupt_depth:AtomicU32,
    pub user_rsp:AtomicU64,
}
impl PerCpu {
    const fn new(logical_id:u64)->Self {Self{logical_id,apic_id:AtomicU32::new(0),state:AtomicU8::new(State::Offline as u8),current_tid:AtomicU64::new(0),kernel_stack:AtomicU64::new(0),preempt_depth:AtomicU32::new(0),interrupt_depth:AtomicU32::new(0),user_rsp:AtomicU64::new(0)}}
}
pub const KERNEL_STACK_OFFSET:usize=offset_of!(PerCpu,kernel_stack);
pub const USER_RSP_OFFSET:usize=offset_of!(PerCpu,user_rsp);
static CPUS:[PerCpu;8]=[
    PerCpu::new(0),PerCpu::new(1),PerCpu::new(2),PerCpu::new(3),
    PerCpu::new(4),PerCpu::new(5),PerCpu::new(6),PerCpu::new(7),
];
static DISCOVERED:AtomicU32=AtomicU32::new(1);
pub fn set_discovered(count:usize){DISCOVERED.store(count.min(u32::MAX as usize) as u32,Ordering::Release)}
pub fn discovered()->u32{DISCOVERED.load(Ordering::Acquire)}
pub fn entered()->usize{CPUS.iter().filter(|cpu|matches!(cpu.state.load(Ordering::Acquire),x if x==State::Idle as u8||x==State::Running as u8)).count()}
pub fn record(logical:usize)->Option<&'static PerCpu>{CPUS.get(logical)}
pub fn state(logical:usize)->Option<State>{Some(match record(logical)?.state.load(Ordering::Acquire){0=>State::Offline,1=>State::Starting,2=>State::Idle,3=>State::Running,_=>State::Failed})}
pub fn set_state(logical:usize,state:State){if let Some(cpu)=record(logical){cpu.state.store(state as u8,Ordering::Release)}}
pub fn apic_id(logical:usize)->Option<u32>{Some(record(logical)?.apic_id.load(Ordering::Acquire))}
pub fn set_apic_id(logical:usize,id:u32){if let Some(cpu)=record(logical){cpu.apic_id.store(id,Ordering::Release)}}
pub fn init_current(logical:usize,apic_id:u32,stack:u64){
    // Ring 3 must not be able to alter GS_BASE while the syscall entry uses
    // it to find the trusted CPU-local kernel stack.
    super::cpu::disable_fsgsbase();
    let local=record(logical).expect("CPU index exceeds per-CPU records");
    local.apic_id.store(apic_id,Ordering::Release);
    local.kernel_stack.store(stack,Ordering::Release);
    let base=local as *const PerCpu as u64;
    // SAFETY: both GS base MSRs exist in x86_64 long mode. The pinned record
    // remains live for this CPU. The initial user GS base is zero, so the
    // first SWAPGS into Ring 3 cannot expose a kernel pointer. SWAPGS places
    // the trusted base in IA32_KERNEL_GS_BASE; every user-to-kernel path swaps
    // it back before reading GS, even if userspace changed its own GS selector.
    unsafe{
        asm!("wrmsr",in("ecx")0xc000_0101u32,in("eax")base as u32,in("edx")(base>>32) as u32,options(nostack,preserves_flags));
        asm!("wrmsr",in("ecx")0xc000_0102u32,in("eax")0u32,in("edx")0u32,options(nostack,preserves_flags));
    }
}
pub fn set_kernel_stack(top:u64){
    record(current_logical()).expect("current CPU missing").kernel_stack.store(top,Ordering::Release);
}
pub fn current_logical()->usize{
    let id:u64;
    // SAFETY: init_current installed GS_BASE on this CPU. Offset zero is
    // logical_id by repr(C), and the record remains pinned.
    unsafe{asm!("mov {}, gs:[0]",out(reg)id,options(nostack,readonly,preserves_flags))}
    id as usize
}
