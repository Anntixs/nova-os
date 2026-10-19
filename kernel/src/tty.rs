use alloc::{collections::VecDeque,vec::Vec};
use core::cell::UnsafeCell;
use crate::{input,process::ProcessId,arch::x86_64::cpu};
struct Tty {queue:VecDeque<u8>,line:[u8;256],len:usize,waiter:Option<ProcessId>}
struct Cell(UnsafeCell<Option<Tty>>);
// SAFETY: one CPU; IRQ handlers only enqueue KeyEvent, and TTY state is touched
// by the kernel host loop or a syscall with interrupts masked.
unsafe impl Sync for Cell {}
static TTY:Cell=Cell(UnsafeCell::new(None));
fn with_tty<R>(f:impl FnOnce(&mut Tty)->R)->R{
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 // SAFETY: BSP-only access with IRQs masked; init precedes all calls.
 let result=f(unsafe{(*TTY.0.get()).as_mut().expect("TTY not initialized")});
 if enabled{cpu::enable_interrupts()}result
}
pub fn init(){
 // SAFETY: one-shot boot initialization before userspace and keyboard consumption.
 unsafe{*TTY.0.get()=Some(Tty{queue:VecDeque::new(),line:[0;256],len:0,waiter:None})}
}
pub fn service(){
 while let Some(event)=input::pop(){
  if input::captured(){continue}
  if !event.pressed{continue}
  let wake=with_tty(|tty|match event.key {
   b'\n'=>{
    for &byte in &tty.line[..tty.len]{tty.queue.push_back(byte)}
    tty.queue.push_back(b'\n');tty.len=0;
    crate::process::output_byte(b'\n');tty.waiter.take()
   },
   8=>{if tty.len>0{tty.len-=1;crate::process::output_byte(8)}None},
   b if (32..127).contains(&b)=>{
    if tty.len<tty.line.len(){tty.line[tty.len]=b;tty.len+=1;crate::process::output_byte(b)}None
   },_=>None
  });
  if let Some(pid)=wake{crate::scheduler::wake(pid)}
 }
}
pub fn take(max:usize)->Option<Vec<u8>>{with_tty(|tty|{
 if tty.queue.is_empty(){return None}
 let limit=max.min(tty.queue.len());
 let count=tty.queue.iter().take(limit).position(|&byte|byte==b'\n').map_or(limit,|index|index+1);
 let mut out=Vec::with_capacity(count);
 for _ in 0..count {out.push(tty.queue.pop_front().unwrap())}Some(out)
})}
pub fn wait(pid:ProcessId)->Result<(),()>{with_tty(|tty|{
 if tty.waiter.is_some_and(|other|other!=pid){return Err(())}tty.waiter=Some(pid);Ok(())
})}
