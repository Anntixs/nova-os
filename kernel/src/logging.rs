use core::fmt::{self, Write};
use core::cell::UnsafeCell;
use alloc::vec::Vec;
use crate::arch::x86_64::cpu;
const LOG_SIZE:usize=65536;
struct Ring {data:[u8;LOG_SIZE],next:usize,full:bool}
struct Cell(UnsafeCell<Ring>);
// SAFETY: every ring access runs on the BSP with interrupts masked. IRQ code
// does not call the formatted logger, so there is no recursive borrow.
unsafe impl Sync for Cell {}
static RING:Cell=Cell(UnsafeCell::new(Ring{data:[0;LOG_SIZE],next:0,full:false}));
fn append(bytes:&[u8]){
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 // SAFETY: BSP-only access with IF clear. LOG_SIZE is fixed and next wraps.
 let ring=unsafe{&mut *RING.0.get()};
 for &byte in bytes{ring.data[ring.next]=byte;ring.next+=1;if ring.next==LOG_SIZE{ring.next=0;ring.full=true}}
 if enabled{cpu::enable_interrupts()}
}
pub fn snapshot()->Vec<u8>{
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 // SAFETY: BSP-only copy with IF clear; the ring is never reallocated.
 let ring=unsafe{&*RING.0.get()};
 let mut data=Vec::with_capacity(if ring.full{LOG_SIZE}else{ring.next});
 if ring.full{data.extend_from_slice(&ring.data[ring.next..])}
 data.extend_from_slice(&ring.data[..ring.next]);
 if enabled{cpu::enable_interrupts()}data
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
    let mut out = SerialWriter;
    let _ = writeln!(out, "[NOVA][{}][{}] {}", level.label(), component, message);
}
#[macro_export]
macro_rules! nova_log {($level:ident,$component:expr,$($arg:tt)*)=>{$crate::logging::record($crate::logging::Level::$level,$component,format_args!($($arg)*))};}
