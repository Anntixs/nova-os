pub mod virtio;
use core::{cell::UnsafeCell,sync::atomic::{AtomicBool,Ordering}};
use crate::{arch::x86_64::cpu,drivers::pci::PciDevice};
use virtio::VirtioBlock;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Error {Unavailable,Unsupported,InvalidDevice,InvalidBuffer,OutOfRange,Timeout,Io}
pub trait BlockDevice {
    fn block_size(&self)->usize;
    fn block_count(&self)->u64;
    fn read_blocks(&mut self,lba:u64,buffer:&mut [u8])->Result<(),Error>;
    fn write_blocks(&mut self,lba:u64,buffer:&[u8])->Result<(),Error>;
}
struct Registry {device:UnsafeCell<Option<VirtioBlock>>, lock:AtomicBool}
// SAFETY: every mutable device access holds the atomic lock with local IRQs
// masked. The device and its DMA buffers are initialized before AP startup.
unsafe impl Sync for Registry {}
static REGISTRY:Registry=Registry{device:UnsafeCell::new(None),lock:AtomicBool::new(false)};
pub fn init(devices:&[PciDevice])->Result<(),Error> {
    let pci=devices.iter().find(|d|d.vendor==0x1af4&&d.device==0x1001).ok_or(Error::Unavailable)?;
    let device=VirtioBlock::init(pci)?;
    crate::nova_log!(Info,"BLOCK","registered vda: {} blocks of {} bytes",device.block_count(),device.block_size());
    // SAFETY: one-shot initialization on BSP before VFS or other block users exist.
    unsafe {*REGISTRY.device.get()=Some(device);}
    Ok(())
}
pub fn with_device<R>(f:impl FnOnce(&mut dyn BlockDevice)->Result<R,Error>)->Result<R,Error> {
    let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
    while REGISTRY.lock.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
    // SAFETY: the atomic lock excludes other CPUs and IRQ masking prevents
    // reentry on this CPU while the unique device reference is borrowed.
    let result=unsafe {match (*REGISTRY.device.get()).as_mut() {Some(d)=>f(d),None=>Err(Error::Unavailable)}};
    REGISTRY.lock.store(false,Ordering::Release);
    if enabled {cpu::enable_interrupts();}
    result
}
pub fn capacity()->Result<(u64,usize),Error> {with_device(|d|Ok((d.block_count(),d.block_size())))}
pub fn read(lba:u64,buffer:&mut [u8])->Result<(),Error> {with_device(|d|d.read_blocks(lba,buffer))}
pub fn write(lba:u64,buffer:&[u8])->Result<(),Error> {with_device(|d|d.write_blocks(lba,buffer))}
