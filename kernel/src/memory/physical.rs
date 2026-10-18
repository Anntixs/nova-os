use super::{PhysAddr, PhysFrame};
use crate::arch::x86_64::cpu;
use core::sync::atomic::{AtomicBool, Ordering};
use nova_core::NovaBootInfo;

const FRAME_SIZE: u64 = 4096;
const MAX_FRAMES: usize = 16 * 1024 * 1024; // 64 GiB, with a 2 MiB static bitmap.
const WORDS: usize = MAX_FRAMES / 64;
static mut FREE_BITS: [u64; WORDS] = [0; WORDS];
static mut ALLOCATED_BITS: [u64; WORDS] = [0; WORDS];
static LOCK: AtomicBool = AtomicBool::new(false);
static mut INITIALIZED: bool = false;
static mut USABLE: u64 = 0;
static mut FREE: u64 = 0;
static mut TOTAL: u64 = 0;
static mut NEXT: usize = 0;
#[derive(Clone, Copy)]
pub struct Stats {
    pub total_bytes: u64,
    pub usable_bytes: u64,
    pub free_bytes: u64,
    pub used_bytes: u64,
    pub reserved_bytes: u64,
}
#[derive(Debug)]
pub enum Error {
    InvalidMap,
    OutOfMemory,
    InvalidFrame,
    DoubleFree,
    NotInitialized,
}
fn lock() -> bool {
    let was = cpu::interrupts_enabled();
    cpu::disable_interrupts();
    while LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    was
}
fn unlock(was: bool) {
    LOCK.store(false, Ordering::Release);
    if was {
        cpu::enable_interrupts();
    }
}
fn is_free(n: usize) -> bool {
    // SAFETY: caller holds LOCK; n < MAX_FRAMES.
    unsafe { (*(&raw const FREE_BITS))[n / 64] & (1u64 << (n % 64)) != 0 }
}
fn is_allocated(n: usize) -> bool {
    // SAFETY: caller holds LOCK; n < MAX_FRAMES.
    unsafe { (*(&raw const ALLOCATED_BITS))[n / 64] & (1u64 << (n % 64)) != 0 }
}
fn set_allocated(n: usize, yes: bool) {
    // SAFETY: caller holds LOCK; n < MAX_FRAMES.
    unsafe {
        let bits = &raw mut ALLOCATED_BITS;
        if yes {
            (*bits)[n / 64] |= 1u64 << (n % 64);
        } else {
            (*bits)[n / 64] &= !(1u64 << (n % 64));
        }
    }
}
fn set_free(n: usize, yes: bool) {
    // SAFETY: caller holds LOCK; n < MAX_FRAMES.
    unsafe {
        let bits = &raw mut FREE_BITS;
        if yes {
            (*bits)[n / 64] |= 1u64 << (n % 64);
        } else {
            (*bits)[n / 64] &= !(1u64 << (n % 64));
        }
    }
}
fn descriptor(info: &NovaBootInfo, i: usize) -> (u32, u64, u64) {
    // SAFETY: init validates size and count before this call; read_unaligned handles firmware padding.
    unsafe {
        let p = (info.memory_map as *const u8).add(i * info.memory_descriptor_size as usize);
        (
            core::ptr::read_unaligned(p as *const u32),
            core::ptr::read_unaligned(p.add(8) as *const u64),
            core::ptr::read_unaligned(p.add(24) as *const u64),
        )
    }
}
fn conventional(info: &NovaBootInfo, n: usize) -> bool {
    let count = info.memory_map_size as usize / info.memory_descriptor_size as usize;
    let addr = n as u64 * FRAME_SIZE;
    for i in 0..count {
        let (kind, base, pages) = descriptor(info, i);
        if kind == 7
            && addr >= base
            && addr
                .checked_sub(base)
                .map_or(false, |x| x / FRAME_SIZE < pages)
        {
            return true;
        }
    }
    false
}
pub fn init(info: &NovaBootInfo) -> Result<Stats, Error> {
    let stride = info.memory_descriptor_size as usize;
    if info.memory_map == 0
        || stride < 40
        || stride > 4096
        || info.memory_map_size == 0
        || info.memory_map_size as usize % stride != 0
        || info.memory_map_size > 256 * 1024
    {
        return Err(Error::InvalidMap);
    }
    let was = lock();
    // SAFETY: initialization is one-shot under LOCK and before hardware IRQs are enabled.
    unsafe {
        if INITIALIZED {
            let s = stats_locked();
            unlock(was);
            return Ok(s);
        }
        for i in 0..info.memory_map_size as usize / stride {
            let (kind, base, pages) = descriptor(info, i);
            if (1..=10).contains(&kind) || kind == 14 {
                TOTAL = TOTAL.saturating_add(pages.saturating_mul(FRAME_SIZE));
            }
            if kind == 7 && base % FRAME_SIZE == 0 {
                let descriptor_start = (base / FRAME_SIZE) as usize;
                let start = descriptor_start.max(256);
                let end = descriptor_start
                    .saturating_add(pages.min(usize::MAX as u64) as usize)
                    .min(MAX_FRAMES);
                for n in start..end {
                    let a = n as u64 * FRAME_SIZE;
                    if a >= info.kernel_base
                        && a < info.kernel_base.saturating_add(info.kernel_size)
                    {
                        continue;
                    }
                    set_free(n, true);
                    FREE += FRAME_SIZE;
                    USABLE += FRAME_SIZE;
                }
            }
        }
        INITIALIZED = true;
        let s = stats_locked();
        unlock(was);
        Ok(s)
    }
}
unsafe fn stats_locked() -> Stats {
    Stats {
        total_bytes: TOTAL,
        usable_bytes: USABLE,
        free_bytes: FREE,
        used_bytes: USABLE - FREE,
        reserved_bytes: TOTAL.saturating_sub(USABLE),
    }
}
pub fn stats() -> Stats {
    let was = lock(); // SAFETY: protected by LOCK.
    let s = unsafe { stats_locked() };
    unlock(was);
    s
}
pub fn allocate_frame() -> Result<PhysFrame, Error> {
    let was = lock();
    // SAFETY: protected by LOCK; bitmap bounds are fixed.
    let result = unsafe {
        if !INITIALIZED {
            Err(Error::NotInitialized)
        } else {
            let mut found = None;
            for step in 0..WORDS {
                let word = (NEXT / 64 + step) % WORDS;
                let bits = (*(&raw const FREE_BITS))[word];
                if bits != 0 {
                    let n = word * 64 + bits.trailing_zeros() as usize;
                    found = Some(n);
                    break;
                }
            }
            match found {
                Some(n) => {
                    set_free(n, false);
                    set_allocated(n, true);
                    FREE -= FRAME_SIZE;
                    NEXT = n + 1;
                    Ok(PhysFrame(PhysAddr(n as u64 * FRAME_SIZE)))
                }
                None => Err(Error::OutOfMemory),
            }
        }
    };
    unlock(was);
    result
}
pub fn free_frame(info: &NovaBootInfo, frame: PhysFrame) -> Result<(), Error> {
    let addr = frame.address();
    if addr % FRAME_SIZE != 0 || addr / FRAME_SIZE >= MAX_FRAMES as u64 {
        return Err(Error::InvalidFrame);
    }
    let n = (addr / FRAME_SIZE) as usize;
    if n < 256
        || !conventional(info, n)
        || (addr >= info.kernel_base && addr < info.kernel_base.saturating_add(info.kernel_size))
    {
        return Err(Error::InvalidFrame);
    }
    free_allocated_frame(frame)
}
/// Reclaims a frame previously returned by allocate_frame. The allocation bitmap is
/// authoritative: it contains only validated UEFI ConventionalMemory frames.
pub fn free_allocated_frame(frame: PhysFrame) -> Result<(), Error> {
    let addr = frame.address();
    if addr % FRAME_SIZE != 0 || addr / FRAME_SIZE >= MAX_FRAMES as u64 || addr < 0x100000 {
        return Err(Error::InvalidFrame);
    }
    let n = (addr / FRAME_SIZE) as usize;
    let was = lock();
    // SAFETY: protected by LOCK and frame validated against the UEFI map.
    let result = unsafe {
        if !INITIALIZED {
            Err(Error::NotInitialized)
        } else if !is_allocated(n) {
            Err(Error::DoubleFree)
        } else {
            set_allocated(n, false);
            set_free(n, true);
            FREE += FRAME_SIZE;
            Ok(())
        }
    };
    unlock(was);
    result
}
