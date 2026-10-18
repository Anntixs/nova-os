#![no_std]
#![no_main]
mod common;
use common::{call,write,exit};
#[no_mangle]
pub extern "C" fn _start()->!{
 let before=call(nova_core::abi::CLOCK_GET,0,0,0);
 if before<0||call(nova_core::abi::SLEEP,20,0,0)!=0{write(b"sleep failed\n");exit(1)}
 let after=call(nova_core::abi::CLOCK_GET,0,0,0);
 if after-before<20{write(b"sleep returned early\n");exit(2)}
 write(b"Sleep test passed.\n");exit(0)
}
