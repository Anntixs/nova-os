//! Nova ABI integer wire values. Keep these numbers stable across kernel and userspace.
pub const WRITE:u64=1;
pub const EXIT:u64=2;
pub const GETPID:u64=3;
pub const YIELD:u64=4;
pub const OPEN:u64=5;
pub const CLOSE:u64=6;
pub const READ:u64=7;
pub const SEEK:u64=8;
pub const STAT:u64=9;
pub const READDIR:u64=10;
pub const SPAWN:u64=11;
pub const WAIT:u64=12;
pub const GETCWD:u64=13;
pub const CHDIR:u64=14;
pub const MAP:u64=15;
pub const UNMAP:u64=16;
pub const CLOCK_GET:u64=17;
pub const SLEEP:u64=18;
pub const PIPE:u64=19;
pub const CHANNEL_CREATE:u64=20;
pub const CHANNEL_SEND:u64=21;
pub const CHANNEL_RECEIVE:u64=22;
pub const THREAD_CREATE:u64=23;
pub const THREAD_EXIT:u64=24;
pub const THREAD_JOIN:u64=25;
pub const FB_INFO:u64=26;
pub const FB_ACQUIRE:u64=27;
pub const FB_RELEASE:u64=28;
pub const FB_FILL:u64=29;
pub const FB_BLIT:u64=30;
pub const TTY_CLEAR:u64=31;
pub const KILL:u64=32;
pub const WAIT_TRY:u64=33;
pub const INPUT_ACQUIRE:u64=34;
pub const INPUT_RELEASE:u64=35;
// Numbers added for ABI 0.3 are reserved here only when implemented by the kernel.
pub const ENOENT:i64=-2;
pub const EIO:i64=-5;
pub const EBADF:i64=-9;
pub const ENOMEM:i64=-12;
pub const EFAULT:i64=-14;
pub const EEXIST:i64=-17;
pub const ENOTDIR:i64=-20;
pub const EISDIR:i64=-21;
pub const EINVAL:i64=-22;
pub const ENOSYS:i64=-38;
pub const ENOTSUP:i64=-95;
pub const EAGAIN:i64=-11;
pub const EMSGSIZE:i64=-90;
pub const EPIPE:i64=-32;
#[repr(C)]
#[derive(Clone,Copy)]
pub struct Stat {pub kind:u32,pub reserved:u32,pub size:u64}
#[repr(C)]
#[derive(Clone,Copy)]
pub struct DirEntry {pub kind:u32,pub name_length:u32,pub name:[u8;264]}
