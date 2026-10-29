use alloc::{boxed::Box,string::{String,ToString},vec::Vec};
use super::fat32::{self,Entry,Volume};
use core::{cell::UnsafeCell,sync::atomic::{AtomicBool,Ordering}};
use crate::arch::x86_64::cpu;

#[derive(Clone,Copy,Debug)]pub enum Error {NotMounted,Path,Unsupported,Fs(fat32::Error)}
impl From<fat32::Error> for Error {fn from(e:fat32::Error)->Self{Self::Fs(e)}}
#[derive(Clone,Debug)]pub struct Node {pub name:String,pub size:u64,pub directory:bool}
impl From<Entry> for Node {fn from(e:Entry)->Self{Self{name:e.name,size:e.size as u64,directory:e.directory}}}

pub trait FileSystem: Send + Sync {
 fn lookup(&self,path:&str)->Result<Node,Error>;
 fn readdir(&self,path:&str)->Result<Vec<Node>,Error>;
 fn read(&self,path:&str)->Result<Vec<u8>,Error>;
 fn create(&self,path:&str,directory:bool)->Result<(),Error>;
 fn remove(&self,path:&str)->Result<(),Error>;
 fn write(&self,path:&str,offset:usize,data:&[u8],truncate:bool)->Result<usize,Error>;
}
struct Fat32FileSystem {volume:Volume, lock:AtomicBool}
impl Fat32FileSystem {
 fn transaction<R>(&self,f:impl FnOnce(&Self)->R)->R {
  let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
  while self.lock.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
  let result=f(self);
  self.lock.store(false,Ordering::Release);
  if enabled{cpu::enable_interrupts()}
  result
 }
 fn resolve(&self,path:&str)->Result<Entry,Error>{
  let mut current=Entry{name:"/".to_string(),cluster:self.volume.root,size:0,directory:true};
  for part in path.split('/').filter(|x|!x.is_empty()){
   if !current.directory{return Err(fat32::Error::NotDirectory.into())}
   current=self.volume.entries(current.cluster)?.into_iter().find(|e|e.name.eq_ignore_ascii_case(part)).ok_or(fat32::Error::NotFound)?;
  }Ok(current)
 }
 fn parent(&self,path:&str)->Result<(u32,String),Error>{
  if path=="/"{return Err(Error::Path)}
  let (parent,name)=path.rsplit_once('/').ok_or(Error::Path)?;
  let p=self.resolve(if parent.is_empty(){"/"}else{parent})?;
  if !p.directory{return Err(fat32::Error::NotDirectory.into())}Ok((p.cluster,name.to_string()))
 }
}
impl FileSystem for Fat32FileSystem {
 fn lookup(&self,path:&str)->Result<Node,Error>{self.transaction(|fs|Ok(fs.resolve(path)?.into()))}
 fn readdir(&self,path:&str)->Result<Vec<Node>,Error>{self.transaction(|fs|{let e=fs.resolve(path)?;if !e.directory{return Err(fat32::Error::NotDirectory.into())}Ok(fs.volume.entries(e.cluster)?.into_iter().map(Into::into).collect())})}
 fn read(&self,path:&str)->Result<Vec<u8>,Error>{self.transaction(|fs|{let e=fs.resolve(path)?;fs.volume.read_file(&e).map_err(Into::into)})}
 fn create(&self,path:&str,directory:bool)->Result<(),Error>{self.transaction(|fs|{let(p,n)=fs.parent(path)?;fs.volume.create(p,&n,directory).map_err(Into::into)})}
 fn remove(&self,path:&str)->Result<(),Error>{self.transaction(|fs|{let(p,n)=fs.parent(path)?;fs.volume.remove(p,&n).map_err(Into::into)})}
 fn write(&self,path:&str,offset:usize,data:&[u8],truncate:bool)->Result<usize,Error>{self.transaction(|fs|{let(p,n)=fs.parent(path)?;fs.volume.write_file(p,&n,offset,data,truncate).map_err(Into::into)})}
}
struct Mounted {point:String, filesystem:Box<dyn FileSystem>}
struct Mount(UnsafeCell<Option<Vec<Mounted>>>);
// SAFETY: mounts are installed on the BSP before AP startup, then frozen.
// The FileSystem trait requires Send + Sync, and boxed filesystem objects
// remain pinned for the kernel lifetime after mount-table publication.
unsafe impl Sync for Mount {}
static ROOT:Mount=Mount(UnsafeCell::new(None));
static FROZEN:AtomicBool=AtomicBool::new(false);
pub fn freeze_mounts(){FROZEN.store(true,Ordering::Release)}
pub fn mount_root()->Result<(),Error>{
 let v=Volume::mount()?;
 crate::nova_log!(Info,"FAT32","{} sectors, {} sectors/cluster",v.sectors,v.sectors_per_cluster);
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 // SAFETY: one-shot mount before shell/user access. Mounted filesystems stay allocated forever.
 unsafe{*ROOT.0.get()=Some(alloc::vec![Mounted {point:String::from("/"),filesystem:Box::new(Fat32FileSystem{volume:v,lock:AtomicBool::new(false)})}])}
 if enabled{cpu::enable_interrupts()};Ok(())
}
pub fn mount(point:&str,filesystem:Box<dyn FileSystem>)->Result<(),Error>{
 if FROZEN.load(Ordering::Acquire){return Err(Error::Unsupported)}
 let path=normalize("/",point)?;
 if path=="/"{return Err(Error::Path)}
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 // SAFETY: mounts are changed only during bootstrap on one CPU; no borrowed
 // reference exists across this call. Heap-owned filesystem objects stay pinned.
 let result=unsafe{(*ROOT.0.get()).as_mut().ok_or(Error::NotMounted).and_then(|mounts|{
  if mounts.iter().any(|m|m.point==path){return Err(Error::Path)}
  mounts.push(Mounted{point:path,filesystem});Ok(())
 })};
 if enabled{cpu::enable_interrupts()}result
}
fn filesystem(path:&str)->Result<(&'static dyn FileSystem,&str),Error>{
 // SAFETY: the mount vector is frozen before AP startup. A Box may move
 // within the vector but its heap allocation and vtable remain stable.
 let mounts=unsafe{(*ROOT.0.get()).as_ref().ok_or(Error::NotMounted)?};
 let selected=mounts.iter().filter(|m|m.point=="/"||path==m.point||path.starts_with(&alloc::format!("{}/",m.point))).max_by_key(|m|m.point.len()).ok_or(Error::NotMounted)?;
 let inner=if selected.point=="/"{path}else if path==selected.point{"/"}else{&path[selected.point.len()..]};
 Ok((selected.filesystem.as_ref(),inner))
}
pub fn normalize(cwd:&str,path:&str)->Result<String,Error>{
 if path.is_empty(){return Err(Error::Path)}
 let mut parts:Vec<&str>=Vec::new();
 if !path.starts_with('/') {for p in cwd.split('/').filter(|x|!x.is_empty()){parts.push(p)}}
 for p in path.split('/'){match p{""|"."=>{},".."=>{parts.pop();},x=>parts.push(x)}}
 let mut result=String::from("/");result.push_str(&parts.join("/"));Ok(result)
}
pub fn lookup(path:&str)->Result<Node,Error>{let path=normalize("/",path)?;let(fs,inner)=filesystem(&path)?;fs.lookup(inner)}
pub fn readdir(path:&str)->Result<Vec<Node>,Error>{
 let path=normalize("/",path)?;let(fs,inner)=filesystem(&path)?;let mut entries=fs.readdir(inner)?;
 if path=="/"{
  // SAFETY: mount table is immutable after bootstrap.
  if let Some(mounts)=unsafe{(*ROOT.0.get()).as_ref()} {for mount in mounts.iter().filter(|m|m.point!="/"){
   let name=&mount.point[1..];if !name.contains('/')&&!entries.iter().any(|n|n.name.eq_ignore_ascii_case(name)){
    entries.push(Node{name:name.to_string(),size:0,directory:true});
   }
  }}
 }Ok(entries)
}
pub fn read(path:&str)->Result<Vec<u8>,Error>{let path=normalize("/",path)?;let(fs,inner)=filesystem(&path)?;fs.read(inner)}
#[derive(Clone)]
pub struct File {pub path:String,pub flags:u64,pub node:Node,pub offset:usize,pub data:Vec<u8>,pub directory_entries:Vec<Node>,pub dir_offset:usize}
pub fn open(path:&str)->Result<File,Error>{
 let path=normalize("/",path)?;let(fs,inner)=filesystem(&path)?;let node=fs.lookup(inner)?;
 let data=if node.directory{Vec::new()}else{fs.read(inner)?};
 let directory_entries=if node.directory{fs.readdir(inner)?}else{Vec::new()};
 Ok(File{path,flags:0,node,offset:0,data,directory_entries,dir_offset:0})
}
pub fn create(path:&str,directory:bool)->Result<(),Error>{let path=normalize("/",path)?;let(fs,inner)=filesystem(&path)?;fs.create(inner,directory)}
pub fn remove(path:&str)->Result<(),Error>{let path=normalize("/",path)?;let(fs,inner)=filesystem(&path)?;fs.remove(inner)}
pub fn write(path:&str,offset:usize,data:&[u8],truncate:bool)->Result<usize,Error>{let path=normalize("/",path)?;let(fs,inner)=filesystem(&path)?;fs.write(inner,offset,data,truncate)}
