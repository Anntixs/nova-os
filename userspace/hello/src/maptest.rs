#![no_std]
#![no_main]
mod common;
use common::{call,write,exit};
#[no_mangle]
pub extern "C" fn _start()->!{
 if call(nova_core::abi::MAP,4096,7,0)!=-22{write(b"W+X accepted\n");exit(1)}
 for _ in 0..8{
  let p=call(nova_core::abi::MAP,8192,3,0);
  if p<0{write(b"map failed\n");exit(2)}
  let data=p as *mut u64;
  // SAFETY: MAP returned two writable USER pages owned by this process.
  unsafe{data.write_volatile(0x1234_5678_9abc_def0);data.add(1023).write_volatile(42);
   if data.read_volatile()!=0x1234_5678_9abc_def0||data.add(1023).read_volatile()!=42{write(b"map data failed\n");exit(3)}}
  if call(nova_core::abi::UNMAP,p as u64,8192,0)!=0{write(b"unmap failed\n");exit(4)}
 }
 write(b"Map test passed.\n");exit(0)
}
