pub mod timer;
use crate::{
    arch::x86_64::{apic, cpu},
    drivers::ps2,
};
#[no_mangle]
extern "C" fn nova_irq_dispatch(vector: u64) {
    match vector {
        32 => {
            timer::on_tick();
            apic::eoi();
            if timer::ticks() % 10 == 0 { crate::scheduler::preempt_if_running(); }
        }
        33 => {
            ps2::on_irq();
            apic::eoi();
        }
        240 => {
            crate::arch::x86_64::tlb::on_ipi();
            apic::eoi();
        }
        241 => {
            apic::eoi();
        }
        255 => {}
        _ => {
            cpu::halt_forever();
        }
    }
}
