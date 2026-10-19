use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU64,AtomicUsize, Ordering},
};
#[derive(Clone, Copy)]
pub struct KeyEvent {
    pub key: u8,
    pub pressed: bool,
    pub modifiers: u8,
    pub code: u16,
    pub timestamp_ms: u64,
}
#[derive(Clone,Copy)]
pub struct PointerEvent {pub x:i32,pub y:i32,pub buttons:u8,pub timestamp_ms:u64}
#[derive(Clone,Copy)]
pub enum InputEvent {Key(KeyEvent),Pointer(PointerEvent)}
impl KeyEvent {
    pub const EMPTY: Self = Self {
        key: 0,
        pressed: false,
        modifiers: 0,
        code: 0,
        timestamp_ms: 0,
    };
}
pub const SHIFT: u8 = 1;
pub const CTRL: u8 = 2;
pub const ALT: u8 = 4;
pub const CAPS: u8 = 8;
struct Queue(UnsafeCell<[KeyEvent; 128]>);
// SAFETY: one bootstrap CPU produces in IRQ context and one main-loop consumer reads.
// Atomic head/tail ordering publishes each fully written event before it is observed.
unsafe impl Sync for Queue {}
static QUEUE: Queue = Queue(UnsafeCell::new([KeyEvent::EMPTY; 128]));
static RAW: Queue = Queue(UnsafeCell::new([KeyEvent::EMPTY; 128]));
static RAW_HEAD: AtomicUsize = AtomicUsize::new(0);
static RAW_TAIL: AtomicUsize = AtomicUsize::new(0);
static RAW_WAITER: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicUsize = AtomicUsize::new(0);
static OWNER: AtomicU64 = AtomicU64::new(0);
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);
pub fn captured()->bool{OWNER.load(Ordering::Acquire)!=0}
pub fn owned_by(pid:crate::process::ProcessId)->bool{OWNER.load(Ordering::Acquire)==pid.0}
pub fn acquire(pid:crate::process::ProcessId)->Result<(),i64>{
 if pid.0==0{return Err(-22)}
 match OWNER.compare_exchange(0,pid.0,Ordering::AcqRel,Ordering::Acquire){
  Ok(_)=>{
   // The caller enters with interrupts disabled in the syscall path. Old
   // shell keystrokes must not be delivered to the new raw-input owner.
   RAW_TAIL.store(RAW_HEAD.load(Ordering::Acquire),Ordering::Release);
   TAIL.store(HEAD.load(Ordering::Acquire),Ordering::Release);
   Ok(())
  },
  Err(old) if old==pid.0=>Ok(()),
  Err(_)=>Err(-16),
 }
}
pub fn release(pid:crate::process::ProcessId)->Result<(),i64>{
 OWNER.compare_exchange(pid.0,0,Ordering::AcqRel,Ordering::Acquire).map_err(|_|-1)?;
 RAW_WAITER.store(0,Ordering::Release);
 Ok(())
}
pub fn release_if_owner(pid:crate::process::ProcessId){let _=release(pid);}
pub fn push(event: KeyEvent) {
    let raw_head=RAW_HEAD.load(Ordering::Relaxed);
    let raw_next=(raw_head+1)%128;
    if raw_next==RAW_TAIL.load(Ordering::Acquire){
        RAW_TAIL.store((raw_next+1)%128,Ordering::Release);
    }
    // SAFETY: IRQ1 is the only producer of raw events. The syscall consumer
    // runs with IF clear, so it cannot race this slot write on the BSP.
    unsafe{(*RAW.0.get())[raw_head]=event};
    RAW_HEAD.store(raw_next,Ordering::Release);
    // Route a captured key only to the raw owner. Enqueuing it for the TTY
    // would allow delayed host-loop service to echo it after lease release.
    if captured(){return}
    let head = HEAD.load(Ordering::Relaxed);
    let next = (head + 1) % 128;
    if next == TAIL.load(Ordering::Acquire) {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    // SAFETY: only IRQ producer writes this slot before publishing HEAD.
    unsafe {
        (*QUEUE.0.get())[head] = event;
    }
    HEAD.store(next, Ordering::Release);
}
pub fn pop_raw()->Option<KeyEvent>{
    let tail=RAW_TAIL.load(Ordering::Relaxed);
    if tail==RAW_HEAD.load(Ordering::Acquire){return None}
    // SAFETY: the record is published by IRQ1 before RAW_HEAD advances;
    // syscalls read with interrupts masked on the sole active CPU.
    let event=unsafe{(*RAW.0.get())[tail]};
    RAW_TAIL.store((tail+1)%128,Ordering::Release);Some(event)
}
pub fn wait_raw(pid:crate::process::ProcessId)->Result<(),()>{
    RAW_WAITER.compare_exchange(0,pid.0,Ordering::AcqRel,Ordering::Acquire)
        .map(|_|()).map_err(|_|())
}
pub fn service_raw(){
    if RAW_HEAD.load(Ordering::Acquire)!=RAW_TAIL.load(Ordering::Acquire){
        let pid=RAW_WAITER.swap(0,Ordering::AcqRel);
        if pid!=0{crate::scheduler::wake(crate::process::ProcessId(pid))}
    }
}
pub fn pop() -> Option<KeyEvent> {
    let tail = TAIL.load(Ordering::Relaxed);
    if tail == HEAD.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: only the main consumer reads this published slot, before advancing TAIL.
    let event = unsafe { (*QUEUE.0.get())[tail] };
    TAIL.store((tail + 1) % 128, Ordering::Release);
    Some(event)
}

pub fn dropped() -> usize {
    DROPPED.load(Ordering::Relaxed)
}
