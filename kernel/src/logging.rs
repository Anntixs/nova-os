use core::fmt::{self, Write};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool,Ordering};
use alloc::vec::Vec;
use crate::arch::x86_64::cpu;
const LOG_SIZE:usize=65536;
struct Ring {data:[u8;LOG_SIZE],next:usize,full:bool}
struct Cell {ring:UnsafeCell<Ring>, lock:AtomicBool}
// SAFETY: every ring access holds the atomic lock with local IRQs masked.
// Formatted logging does not recursively enter the logger.
unsafe impl Sync for Cell {}
static RING:Cell=Cell{ring:UnsafeCell::new(Ring{data:[0;LOG_SIZE],next:0,full:false}),lock:AtomicBool::new(false)};
fn critical<R>(f:impl FnOnce()->R)->R{
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 while RING.lock.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
 let result=f();
 RING.lock.store(false,Ordering::Release);
 if enabled{cpu::enable_interrupts()}
 result
}
fn append(bytes:&[u8]){
 // SAFETY: record() holds the lock and masks local IRQs. LOG_SIZE is fixed.
 let ring=unsafe{&mut *RING.ring.get()};
 for &byte in bytes{ring.data[ring.next]=byte;ring.next+=1;if ring.next==LOG_SIZE{ring.next=0;ring.full=true}}
}
pub fn snapshot()->Vec<u8>{
 critical(||{
 // SAFETY: the atomic lock excludes concurrent writers and the ring is static.
 let ring=unsafe{&*RING.ring.get()};
 let mut data=Vec::with_capacity(if ring.full{LOG_SIZE}else{ring.next});
 if ring.full{data.extend_from_slice(&ring.data[ring.next..])}
 data.extend_from_slice(&ring.data[..ring.next]);
 data
 })
}
#[derive(Clone, Copy)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}
impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Fatal => "PANIC",
        }
    }
}
struct SerialWriter;
impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        append(s.as_bytes());
        for b in s.bytes() {
            if b == b'\n' {
                crate::serial_byte(b'\r');
            }
            crate::serial_byte(b);
        }
        Ok(())
    }
}
pub fn record(level: Level, component: &str, message: fmt::Arguments<'_>) {
    if !cfg!(debug_assertions) && matches!(level, Level::Trace | Level::Debug) {
        return;
    }
    critical(||{
        let mut out = SerialWriter;
        let _ = writeln!(out, "[NOVA][{}][{}] {}", level.label(), component, message);
    });
}
#[macro_export]
macro_rules! nova_log {($level:ident,$component:expr,$($arg:tt)*)=>{$crate::logging::record($crate::logging::Level::$level,$component,format_args!($($arg)*))};}
