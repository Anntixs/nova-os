use alloc::vec::Vec;
use crate::arch::x86_64::cpu;

#[derive(Clone, Copy, Debug)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor: u16,
    pub device: u16,
    pub class: u8,
    pub subclass: u8,
    pub bars: [u32; 6],
}
fn address(bus:u8,slot:u8,function:u8,offset:u8)->u32 {
    0x8000_0000 | (bus as u32)<<16 | (slot as u32)<<11 | (function as u32)<<8 | (offset as u32 & 0xfc)
}
pub fn read32(bus:u8,slot:u8,function:u8,offset:u8)->u32 {
    cpu::port_out32(0xcf8,address(bus,slot,function,offset));
    cpu::port_in32(0xcfc)
}
fn write16(bus:u8,slot:u8,function:u8,offset:u8,value:u16) {
    cpu::port_out32(0xcf8,address(bus,slot,function,offset));
    cpu::port_out16(0xcfc + (offset as u16 & 2),value);
}
pub fn enable_io_busmaster(device:&PciDevice) {
    let value=read32(device.bus,device.slot,device.function,4) as u16;
    // INTx is disabled because the first VirtIO driver polls its used ring.
    write16(device.bus,device.slot,device.function,4,(value|0x5|0x400));
}
pub fn scan()->Vec<PciDevice> {
    crate::nova_log!(Info,"PCI","enumerating devices");
    let mut found=Vec::new();
    for bus in 0..=255u8 {
        for slot in 0..32u8 {
            let first=read32(bus,slot,0,0);
            if first as u16==0xffff {continue;}
            let header=read32(bus,slot,0,0x0c);
            let functions=if header & (1<<23)!=0 {8} else {1};
            for function in 0..functions {
                let id=read32(bus,slot,function,0);
                if id as u16==0xffff {continue;}
                let kind=read32(bus,slot,function,8);
                let mut bars=[0;6];
                for (i,bar) in bars.iter_mut().enumerate() {*bar=read32(bus,slot,function,0x10+i as u8*4);}
                let d=PciDevice {bus,slot,function,vendor:id as u16,device:(id>>16) as u16,class:(kind>>24) as u8,subclass:(kind>>16) as u8,bars};
                crate::nova_log!(Info,"PCI","{:02x}:{:02x}.{} {:04x}:{:04x} class {:02x}:{:02x}",bus,slot,function,d.vendor,d.device,d.class,d.subclass);
                found.push(d);
            }
        }
    }
    found
}
