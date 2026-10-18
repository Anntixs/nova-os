#![no_std]
#![no_main]
mod common;
use common::{call,write,exit};
#[no_mangle]
pub extern "C" fn _start()->!{
 let pid=call(nova_core::abi::SPAWN,b"/bin/exit42\0".as_ptr() as u64,0,0);
 if pid<1{write(b"spawn failed\n");exit(1)}
 let mut code=0i32;
 let got=call(nova_core::abi::WAIT,pid as u64,&mut code as *mut i32 as u64,0);
 if got!=pid||code!=42{write(b"wait failed\n");exit(2)}
 write(b"Spawn test passed. Child exited with code 42\n");exit(0)
}
