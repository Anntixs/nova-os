use super::{
    paging::{self, PageFlags},
    physical,
};
use super::{Page, PhysFrame, VirtAddr};
use crate::arch::x86_64::cpu;
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

pub const HEAP_BASE: u64 = 0xffff_9000_0000_0000;
pub const HEAP_SIZE: usize = 1024 * 1024;
#[repr(C)]
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}
#[repr(C)]
struct Allocation {
    start: usize,
    size: usize,
}
struct State {
    head: *mut FreeBlock,
    ready: bool,
}
pub struct Heap {
    lock: AtomicBool,
    state: UnsafeCell<State>,
}
// SAFETY: every access to State and its free-list nodes is serialized by lock with local IRQs disabled.
unsafe impl Sync for Heap {}
#[global_allocator]
static HEAP: Heap = Heap {
    lock: AtomicBool::new(false),
    state: UnsafeCell::new(State {
        head: ptr::null_mut(),
        ready: false,
    }),
};
fn align_up(x: usize, a: usize) -> Option<usize> {
    x.checked_add(a - 1).map(|v| v & !(a - 1))
}
impl Heap {
    fn lock(&self) -> bool {
        let was = cpu::interrupts_enabled();
        cpu::disable_interrupts();
        while self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        was
    }
    fn unlock(&self, was: bool) {
        self.lock.store(false, Ordering::Release);
        if was {
            cpu::enable_interrupts();
        }
    }
}
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let was = self.lock();
        // SAFETY: lock excludes concurrent accesses; every node is within mapped heap memory.
        let result = unsafe {
            let state = &mut *self.state.get();
            if !state.ready {
                ptr::null_mut()
            } else {
                let mut link = &mut state.head as *mut *mut FreeBlock;
                let mut result = ptr::null_mut();
                while !(*link).is_null() {
                    let block = *link;
                    let start = block as usize;
                    let size = (*block).size;
                    let Some(user) = align_up(
                        start + core::mem::size_of::<Allocation>(),
                        layout.align().max(16),
                    ) else {
                        break;
                    };
                    let Some(end) = user
                        .checked_add(layout.size().max(1))
                        .and_then(|n| align_up(n, 16))
                    else {
                        break;
                    };
                    if end <= start + size {
                        let rest = start + size - end;
                        let next = (*block).next;
                        let consumed = if rest >= core::mem::size_of::<FreeBlock>() {
                            let tail = end as *mut FreeBlock;
                            tail.write(FreeBlock { size: rest, next });
                            *link = tail;
                            end - start
                        } else {
                            *link = next;
                            size
                        };
                        ((user - core::mem::size_of::<Allocation>()) as *mut Allocation).write(
                            Allocation {
                                start,
                                size: consumed,
                            },
                        );
                        result = user as *mut u8;
                        break;
                    }
                    link = &mut (*block).next;
                }
                result
            }
        };
        self.unlock(was);
        result
    }
    unsafe fn dealloc(&self, p: *mut u8, _layout: Layout) {
        if p.is_null() {
            return;
        }
        let was = self.lock();
        // SAFETY: GlobalAlloc requires p to come from this allocator and be freed once. Lock protects the list.
        unsafe {
            let state = &mut *self.state.get();
            let alloc =
                ((p as usize - core::mem::size_of::<Allocation>()) as *const Allocation).read();
            let mut link = &mut state.head as *mut *mut FreeBlock;
            while !(*link).is_null() && (*link as usize) < alloc.start {
                link = &mut (**link).next;
            }
            let node = alloc.start as *mut FreeBlock;
            node.write(FreeBlock {
                size: alloc.size,
                next: *link,
            });
            *link = node;
            if !(*node).next.is_null() && (node as usize) + (*node).size == (*node).next as usize {
                let next = (*node).next;
                (*node).size += (*next).size;
                (*node).next = (*next).next;
            }
            let head = &mut state.head as *mut *mut FreeBlock;
            let mut prev = *head;
            while !prev.is_null() && (*prev).next != node {
                prev = (*prev).next;
            }
            if !prev.is_null() && (prev as usize) + (*prev).size == node as usize {
                (*prev).size += (*node).size;
                (*prev).next = (*node).next;
            }
        }
        self.unlock(was);
    }
}
pub fn init(nx: bool) -> Result<(), paging::Error> {
    for offset in (0..HEAP_SIZE).step_by(4096) {
        let frame = physical::allocate_frame().map_err(|_| paging::Error::OutOfFrames)?;
        paging::map_page(
            Page(VirtAddr(HEAP_BASE + offset as u64)),
            PhysFrame(frame.0),
            if nx {
                PageFlags::KERNEL_RW_NX
            } else {
                PageFlags::WRITABLE
            },
        )?;
    }
    let was = HEAP.lock();
    // SAFETY: all heap pages are mapped and no allocation has occurred. Lock protects State.
    unsafe {
        let s = &mut *HEAP.state.get();
        let node = HEAP_BASE as *mut FreeBlock;
        node.write(FreeBlock {
            size: HEAP_SIZE,
            next: ptr::null_mut(),
        });
        s.head = node;
        s.ready = true;
    }
    HEAP.unlock(was);
    Ok(())
}
#[cfg(feature = "selftest")]
pub fn allocator() -> &'static dyn GlobalAlloc {
    &HEAP
}
