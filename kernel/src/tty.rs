use alloc::{collections::VecDeque,vec::Vec};
use core::{cell::UnsafeCell,sync::atomic::{AtomicBool,Ordering}};
use crate::{input,process::ProcessId,arch::x86_64::cpu};
struct Tty {queue:VecDeque<u8>,line:[u8;256],len:usize,waiter:Option<ProcessId>}
struct Cell {data:UnsafeCell<Option<Tty>>,lock:AtomicBool}
// SAFETY: the atomic lock excludes concurrent CPU access to TTY state; local
// IRQ masking prevents reentry. No TTY reference escapes a locked closure.
unsafe impl Sync for Cell {}
static TTY:Cell=Cell{data:UnsafeCell::new(None),lock:AtomicBool::new(false)};
fn with_tty<R>(f:impl FnOnce(&mut Tty)->R)->R{
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 while TTY.lock.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
 // SAFETY: the atomic lock excludes other CPUs and IRQ masking prevents
 // reentry while the unique mutable TTY reference is borrowed.
 let result=f(unsafe{(*TTY.data.get()).as_mut().expect("TTY not initialized")});
 TTY.lock.store(false,Ordering::Release);
 if enabled{cpu::enable_interrupts()}result
}
pub fn init(){
 // SAFETY: one-shot boot initialization before userspace and keyboard consumption.
 unsafe{*TTY.data.get()=Some(Tty{queue:VecDeque::new(),line:[0;256],len:0,waiter:None})}
}
pub fn service(){
 while let Some(event)=input::pop(){
  if input::captured(){continue}
  if !event.pressed{continue}
  let (wake,echo)=with_tty(|tty|match event.key {
   b'\n'=>{
    for &byte in &tty.line[..tty.len]{tty.queue.push_back(byte)}
    tty.queue.push_back(b'\n');tty.len=0;
    (tty.waiter.take(),Some(b'\n'))
   },
   8=>{if tty.len>0{tty.len-=1;(None,Some(8))}else{(None,None)}},
   b if (32..127).contains(&b)=>{
    if tty.len<tty.line.len(){tty.line[tty.len]=b;tty.len+=1;(None,Some(b))}else{(None,None)}
   },_=>(None,None)
  });
  if let Some(byte)=echo{crate::process::output_byte(byte)}
  if let Some(pid)=wake{crate::scheduler::wake(pid)}
 }
}
/// Dequeue input or register the reader under one lock. A key arriving on
/// another CPU cannot slip between the empty check and waiter registration.
pub fn take_or_wait(max:usize,pid:ProcessId)->Result<Option<Vec<u8>>,()>{with_tty(|tty|{
 if tty.queue.is_empty(){
  if tty.waiter.is_some_and(|other|other!=pid){return Err(())}
  tty.waiter=Some(pid);return Ok(None)
 }
 let limit=max.min(tty.queue.len());
 let count=tty.queue.iter().take(limit).position(|&byte|byte==b'\n').map_or(limit,|index|index+1);
 let mut out=Vec::with_capacity(count);
 for _ in 0..count{out.push(tty.queue.pop_front().unwrap())}
 Ok(Some(out))
})}
