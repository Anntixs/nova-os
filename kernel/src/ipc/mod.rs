//! Single-CPU IPC primitives. Endpoint state is shared with Arc; interrupt
//! masking protects short queue operations. No user pointer enters this layer.
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use core::{cell::UnsafeCell,sync::atomic::{AtomicBool,Ordering}};
use crate::{arch::x86_64::cpu, process::ProcessId};

const PIPE_CAPACITY: usize = 4096;
const CHANNEL_CAPACITY: usize = 32;
const MAX_MESSAGE: usize = 4096;

struct Protected<T>{data:UnsafeCell<T>,lock:AtomicBool}
impl<T> Protected<T>{fn new(data:T)->Self{Self{data:UnsafeCell::new(data),lock:AtomicBool::new(false)}}}
// SAFETY: every access to the enclosed state acquires the atomic lock with
// local interrupts masked. The state is Send and no mutable borrow escapes.
unsafe impl<T:Send> Sync for Protected<T>{}
fn critical<T, R>(cell: &Protected<T>, f: impl FnOnce(&mut T) -> R) -> R {
    let enabled = cpu::interrupts_enabled();
    cpu::disable_interrupts();
    while cell.lock.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
    // SAFETY: the atomic lock excludes other CPUs; local IRQs cannot reenter
    // while this unique mutable reference is borrowed.
    let result = f(unsafe { &mut *cell.data.get() });
    cell.lock.store(false,Ordering::Release);
    if enabled { cpu::enable_interrupts(); }
    result
}

struct PipeState {
    bytes: VecDeque<u8>, readers: usize, writers: usize,
    reading: Vec<ProcessId>, writing: Vec<ProcessId>,
}
struct Pipe(Protected<PipeState>);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction { Read, Write }
pub struct PipeEnd { pipe: Arc<Pipe>, direction: Direction }
impl Clone for PipeEnd {
    fn clone(&self) -> Self {
        critical(&self.pipe.0, |state| match self.direction {
            Direction::Read => state.readers += 1,
            Direction::Write => state.writers += 1,
        });
        Self { pipe: self.pipe.clone(), direction: self.direction }
    }
}
impl Drop for PipeEnd {
    fn drop(&mut self) {
        let wake = critical(&self.pipe.0, |state| match self.direction {
            Direction::Read => {state.readers -= 1; if state.readers == 0 {core::mem::take(&mut state.writing)} else {Vec::new()}},
            Direction::Write => {state.writers -= 1; if state.writers == 0 {core::mem::take(&mut state.reading)} else {Vec::new()}},
        });
        for pid in wake { crate::scheduler::wake(pid); }
    }
}
pub fn pipe() -> (PipeEnd, PipeEnd) {
    let shared = Arc::new(Pipe(Protected::new(PipeState {
        bytes: VecDeque::new(), readers: 1, writers: 1,
        reading: Vec::new(), writing: Vec::new(),
    })));
    (PipeEnd { pipe: shared.clone(), direction: Direction::Read },
     PipeEnd { pipe: shared, direction: Direction::Write })
}
pub enum PipeRead { Data(Vec<u8>), Eof, Block }
pub enum PipeWrite { Done(usize), Closed, Block }
fn add_waiter(waiters: &mut Vec<ProcessId>, pid: ProcessId) {
    if !waiters.contains(&pid) { waiters.push(pid); }
}
impl PipeEnd {
    pub fn can_read(&self) -> bool { self.direction == Direction::Read }
    pub fn can_write(&self) -> bool { self.direction == Direction::Write }
    pub fn read(&self, count: usize, pid: ProcessId) -> PipeRead {
        if self.direction != Direction::Read { return PipeRead::Eof; }
        let (result, wake) = critical(&self.pipe.0, |state| {
            if !state.bytes.is_empty() {
                let n = count.min(state.bytes.len());
                let mut out = Vec::with_capacity(n);
                for _ in 0..n { out.push(state.bytes.pop_front().unwrap()); }
                (PipeRead::Data(out), core::mem::take(&mut state.writing))
            } else if state.writers == 0 { (PipeRead::Eof, Vec::new()) }
            else { add_waiter(&mut state.reading, pid); (PipeRead::Block, Vec::new()) }
        });
        for pid in wake { crate::scheduler::wake(pid); }
        result
    }
    pub fn write(&self, data: &[u8], pid: ProcessId) -> PipeWrite {
        if self.direction != Direction::Write { return PipeWrite::Closed; }
        let (result, wake) = critical(&self.pipe.0, |state| {
            if state.readers == 0 { return (PipeWrite::Closed, Vec::new()); }
            let n = data.len().min(PIPE_CAPACITY - state.bytes.len());
            if n == 0 { add_waiter(&mut state.writing, pid); return (PipeWrite::Block, Vec::new()); }
            state.bytes.extend(data[..n].iter().copied());
            (PipeWrite::Done(n), core::mem::take(&mut state.reading))
        });
        for pid in wake { crate::scheduler::wake(pid); }
        result
    }
}

struct ChannelState { messages: VecDeque<Vec<u8>>, receiving: Vec<ProcessId>, sending: Vec<ProcessId> }
struct Channel(Protected<ChannelState>);
#[derive(Clone)]
pub struct ChannelEnd { channel: Arc<Channel>, nonblocking: bool }
pub fn channel(nonblocking: bool) -> ChannelEnd {
    ChannelEnd {channel: Arc::new(Channel(Protected::new(ChannelState {
        messages: VecDeque::new(), receiving: Vec::new(), sending: Vec::new(),
    }))), nonblocking}
}
pub enum MessageRead { Data(Vec<u8>), TooSmall, Block, Again }
pub enum MessageWrite { Done, Block, Again }
impl ChannelEnd {
    pub fn send(&self, data: &[u8], pid: ProcessId) -> Result<MessageWrite, i64> {
        if data.len() > MAX_MESSAGE { return Err(-22); }
        let (result, wake) = critical(&self.channel.0, |state| {
            if state.messages.len() == CHANNEL_CAPACITY {
                if self.nonblocking { (MessageWrite::Again, Vec::new()) }
                else { add_waiter(&mut state.sending, pid); (MessageWrite::Block, Vec::new()) }
            } else {
                state.messages.push_back(data.to_vec());
                (MessageWrite::Done, core::mem::take(&mut state.receiving))
            }
        });
        for pid in wake { crate::scheduler::wake(pid); }
        Ok(result)
    }
    pub fn receive(&self, capacity: usize, pid: ProcessId) -> MessageRead {
        let (result, wake) = critical(&self.channel.0, |state| {
            if let Some(first) = state.messages.front() {
                if first.len() > capacity { return (MessageRead::TooSmall, Vec::new()); }
                (MessageRead::Data(state.messages.pop_front().unwrap()), core::mem::take(&mut state.sending))
            } else if self.nonblocking { (MessageRead::Again, Vec::new()) }
            else { add_waiter(&mut state.receiving, pid); (MessageRead::Block, Vec::new()) }
        });
        for pid in wake { crate::scheduler::wake(pid); }
        result
    }
}
