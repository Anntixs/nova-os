#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub rsp: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}
impl Context {
    pub const fn new(rsp: u64) -> Self {
        Self {
            rsp,
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }
}
