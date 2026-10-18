use crate::{arch::x86_64::cpu,drivers::pci::{self,PciDevice}};
use core::{ptr, sync::atomic::{fence,Ordering}};
use super::{BlockDevice,Error};

const QUEUE_BYTES:usize=32768;
const DMA_BYTES:usize=4096;
#[repr(align(4096))]
struct AlignedQueue([u8;QUEUE_BYTES]);
#[repr(align(4096))]
struct AlignedDma([u8;DMA_BYTES]);
static mut QUEUE:AlignedQueue=AlignedQueue([0;QUEUE_BYTES]);
static mut DMA:AlignedDma=AlignedDma([0;DMA_BYTES]);
#[repr(C)]
#[derive(Clone,Copy)]
struct Desc {addr:u64,len:u32,flags:u16,next:u16}

pub struct VirtioBlock {base:u16,capacity:u64,queue_size:u16,avail_idx:u16,used_idx:u16,used_off:usize}
impl VirtioBlock {
    pub fn init(pci:&PciDevice)->Result<Self,Error> {
        crate::nova_log!(Info,"BLOCK","VirtIO BAR0={:#x}",pci.bars[0]);
        if pci.vendor!=0x1af4 || pci.device!=0x1001 || pci.bars[0]&1==0 {return Err(Error::Unsupported);}
        let base=(pci.bars[0]&!3) as u16;
        if base==0 || base>0xffe0 {return Err(Error::InvalidDevice);}
        pci::enable_io_busmaster(pci);
        cpu::port_out(base+0x12,0);
        cpu::port_out(base+0x12,1); // ACKNOWLEDGE
        cpu::port_out(base+0x12,3); // DRIVER
        let _features=cpu::port_in32(base);
        cpu::port_out32(base+4,0); // no optional features negotiated
        cpu::port_out16(base+0x0e,0);
        let queue_size=cpu::port_in16(base+0x0c);
        crate::nova_log!(Info,"BLOCK","VirtIO queue size={}",queue_size);
        if queue_size<3 || queue_size>1024 {return Err(Error::Unsupported);}
        let used_off=((queue_size as usize*16+4+queue_size as usize*2+2+4095)&!4095);
        if used_off+4+queue_size as usize*8+2>QUEUE_BYTES {return Err(Error::InvalidDevice);}
        // SAFETY: these aligned static buffers remain pinned for the entire kernel lifetime.
        // Only the one block driver owns them and calls are serialized by the block registry.
        let queue=(&raw mut QUEUE) as *mut u8 as u64;
        let dma=(&raw mut DMA) as *mut u8 as u64;
        if queue&4095!=0 || dma&4095!=0 || queue>>44!=0 || dma>>32!=0 {return Err(Error::InvalidDevice);}
        // SAFETY: the driver has exclusive access to the static buffers before queue registration.
        unsafe {ptr::write_bytes(queue as *mut u8,0,QUEUE_BYTES);ptr::write_bytes(dma as *mut u8,0,DMA_BYTES);}
        cpu::port_out32(base+8,(queue>>12) as u32);
        cpu::port_out(base+0x12,7); // DRIVER_OK
        if cpu::port_in(base+0x12)&0x80!=0 {return Err(Error::InvalidDevice);}
        let capacity=cpu::port_in32(base+0x14) as u64 | ((cpu::port_in32(base+0x18) as u64)<<32);
        if capacity==0 {return Err(Error::InvalidDevice);}
        Ok(Self{base,capacity,queue_size,avail_idx:0,used_idx:0,used_off})
    }
    fn transfer(&mut self,lba:u64,block:&mut [u8;512],write:bool)->Result<(),Error> {
        if lba>=self.capacity {return Err(Error::OutOfRange);}
        let queue=(&raw mut QUEUE) as *mut u8;
        let dma=(&raw mut DMA) as *mut u8;
        // SAFETY: buffers are pinned, aligned, and exclusively owned while the block registry
        // masks interrupts. A request cannot be replaced before the used index advances.
        unsafe {
            ptr::write_unaligned(dma as *mut u32,if write {1} else {0});
            ptr::write_unaligned(dma.add(4) as *mut u32,0);
            ptr::write_unaligned(dma.add(8) as *mut u64,lba);
            if write {ptr::copy_nonoverlapping(block.as_ptr(),dma.add(512),512);}
            ptr::write_volatile(dma.add(1024),0xff);
            let phys=dma as u64;
            let desc=queue as *mut Desc;
            ptr::write_volatile(desc,Desc{addr:phys,len:16,flags:1,next:1});
            ptr::write_volatile(desc.add(1),Desc{addr:phys+512,len:512,flags:if write {1} else {3},next:2});
            ptr::write_volatile(desc.add(2),Desc{addr:phys+1024,len:1,flags:2,next:0});
            let ring=queue.add(self.queue_size as usize*16+4) as *mut u16;
            ptr::write_volatile(ring.add(self.avail_idx as usize%self.queue_size as usize),0);
            fence(Ordering::SeqCst);
            self.avail_idx=self.avail_idx.wrapping_add(1);
            ptr::write_volatile(queue.add(self.queue_size as usize*16+2) as *mut u16,self.avail_idx);
        }
        fence(Ordering::SeqCst);
        cpu::port_out16(self.base+0x10,0);
        // SAFETY: used_off was bounded by QUEUE_BYTES during initialization, and the
        // pinned queue stays alive for the entire request.
        let used=unsafe {queue.add(self.used_off+2) as *const u16};
        let mut done=false;
        for _ in 0..20_000_000 {
            // SAFETY: device owns the used index after queue registration.
            if unsafe {ptr::read_volatile(used)} != self.used_idx {done=true;break;}
            core::hint::spin_loop();
        }
        if !done {return Err(Error::Timeout);}
        self.used_idx=self.used_idx.wrapping_add(1);
        fence(Ordering::SeqCst);
        // SAFETY: completion makes the status and data buffer visible to the CPU.
        let status=unsafe {ptr::read_volatile(dma.add(1024))};
        if status!=0 {return Err(Error::Io);}
        // SAFETY: completion succeeded and both buffers contain at least 512 bytes.
        if !write {unsafe {ptr::copy_nonoverlapping(dma.add(512),block.as_mut_ptr(),512);}}
        Ok(())
    }
}
impl BlockDevice for VirtioBlock {
    fn block_size(&self)->usize {512}
    fn block_count(&self)->u64 {self.capacity}
    fn read_blocks(&mut self,lba:u64,buffer:&mut [u8])->Result<(),Error> {
        if buffer.len()%512!=0 {return Err(Error::InvalidBuffer);}
        if lba.checked_add((buffer.len()/512) as u64).filter(|&v|v<=self.capacity).is_none() {return Err(Error::OutOfRange);}
        for (i,chunk) in buffer.chunks_exact_mut(512).enumerate() {
            let mut block=[0u8;512];self.transfer(lba+i as u64,&mut block,false)?;chunk.copy_from_slice(&block);
        }
        Ok(())
    }
    fn write_blocks(&mut self,lba:u64,buffer:&[u8])->Result<(),Error> {
        if buffer.len()%512!=0 {return Err(Error::InvalidBuffer);}
        if lba.checked_add((buffer.len()/512) as u64).filter(|&v|v<=self.capacity).is_none() {return Err(Error::OutOfRange);}
        for (i,chunk) in buffer.chunks_exact(512).enumerate() {
            let mut block=[0u8;512];block.copy_from_slice(chunk);self.transfer(lba+i as u64,&mut block,true)?;
        }
        Ok(())
    }
}
