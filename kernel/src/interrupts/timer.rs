use crate::arch::x86_64::cpu;
use core::sync::atomic::{AtomicU64, Ordering};
static TICKS: AtomicU64 = AtomicU64::new(0);
pub fn init() {
    let divisor: u16 = 1193;
    cpu::port_out(0x43, 0x34);
    cpu::port_out(0x40, divisor as u8);
    cpu::port_out(0x40, (divisor >> 8) as u8);
}
pub fn on_tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
pub fn uptime_ms() -> u64 {
    ticks()
}
