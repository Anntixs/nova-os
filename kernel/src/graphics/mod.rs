//! Controlled framebuffer primitive. Only the lease owner may draw. User
//! memory is copied/translated by syscall code; this module sees RGB values.
use core::sync::atomic::{AtomicBool,AtomicU64,Ordering};
use crate::process::ProcessId;
static OWNER:AtomicU64=AtomicU64::new(0);
static DRAW_LOCK:AtomicBool=AtomicBool::new(false);
struct DrawGuard;
impl DrawGuard {
 fn acquire()->Self {
  while DRAW_LOCK.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
  Self
 }
}
impl Drop for DrawGuard {fn drop(&mut self){DRAW_LOCK.store(false,Ordering::Release)}}
/// The kernel console may render only while no userspace process owns GOP.
pub fn console_visible()->bool{OWNER.load(Ordering::Acquire)==0}

pub struct Rect {pub x:u32,pub y:u32,pub width:u32,pub height:u32}
pub fn info()->Option<[u8;24]>{
 let fb=crate::framebuffer()?;
 let mut data=[0u8;24];
 for (i,value) in [fb.width,fb.height,fb.stride,fb.format,32,fb.stride.saturating_mul(4)].iter().enumerate(){
  data[i*4..i*4+4].copy_from_slice(&value.to_le_bytes());
 }Some(data)
}
pub fn acquire(pid:ProcessId)->Result<(),i64>{
 let _guard=DrawGuard::acquire();
 if crate::framebuffer().is_none(){return Err(-19)}
 match OWNER.compare_exchange(0,pid.0,Ordering::AcqRel,Ordering::Acquire){
  Ok(_)=>Ok(()),Err(old) if old==pid.0=>Ok(()),Err(_)=>Err(-16),
 }
}
pub fn release(pid:ProcessId)->Result<(),i64>{
 let _guard=DrawGuard::acquire();
 OWNER.compare_exchange(pid.0,0,Ordering::AcqRel,Ordering::Acquire).map_err(|_|-1)?;
 crate::process::clear_output();
 Ok(())
}
pub fn release_if_owner(pid:ProcessId){
 let _guard=DrawGuard::acquire();
 if OWNER.compare_exchange(pid.0,0,Ordering::AcqRel,Ordering::Acquire).is_ok(){
  crate::process::clear_output();
 }
}
fn check(pid:ProcessId,rect:&Rect)->Result<nova_core::Framebuffer,i64>{
 if OWNER.load(Ordering::Acquire)!=pid.0{return Err(-1)}
 let fb=crate::framebuffer().ok_or(-19i64)?;
 if rect.width==0||rect.height==0||rect.x.checked_add(rect.width).is_none_or(|x|x>fb.width)
  ||rect.y.checked_add(rect.height).is_none_or(|y|y>fb.height){return Err(-22)}
 Ok(fb)
}
pub fn fill(pid:ProcessId,rect:Rect,color:u32)->Result<(),i64>{
 let _guard=DrawGuard::acquire();
 let fb=check(pid,&rect)?;
 for y in rect.y..rect.y+rect.height {for x in rect.x..rect.x+rect.width {
  crate::pixel(&fb,x as usize,y as usize,color)
 }}Ok(())
}
pub fn blit(pid:ProcessId,rect:Rect,stride:u32,mut get:impl FnMut(u64)->Result<u32,i64>)->Result<(),i64>{
 let _guard=DrawGuard::acquire();
 let fb=check(pid,&rect)?;
 if stride<rect.width{return Err(-22)}
 for y in 0..rect.height {for x in 0..rect.width {
  let offset=(y as u64).checked_mul(stride as u64).and_then(|v|v.checked_add(x as u64)).and_then(|v|v.checked_mul(4)).ok_or(-22i64)?;
  let rgb=get(offset)?;
  crate::pixel(&fb,(rect.x+x) as usize,(rect.y+y) as usize,rgb);
 }}Ok(())
}
