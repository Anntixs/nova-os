//! Initial Nova user stack: [argc:u64, argv:*const *const u8, envp:*const *const u8]
//! at RSP % 16 == 8. Pointer arrays are NUL terminated and strings are UTF-8 NUL bytes.
use alloc::{string::String,vec::Vec};
use crate::memory::address_space::{AddressSpace,USER_STACK_TOP};
const STACK_BYTES:u64=8192;
fn put(space:&AddressSpace,at:u64,data:&[u8])->Result<(),&'static str>{
 let end=at.checked_add(data.len() as u64).ok_or("stack overflow")?;
 if at<USER_STACK_TOP-STACK_BYTES||end>USER_STACK_TOP{return Err("argv exceeds user stack")}
 for (i,b) in data.iter().enumerate(){
  let phys=space.translate_user(at+i as u64,true).ok_or("unmapped user stack")?;
  // SAFETY: a writable USER mapping was checked for each byte, and conventional
  // physical RAM is identity mapped on the bootstrap CPU.
  unsafe{(phys as *mut u8).write(*b)};
 }
 Ok(())
}
fn put64(space:&AddressSpace,at:u64,value:u64)->Result<(),&'static str>{put(space,at,&value.to_le_bytes())}
pub fn build(space:&AddressSpace,args:&[String],env:&[String])->Result<u64,&'static str>{
 if args.is_empty()||args.len()>32||env.len()>32{return Err("invalid argv/envp count")}
 let mut cursor=USER_STACK_TOP;
 let mut argptr=Vec::new();let mut envptr=Vec::new();
 for text in args{if text.len()>255||text.as_bytes().contains(&0){return Err("invalid argument")}
  cursor=cursor.checked_sub(text.len() as u64+1).ok_or("stack overflow")?;
  put(space,cursor,text.as_bytes())?;put(space,cursor+text.len() as u64,&[0])?;argptr.push(cursor);
 }
 for text in env{if text.len()>255||text.as_bytes().contains(&0){return Err("invalid environment")}
  cursor=cursor.checked_sub(text.len() as u64+1).ok_or("stack overflow")?;
  put(space,cursor,text.as_bytes())?;put(space,cursor+text.len() as u64,&[0])?;envptr.push(cursor);
 }
 cursor&=!7;
 cursor=cursor.checked_sub(((envptr.len()+1)*8) as u64).ok_or("stack overflow")?;
 let env_array=cursor;for(i,p)in envptr.iter().enumerate(){put64(space,cursor+i as u64*8,*p)?}put64(space,cursor+envptr.len() as u64*8,0)?;
 cursor=cursor.checked_sub(((argptr.len()+1)*8) as u64).ok_or("stack overflow")?;
 let arg_array=cursor;for(i,p)in argptr.iter().enumerate(){put64(space,cursor+i as u64*8,*p)?}put64(space,cursor+argptr.len() as u64*8,0)?;
 let header=cursor.checked_sub(32).ok_or("stack overflow")? & !15 | 8;
 put64(space,header,args.len() as u64)?;put64(space,header+8,arg_array)?;put64(space,header+16,env_array)?;
 Ok(header)
}
