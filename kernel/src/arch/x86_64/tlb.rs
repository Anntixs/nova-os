//! Reserved IPI vector and remote TLB flush probe. This is not yet a general
//! address-space shootdown protocol: AP user scheduling remains disabled.
use core::sync::atomic::{AtomicU64, Ordering};
use super::{apic, cpu, percpu};

pub const VECTOR: u8 = 240;
static ACKNOWLEDGED: AtomicU64 = AtomicU64::new(0);

/// Runs on the target CPU's interrupt stack with local interrupts masked.
pub fn on_ipi() {
    let root = cpu::read_cr3();
    // SAFETY: CR3 was read from this CPU's active, valid PML4. Writing it back
    // invalidates this CPU's non-global translations before acknowledging.
    cpu::write_cr3(root);
    let logical = percpu::current_logical();
    ACKNOWLEDGED.fetch_or(1u64 << logical, Ordering::Release);
}

/// Check that every AP which entered Nova can receive and acknowledge the
/// reserved IPI. Called by the BSP after AP startup in diagnostic builds.
#[cfg(feature = "smptest")]
pub fn probe() -> Result<usize, &'static str> {
    if percpu::current_logical() != 0 || !cpu::interrupts_enabled() {
        return Err("TLB probe requires BSP with interrupts enabled");
    }
    ACKNOWLEDGED.store(0, Ordering::Release);
    let mut tested = 0;
    for logical in 1..8 {
        if !matches!(percpu::state(logical), Some(percpu::State::Idle | percpu::State::Running)) {
            continue;
        }
        let apic_id = percpu::apic_id(logical).ok_or("APIC ID missing")?;
        let apic_id = u8::try_from(apic_id).map_err(|_| "xAPIC ID out of range")?;
        apic::send_ipi(apic_id, u32::from(VECTOR)).map_err(|_| "TLB IPI delivery failed")?;
        let bit = 1u64 << logical;
        let deadline = crate::interrupts::timer::uptime_ms().saturating_add(100);
        while ACKNOWLEDGED.load(Ordering::Acquire) & bit == 0
            && crate::interrupts::timer::uptime_ms() < deadline
        {
            cpu::halt();
        }
        if ACKNOWLEDGED.load(Ordering::Acquire) & bit == 0 {
            return Err("TLB IPI acknowledgement timed out");
        }
        tested += 1;
    }
    Ok(tested)
}
