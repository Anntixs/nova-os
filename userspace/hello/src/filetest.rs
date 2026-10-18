#![no_std]
#![no_main]
mod common;
use common::{call,write,exit};
#[no_mangle]
pub extern "C" fn _start()->! {
 write(b"Opening /etc/version...\n");
 let fd=call(5,b"/etc/version\0".as_ptr() as u64,0,0);
 if fd<0 {write(b"Open failed\n");exit(1)}
 if call(5,0xffff_8000_0000_0000,0,0)!=-14 || call(7,fd as u64,0xffff_8000_0000_0000,1)!=-14 {write(b"User pointer validation failed\n");exit(11)}
 let mut buffer=[0u8;128];
 let n=call(7,fd as u64,buffer.as_mut_ptr() as u64,buffer.len() as u64);
 if n<0 {write(b"Read failed\n");exit(2)}
 write(&buffer[..n as usize]);
 let mut stat=[0u8;16];
 if call(9,fd as u64,stat.as_mut_ptr() as u64,0)!=0 || u64::from_le_bytes(stat[8..16].try_into().unwrap())!=n as u64 {write(b"Stat failed\n");exit(4)}
 if call(8,fd as u64,0,0)!=0 || call(7,fd as u64,buffer.as_mut_ptr() as u64,buffer.len() as u64)!=n {write(b"Seek failed\n");exit(5)}
 if call(8,fd as u64,999,0)!=999 || call(7,fd as u64,buffer.as_mut_ptr() as u64,buffer.len() as u64)!=0 {write(b"EOF failed\n");exit(9)}
 if call(6,fd as u64,0,0)<0 {write(b"Close failed\n");exit(3)}
 if call(6,fd as u64,0,0)!=-9 {write(b"Invalid FD failed\n");exit(6)}
 let dir=call(5,b"/bin\0".as_ptr() as u64,0,0);
 let mut entry=[0u8;272];
 if dir<0 || call(10,dir as u64,entry.as_mut_ptr() as u64,272)!=1 || u32::from_le_bytes(entry[4..8].try_into().unwrap())==0 {write(b"Readdir failed\n");exit(7)}
 let _=call(6,dir as u64,0,0);
 let output=call(5,b"/home/filetest.txt\0".as_ptr() as u64,0x301,0);
 if output<0 || call(1,output as u64,b"Userspace wrote this\n".as_ptr() as u64,21)!=21 {write(b"File write failed\n");exit(8)}
 let _=call(6,output as u64,0,0);
 let check=call(5,b"/home/filetest.txt\0".as_ptr() as u64,0,0);
 if check<0 || call(7,check as u64,buffer.as_mut_ptr() as u64,buffer.len() as u64)!=21 || &buffer[..21]!=b"Userspace wrote this\n" {write(b"File verify failed\n");exit(10)}
 let _=call(6,check as u64,0,0);
 write(b"File test passed.\n");exit(0)
}
