//! Small Nova device namespace. Device paths are resolved through VFS; the
//! descriptor layer dispatches operations to these abstractions, not drivers.
use alloc::{boxed::Box,string::{String,ToString},vec::Vec};
use crate::fs::{fat32,vfs::{self,FileSystem,Node}};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind { Null, Zero, Console, Framebuffer, Keyboard }
pub const NAMES: &[(&str,Kind)] = &[
    ("null",Kind::Null),("zero",Kind::Zero),("console",Kind::Console),
    ("fb0",Kind::Framebuffer),("kbd0",Kind::Keyboard),
];
pub fn identify(path:&str)->Option<Kind>{
 let name=path.strip_prefix("/dev/")?;
 NAMES.iter().find(|(n,_)|*n==name).map(|(_,kind)|*kind)
}
fn not_found()->vfs::Error{fat32::Error::NotFound.into()}
pub struct DeviceFs;
impl FileSystem for DeviceFs {
 fn lookup(&self,path:&str)->Result<Node,vfs::Error>{
  if path=="/"{return Ok(Node{name:String::from("dev"),size:0,directory:true})}
  let name=path.strip_prefix('/').ok_or_else(not_found)?;
  if NAMES.iter().any(|(n,_)|*n==name){Ok(Node{name:name.to_string(),size:0,directory:false})}else{Err(not_found())}
 }
 fn readdir(&self,path:&str)->Result<Vec<Node>,vfs::Error>{
  if path!="/"{return Err(not_found())}
  Ok(NAMES.iter().map(|(name,_)|Node{name:(*name).to_string(),size:0,directory:false}).collect())
 }
 fn read(&self,path:&str)->Result<Vec<u8>,vfs::Error>{self.lookup(path)?;Ok(Vec::new())}
 fn create(&self,_:&str,_:bool)->Result<(),vfs::Error>{Err(vfs::Error::Unsupported)}
 fn remove(&self,_:&str)->Result<(),vfs::Error>{Err(vfs::Error::Unsupported)}
 fn write(&self,_:&str,_:usize,_:&[u8],_:bool)->Result<usize,vfs::Error>{Err(vfs::Error::Unsupported)}
}
pub fn mount()->Result<(),vfs::Error>{vfs::mount("/dev",Box::new(DeviceFs))}
pub fn framebuffer_description()->Option<[u8;20]>{
 let f=crate::framebuffer()?;
 let mut bytes=[0u8;20];
 bytes[0..4].copy_from_slice(&f.width.to_le_bytes());
 bytes[4..8].copy_from_slice(&f.height.to_le_bytes());
 bytes[8..12].copy_from_slice(&f.stride.to_le_bytes());
 bytes[12..16].copy_from_slice(&f.format.to_le_bytes());
 bytes[16..20].copy_from_slice(&32u32.to_le_bytes());
 Some(bytes)
}
