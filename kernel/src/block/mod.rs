pub mod virtio;
use core::cell::UnsafeCell;
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
struct Registry(UnsafeCell<Option<VirtioBlock>>);
// SAFETY: Nova currently has one active CPU; every block request runs with IRQs masked.
unsafe impl Sync for Registry {}
static REGISTRY:Registry=Registry(UnsafeCell::new(None));
pub fn init(devices:&[PciDevice])->Result<(),Error> {
    let pci=devices.iter().find(|d|d.vendor==0x1af4&&d.device==0x1001).ok_or(Error::Unavailable)?;
    let device=VirtioBlock::init(pci)?;
    crate::nova_log!(Info,"BLOCK","registered vda: {} blocks of {} bytes",device.block_count(),device.block_size());
    // SAFETY: one-shot initialization on BSP before VFS or other block users exist.
    unsafe {*REGISTRY.0.get()=Some(device);}
    Ok(())
}
pub fn with_device<R>(f:impl FnOnce(&mut dyn BlockDevice)->Result<R,Error>)->Result<R,Error> {
    let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
    // SAFETY: BSP-only and IRQs are masked, so no caller can overlap a request.
    let result=unsafe {match (*REGISTRY.0.get()).as_mut() {Some(d)=>f(d),None=>Err(Error::Unavailable)}};
    if enabled {cpu::enable_interrupts();}
    result
}
pub fn capacity()->Result<(u64,usize),Error> {with_device(|d|Ok((d.block_count(),d.block_size())))}
pub fn read(lba:u64,buffer:&mut [u8])->Result<(),Error> {with_device(|d|d.read_blocks(lba,buffer))}
pub fn write(lba:u64,buffer:&[u8])->Result<(),Error> {with_device(|d|d.write_blocks(lba,buffer))}
