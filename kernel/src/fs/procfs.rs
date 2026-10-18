//! Nova-native virtual system and process information. Values are generated
//! when a file is opened; no Linux procfs format or host filesystem is used.
use alloc::{boxed::Box,format,string::ToString,vec::Vec};
use super::{fat32,vfs::{self,FileSystem,Node}};

pub struct ProcFs;
fn not_found()->vfs::Error {fat32::Error::NotFound.into()}
fn node(name:&str,directory:bool,size:u64)->Node {Node{name:name.to_string(),directory,size}}
fn pid_from(path:&str)->Option<(u64,bool)> {
 let mut parts=path.trim_start_matches('/').split('/');
 let pid=parts.next()?.parse::<u64>().ok()?;
 match (parts.next(),parts.next()) {
  (None,None)=>Some((pid,true)),(Some("status"),None)=>Some((pid,false)),_=>None
 }
}
fn content(path:&str)->Result<Vec<u8>,vfs::Error> {
 match path {
  "/meminfo"=>{let s=crate::memory::physical::stats();Ok(format!(
   "TotalBytes: {}\nUsableBytes: {}\nUsedBytes: {}\nFreeBytes: {}\nReservedBytes: {}\n",
   s.total_bytes,s.usable_bytes,s.used_bytes,s.free_bytes,s.reserved_bytes).into_bytes())},
  "/uptime"=>Ok(format!("UptimeMilliseconds: {}\n",crate::interrupts::timer::uptime_ms()).into_bytes()),
  "/kmsg"=>Ok(crate::logging::snapshot()),
  "/system"=>Ok(format!("OS: Nova OS\nKernelVersion: {}\nArchitecture: x86_64\nActiveProcesses: {}\n",
    env!("CARGO_PKG_VERSION"),crate::process::list().iter().filter(|p|!matches!(p.1,crate::process::ProcessState::Exited|crate::process::ProcessState::Faulted)).count()).into_bytes()),
  "/cpuinfo"=>{let cpu=crate::arch::x86_64::cpuid::detect();
   let vendor=core::str::from_utf8(&cpu.vendor).unwrap_or("Unavailable");
   let brand=core::str::from_utf8(&cpu.brand).unwrap_or("Unavailable").trim_end_matches('\0').trim();
   use crate::arch::x86_64::percpu as local;
   let mut text=format!("Vendor: {}\nBrand: {}\nFamily: {}\nModel: {}\nStepping: {}\nDiscoveredCPUs: {}\nEnteredNovaCode: {}\nSchedulerCPUs: 1\nActiveNovaCPUs: 1\n",vendor,brand,cpu.family,cpu.model,cpu.stepping,local::discovered(),local::entered());
   for logical in 0..local::discovered().min(8) as usize {
    if let (Some(id),Some(state))=(local::apic_id(logical),local::state(logical)) {
     use core::fmt::Write;
     let _=writeln!(text,"CPU{}: APIC {} {:?}",logical,id,state);
    }
   }
   Ok(text.into_bytes())
  },
  _=>{let Some((pid,false))=pid_from(path)else{return Err(not_found())};
   let Some((_,state,name,parent))=crate::process::list().into_iter().find(|p|p.0.0==pid)else{return Err(not_found())};
   Ok(format!("Pid: {}\nParent: {}\nState: {:?}\nName: {}\n",pid,parent.map_or(0,|p|p.0),state,name).into_bytes())
  }
 }
}
impl FileSystem for ProcFs {
 fn lookup(&self,path:&str)->Result<Node,vfs::Error>{
  if path=="/" {return Ok(node("proc",true,0))}
  if let Some((pid,true))=pid_from(path){if crate::process::list().iter().any(|p|p.0.0==pid){return Ok(node(&pid.to_string(),true,0))}return Err(not_found())}
  let data=content(path)?;Ok(node(path.rsplit('/').next().unwrap_or(""),false,data.len() as u64))
 }
 fn readdir(&self,path:&str)->Result<Vec<Node>,vfs::Error>{
  if path=="/"{let mut entries=alloc::vec![node("meminfo",false,0),node("uptime",false,0),node("cpuinfo",false,0),node("kmsg",false,0),node("system",false,0)];
   for(p,_,_,_)in crate::process::list(){entries.push(node(&p.0.to_string(),true,0))}return Ok(entries)}
  if let Some((pid,true))=pid_from(path){if crate::process::list().iter().any(|p|p.0.0==pid){return Ok(alloc::vec![node("status",false,0)])}}
  Err(not_found())
 }
 fn read(&self,path:&str)->Result<Vec<u8>,vfs::Error>{content(path)}
 fn create(&self,_:&str,_:bool)->Result<(),vfs::Error>{Err(vfs::Error::Unsupported)}
 fn remove(&self,_:&str)->Result<(),vfs::Error>{Err(vfs::Error::Unsupported)}
 fn write(&self,_:&str,_:usize,_:&[u8],_:bool)->Result<usize,vfs::Error>{Err(vfs::Error::Unsupported)}
}
pub fn mount()->Result<(),vfs::Error>{vfs::mount("/proc",Box::new(ProcFs))}
