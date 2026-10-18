pub mod heap;
pub mod paging;
pub mod physical;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysAddr(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtAddr(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysFrame(pub PhysAddr);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page(pub VirtAddr);
impl PhysFrame {
    pub fn address(self) -> u64 {
        self.0 .0
    }
}
impl Page {
    pub fn address(self) -> u64 {
        self.0 .0
    }
}
#[cfg(feature = "selftest")]
pub mod selftest;
pub mod address_space;
