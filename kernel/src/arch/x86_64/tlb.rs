//! All-CPU TLB flush with per-target request and acknowledgement generations.
//! User page-map paths do not yet invoke it because they must first release
//! process locks before waiting for other CPUs.
use core::sync::atomic::{AtomicU64, Ordering};
use super::{apic, cpu, percpu};

pub const VECTOR: u8 = 240;
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);
static REQUEST: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
static ACKNOWLEDGED: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

/// Runs on the target CPU's interrupt stack with local interrupts masked.
pub fn on_ipi() {
    let logical = percpu::current_logical();
    let generation = REQUEST[logical].load(Ordering::Acquire);
    let root = cpu::read_cr3();
    // SAFETY: CR3 was read from this CPU's active, valid PML4. Writing it back
    // invalidates this CPU's non-global translations before acknowledging.
    cpu::write_cr3(root);
    ACKNOWLEDGED[logical].fetch_max(generation, Ordering::Release);
}

/// Flush the calling CPU and all CPUs admitted to the kernel scheduler.
/// The caller must have completed its page-table edits, hold no spinlocks,
/// and leave interrupts enabled so another sender's IPI can be serviced.
/// A full CR3 reload is used until Nova tracks per-CPU address-space roots.
pub fn broadcast() -> Result<usize, &'static str> {
    if !cpu::interrupts_enabled() {
        return Err("TLB broadcast requires interrupts enabled");
    }
    let source = percpu::current_logical();
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel)
        .checked_add(1).ok_or("TLB generation exhausted")?;
    let targets = crate::scheduler::kernel_online_mask() & !(1u64 << source);
    let root = cpu::read_cr3();
    cpu::write_cr3(root);
    for logical in 0..8 {
        if targets & (1u64 << logical) == 0 { continue; }
        REQUEST[logical].fetch_max(generation, Ordering::AcqRel);
        let id = percpu::apic_id(logical).ok_or("APIC ID missing")?;
        let id = u8::try_from(id).map_err(|_| "xAPIC ID out of range")?;
        apic::send_ipi(id, u32::from(VECTOR)).map_err(|_| "TLB IPI delivery failed")?;
    }
    let deadline = crate::interrupts::timer::uptime_ms().saturating_add(500);
    loop {
        let mut complete = true;
        for logical in 0..8 {
            if targets & (1u64 << logical) != 0
                && ACKNOWLEDGED[logical].load(Ordering::Acquire) < generation
            {
                complete = false;
                break;
            }
        }
        if complete { return Ok(targets.count_ones() as usize); }
        if crate::interrupts::timer::uptime_ms() >= deadline {
            return Err("TLB IPI acknowledgement timed out");
        }
        core::hint::spin_loop();
    }
}

/// Check that APs can receive a remote flush before diagnostic workers start.
#[cfg(feature = "smptest")]
pub fn probe() -> Result<usize, &'static str> {
    if percpu::current_logical() != 0 || !cpu::interrupts_enabled() {
        return Err("TLB probe requires BSP with interrupts enabled");
    }
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel)
        .checked_add(1).ok_or("TLB generation exhausted")?;
    let mut tested = 0;
    for logical in 1..8 {
        if !matches!(percpu::state(logical), Some(percpu::State::Idle | percpu::State::Running)) {
            continue;
        }
        REQUEST[logical].fetch_max(generation, Ordering::AcqRel);
        let apic_id = percpu::apic_id(logical).ok_or("APIC ID missing")?;
        let apic_id = u8::try_from(apic_id).map_err(|_| "xAPIC ID out of range")?;
        apic::send_ipi(apic_id, u32::from(VECTOR)).map_err(|_| "TLB IPI delivery failed")?;
        let deadline = crate::interrupts::timer::uptime_ms().saturating_add(100);
        while ACKNOWLEDGED[logical].load(Ordering::Acquire) < generation
            && crate::interrupts::timer::uptime_ms() < deadline
        {
            cpu::halt();
        }
        if ACKNOWLEDGED[logical].load(Ordering::Acquire) < generation {
            return Err("TLB IPI acknowledgement timed out");
        }
        tested += 1;
    }
    Ok(tested)
}
