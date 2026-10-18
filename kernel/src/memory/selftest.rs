extern crate alloc;
use super::{
    heap,
    paging::{self, PageFlags},
    physical, Page, PhysAddr, VirtAddr,
};
use alloc::{boxed::Box, vec::Vec};
use core::alloc::Layout;
use nova_core::NovaBootInfo;

pub fn run(info: &NovaBootInfo) {
    let before = physical::stats();
    let frame = physical::allocate_frame().expect("frame allocation");
    physical::free_frame(info, frame).expect("frame free");
    assert!(
        physical::free_frame(info, frame).is_err(),
        "double free accepted"
    );
    let frame = physical::allocate_frame().expect("frame reuse");
    let page = Page(VirtAddr(0xffff_9100_0000_0000));
    paging::map_page(page, frame, PageFlags::KERNEL_RW_NX).expect("page map");
    assert_eq!(paging::translate(page.0), Some(PhysAddr(frame.address())));
    // SAFETY: map_page installed a writable, present page for this address.
    unsafe {
        (page.address() as *mut u64).write_volatile(0x4e4f5641);
        assert_eq!((page.address() as *const u64).read_volatile(), 0x4e4f5641);
    }
    let result = paging::unmap_page(page).expect("page unmap");
    assert_eq!(result, frame);
    assert!(paging::translate(page.0).is_none());
    physical::free_frame(info, frame).expect("mapped frame free");
    let b = Box::new(0x12345678u64);
    assert_eq!(*b, 0x12345678);
    drop(b);
    let mut v = Vec::new();
    for i in 0..256u64 {
        v.push(i);
    }
    assert_eq!(v[255], 255);
    drop(v);
    let layout = Layout::from_size_align(128, 64).unwrap();
    // SAFETY: this test supplies the exact same Layout on deallocation and verifies non-null pointers.
    unsafe {
        let a = heap::allocator().alloc(layout);
        assert!(!a.is_null());
        assert_eq!((a as usize) % 64, 0);
        heap::allocator().dealloc(a, layout);
        let b = heap::allocator().alloc(layout);
        assert_eq!(a, b, "heap block not reused");
        heap::allocator().dealloc(b, layout);
    }
    assert_eq!(before.free_bytes - physical::stats().free_bytes, 3 * 4096);
    crate::log("NOVA_TEST_SUCCESS memory paging heap");
}
