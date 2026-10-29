use crate::memory::address_space::AddressSpace;
use crate::thread::ThreadId;
use alloc::{vec::Vec,string::String};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessState {
    Created,
    Ready,
    Running,
    Blocked,
    Exited,
    Faulted,
}
#[derive(Clone)]
pub enum Descriptor {
    File(crate::fs::vfs::File),
    Pipe(crate::ipc::PipeEnd),
    Channel(crate::ipc::ChannelEnd),
    Device(crate::device::Kind),
}

pub struct Process {
    pub pid: ProcessId,
    pub name: &'static str,
    pub state: ProcessState,
    pub address_space: Option<AddressSpace>,
    pub exit_code: Option<i32>,
    pub threads: Vec<ThreadId>,
    pub fault_address: Option<u64>,
    pub fault_vector: Option<u64>,
    pub files: Vec<Option<Descriptor>>,
    pub cwd: String,
    pub parent: Option<ProcessId>,
    pub orphan: bool,
    pub entry: u64,
    pub user_stack: u64,
    pub env: Vec<String>,
    pub mappings: Vec<(u64,u64)>,
    pub sleep_until: Option<u64>,
}
impl Process {
    pub fn new(pid: ProcessId, name: &'static str) -> Self {
        Self {
            pid,
            name,
            state: ProcessState::Created,
            address_space: None,
            exit_code: None,
            threads: Vec::new(),
            fault_address: None,
            fault_vector: None,
            files: alloc::vec![None,None,None],
            cwd: String::from("/"),
            parent: None,
            orphan: false,
            entry: 0,
            user_stack: crate::memory::address_space::USER_STACK_TOP - 8,
            env: alloc::vec![String::from("PATH=/bin"),String::from("HOME=/home"),String::from("SHELL=/bin/nsh")],
            mappings: Vec::new(),
            sleep_until: None,
        }
    }
}

use crate::{arch::x86_64::cpu, terminal::Terminal};
use core::{cell::UnsafeCell,sync::atomic::{AtomicBool,Ordering}};
struct Manager {
    next_pid: u64,
    current: [Option<ProcessId>; 8],
    processes: Vec<Process>,
    output: *mut Terminal,
    boot_info: *const nova_core::NovaBootInfo,
}
impl Manager {
    fn cpu_slot() -> usize { crate::arch::x86_64::percpu::current_logical() }
    fn active_id(&self) -> Option<ProcessId> { self.current[Self::cpu_slot()] }
    fn active(&self) -> Option<&Process> {
        let id=self.active_id()?;
        self.processes.iter().find(|p|p.pid==id)
    }
    fn active_mut(&mut self) -> Option<&mut Process> {
        let id=self.active_id()?;
        self.find_mut(id)
    }
    fn find_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        self.processes.iter_mut().find(|p|p.pid==pid)
    }
}
struct Cell(UnsafeCell<Option<Manager>>,AtomicBool);
// SAFETY: all manager reads/writes use the atomic lock with local IRQs masked.
// CPU-local current PIDs refer into the locked process table. AP user
// scheduling remains disabled until the rest of the SMP audit is complete.
unsafe impl Sync for Cell {}
static MANAGER: Cell = Cell(UnsafeCell::new(None),AtomicBool::new(false));
static OUTPUT_LOCK: AtomicBool = AtomicBool::new(false);
fn with_output<R>(f:impl FnOnce()->R)->R{
    let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
    while OUTPUT_LOCK.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
    let result=f();
    OUTPUT_LOCK.store(false,Ordering::Release);
    if enabled{cpu::enable_interrupts()}
    result
}
fn with_manager<R>(f: impl FnOnce(&mut Manager) -> R) -> R {
    let enabled = cpu::interrupts_enabled();
    cpu::disable_interrupts();
    while MANAGER.1.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop()}
    // SAFETY: the atomic lock excludes all other CPUs and local IRQs cannot
    // reenter while the unique mutable manager reference is borrowed.
    let result = f(unsafe {
        (*MANAGER.0.get())
            .as_mut()
            .expect("process manager not initialized")
    });
    MANAGER.1.store(false,Ordering::Release);
    if enabled {
        cpu::enable_interrupts();
    }
    result
}
pub fn init(info:&nova_core::NovaBootInfo) {
    // SAFETY: one-shot initialization before IRQs and user processes.
    unsafe {
        *MANAGER.0.get() = Some(Manager {
            next_pid: 1,
            current: [None; 8],
            processes: Vec::new(),
            output: core::ptr::null_mut(),
            boot_info: info,
        });
    }
}
pub fn create(name: &'static str, space: AddressSpace, term: &mut Terminal) -> ProcessId {
    with_manager(|m| {
        assert!(m.active_id().is_none());
        let pid = ProcessId(m.next_pid);
        m.next_pid += 1;
        let mut process = Process::new(pid, name);
        process.address_space = Some(space);
        process.state = ProcessState::Ready;
        m.processes.push(process);
        m.current[Manager::cpu_slot()] = Some(pid);
        m.output = term;
        pid
    })
}
pub fn configure_current(entry:u64, user_stack:u64) {
    with_manager(|m| {let p=m.active_mut().expect("process configuration without process");p.entry=entry;p.user_stack=user_stack;});
}
pub fn current_entry_stack()->Option<(u64,u64)> {
    with_manager(|m|m.active().map(|p|(p.entry,p.user_stack)))
}
pub fn create_child(name:&'static str, space:AddressSpace, entry:u64, stack:u64, env:Vec<String>)->Result<ProcessId,&'static str>{
    with_manager(|m|{
        let parent=m.active().ok_or("spawn without parent")?;
        let parent_pid=parent.pid;let cwd=parent.cwd.clone();let files=parent.files.clone();let id=ProcessId(m.next_pid);
        m.next_pid=m.next_pid.checked_add(1).ok_or("PID overflow")?;
        let mut p=Process::new(id,name);p.parent=Some(parent_pid);p.cwd=cwd;
        p.address_space=Some(space);p.entry=entry;p.user_stack=stack;p.env=env;p.files=files;p.state=ProcessState::Ready;
        m.processes.push(p);Ok(id)
    })
}
pub fn current_pid() -> Option<ProcessId> {
    with_manager(|m| m.active_id())
}
pub fn try_current_pid()->Option<ProcessId>{
 let enabled=cpu::interrupts_enabled();cpu::disable_interrupts();
 if MANAGER.1.compare_exchange(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){
  if enabled{cpu::enable_interrupts()}return None
 }
 // SAFETY: the lock excludes writers; the manager may not yet exist during
 // an early panic or exception.
 let pid=unsafe{(*MANAGER.0.get()).as_ref().and_then(|m|{
  let cpu=crate::arch::x86_64::percpu::try_current_logical()?;
  m.current.get(cpu).copied().flatten()
 })};
 MANAGER.1.store(false,Ordering::Release);
 if enabled{cpu::enable_interrupts()}pid
}
pub fn current_cwd() -> Option<String> {
    with_manager(|m| m.active().map(|p| p.cwd.clone()))
}
pub fn current_env()->Option<Vec<String>>{with_manager(|m|m.active().map(|p|p.env.clone()))}
pub fn add_thread(pid: ProcessId, tid: ThreadId) {
    if pid.0 == 0 {
        return;
    }
    with_manager(|m| {
        let p = m.find_mut(pid).expect("thread without process");
        assert_eq!(p.pid, pid);
        p.threads.push(tid);
    });
}
pub fn remove_thread(tid:ThreadId){with_manager(|m|{
    for p in m.processes.iter_mut() {
        p.threads.retain(|&id|id!=tid);
    }
})}
pub fn mark_running(pid: ProcessId) {
    if pid.0 == 0 {
        return;
    }
    with_manager(|m| {
        let p = m.find_mut(pid).expect("run without process");
        assert_eq!(p.pid, pid);
        p.state = ProcessState::Running;
        m.current[Manager::cpu_slot()] = Some(pid);
    });
}
pub fn mark_ready(pid: ProcessId) {
    if pid.0 == 0 {
        return;
    }
    with_manager(|m| {
        let p = m.find_mut(pid).expect("yield without process");
        assert_eq!(p.pid, pid);
        p.state = ProcessState::Ready;
    });
}
pub fn mark_blocked(pid:ProcessId){if pid.0!=0{with_manager(|m|{m.find_mut(pid).expect("block without process").state=ProcessState::Blocked;});}}
pub fn space_root() -> Option<u64> {
    with_manager(|m| {
        m.active()?
            .address_space
            .as_ref()
            .map(|s| s.root.address())
    })
}
pub fn translate_user(addr: u64, write: bool) -> Option<u64> {
    with_manager(|m| {
        m.active()?
            .address_space
            .as_ref()?
            .translate_user(addr, write)
    })
}
pub fn exit(code: i32) {
    if let Some(pid)=current_pid(){crate::graphics::release_if_owner(pid);crate::input::release_if_owner(pid)}
    let (parent, files, adopted)=with_manager(|m| {
        let p = m.active_mut().expect("exit without process");
        p.state = ProcessState::Exited;
        p.exit_code = Some(code);
        let parent=p.parent;let dead=p.pid;let files=core::mem::take(&mut p.files);
        let mut adopted=false;
        if dead.0!=1 {for child in &mut m.processes {if child.parent==Some(dead){child.parent=Some(ProcessId(1));child.orphan=true;adopted=true;}}}
        (parent, files, adopted)
    });
    drop(files);
    if let Some(pid)=parent{crate::scheduler::wake(pid)}
    if adopted{crate::scheduler::wake(ProcessId(1))}
}
pub fn fault(vector: u64, address: u64) {
    if let Some(pid)=current_pid(){crate::graphics::release_if_owner(pid);crate::input::release_if_owner(pid)}
    let (parent, files, adopted)=with_manager(|m| {
        let p = m.active_mut().expect("fault without process");
        p.state = ProcessState::Faulted;
        p.exit_code = Some(-(vector as i32));
        p.fault_address = Some(address);
        p.fault_vector = Some(vector);
        let parent=p.parent;let dead=p.pid;let files=core::mem::take(&mut p.files);
        let mut adopted=false;
        if dead.0!=1 {for child in &mut m.processes {if child.parent==Some(dead){child.parent=Some(ProcessId(1));child.orphan=true;adopted=true;}}}
        (parent, files, adopted)
    });
    drop(files);
    if let Some(pid)=parent{crate::scheduler::wake(pid)}
    if adopted{crate::scheduler::wake(ProcessId(1))}
    crate::nova_log!(
        Warn,
        "USER",
        "PID {} fault vector {} address {:#x}",
        current_pid().unwrap().0,
        vector,
        address
    );
}
pub fn output_byte(byte: u8) {
    with_output(||{
        let ptr = with_manager(|m| m.output);
        // SAFETY: the host pins Terminal until finish() clears its pointer
        // under OUTPUT_LOCK. That lock also excludes simultaneous renderers.
        if !ptr.is_null() {
            unsafe {
                (*ptr).char(byte);
            }
        }
    });
}
pub fn clear_output(){
 with_output(||{
  let ptr=with_manager(|m|m.output);
  // SAFETY: OUTPUT_LOCK excludes rendering and finish() until clear returns.
  if !ptr.is_null(){unsafe{(*ptr).clear()}}
 });
}
pub fn snapshot() -> Option<(ProcessId, ProcessState, &'static str)> {
    with_manager(|m| m.active().map(|p| (p.pid, p.state, p.name)))
}
pub fn list() -> Vec<(ProcessId, ProcessState, &'static str, Option<ProcessId>)> {
    with_manager(|m| m.processes.iter()
        .map(|p| (p.pid, p.state, p.name, p.parent)).collect())
}
pub fn fault_details() -> Option<(u64, u64)> {
    with_manager(|m| {
        let p = m.active()?;
        Some((p.fault_vector?, p.fault_address?))
    })
}
pub fn finish(
    info: &nova_core::NovaBootInfo,
) -> Result<(ProcessId, ProcessState, i32), &'static str> {
    let mut process = with_output(||with_manager(|m| {
        m.output = core::ptr::null_mut();
        let id=m.active_id().expect("no process");
        m.current[Manager::cpu_slot()]=None;
        let index=m.processes.iter().position(|p|p.pid==id).expect("active process missing");
        m.processes.swap_remove(index)
    }));
    let result = (process.pid, process.state, process.exit_code.unwrap_or(-1));
    crate::scheduler::reap(process.pid);
    process.address_space.take().unwrap().destroy(info)?;
    Ok(result)
}

pub fn open_file(file:crate::fs::vfs::File)->Result<u64,()> {
 open_descriptor(Descriptor::File(file))
}
pub fn open_descriptor(descriptor:Descriptor)->Result<u64,()> {
 with_manager(|m|{
  let files=&mut m.active_mut().ok_or(())?.files;
  if let Some((i,slot))=files.iter_mut().enumerate().skip(3).find(|(_,f)|f.is_none()){*slot=Some(descriptor);return Ok(i as u64)}
  if files.len()>=64{return Err(())}let fd=files.len() as u64;files.push(Some(descriptor));Ok(fd)
 })
}
pub fn descriptor(fd:u64)->Option<Descriptor>{with_manager(|m|m.active()?.files.get(fd as usize)?.clone())}
pub fn close_file(fd:u64)->Result<(),()> {
 let old=with_manager(|m|{
  let file=m.active_mut().ok_or(())?.files.get_mut(fd as usize).ok_or(())?;
  if fd<3{return Err(())}file.take().ok_or(())
 })?;
 drop(old);Ok(())
}
pub fn read_file(fd:u64,len:usize)->Result<Vec<u8>,()> {
 with_manager(|m|{
  let Descriptor::File(file)=m.active_mut().ok_or(())?.files.get_mut(fd as usize).and_then(Option::as_mut).ok_or(())? else{return Err(())};
  if file.node.directory||file.flags&3==1{return Err(())}
  if file.offset>=file.data.len(){return Ok(Vec::new())}
  let end=file.offset.saturating_add(len).min(file.data.len());
  let data=file.data[file.offset..end].to_vec();file.offset=end;Ok(data)
 })
}
pub enum FileWriteError {BadFd,Fs(crate::fs::vfs::Error)}
pub fn write_file(fd:u64,data:&[u8])->Result<usize,FileWriteError>{
 let (path,offset)=with_manager(|m|{
  let Descriptor::File(file)=m.active()?.files.get(fd as usize)?.as_ref()? else{return None};
  if file.node.directory||file.flags&3==0{return None}
  Some((file.path.clone(),file.offset))
 }).ok_or(FileWriteError::BadFd)?;
 let written=crate::fs::vfs::write(&path,offset,data,false).map_err(FileWriteError::Fs)?;
 let refreshed=crate::fs::vfs::read(&path).map_err(FileWriteError::Fs)?;
 with_manager(|m|{if let Some(Some(Descriptor::File(file)))=m.active_mut().and_then(|p|p.files.get_mut(fd as usize)){file.offset=offset+written;file.node.size=refreshed.len() as u64;file.data=refreshed;}});
 Ok(written)
}
pub fn seek_file(fd:u64,offset:i64,whence:u64)->Result<u64,()> {
 with_manager(|m|{
  let Descriptor::File(file)=m.active_mut().ok_or(())?.files.get_mut(fd as usize).and_then(Option::as_mut).ok_or(())? else{return Err(())};
  if file.node.directory{return Err(())}
  let base=match whence{0=>0,1=>file.offset as i64,2=>file.data.len() as i64,_=>return Err(())};
  let next=base.checked_add(offset).filter(|x|*x>=0).ok_or(())?;
  file.offset=next as usize;Ok(next as u64)
 })
}
pub fn stat_file(fd:u64)->Result<(bool,u64),()> {
 with_manager(|m|{let Descriptor::File(file)=m.active().ok_or(())?.files.get(fd as usize).and_then(Option::as_ref).ok_or(())? else{return Err(())};Ok((file.node.directory,file.node.size as u64))})
}
pub fn next_dir_entry(fd:u64)->Result<Option<crate::fs::vfs::Node>,()> {
 with_manager(|m|{
  let Descriptor::File(file)=m.active_mut().ok_or(())?.files.get_mut(fd as usize).and_then(Option::as_mut).ok_or(())? else{return Err(())};
  if !file.node.directory{return Err(())}
  while file.dir_offset<file.directory_entries.len(){let e=file.directory_entries[file.dir_offset].clone();file.dir_offset+=1;if e.name!="."&&e.name!=".."{return Ok(Some(e))}}
  Ok(None)
 })
}

#[derive(Clone,Copy,Debug)]
pub enum ChildStatus {Running,Done(ProcessId,i32)}
pub fn child_status(pid:ProcessId)->Option<ChildStatus>{
 with_manager(|m|{
  let parent=m.active()?.pid;
  let child=if pid.0==0 {
   m.processes.iter().filter(|p|p.parent==Some(parent)&&p.orphan).find(|p|matches!(p.state,ProcessState::Exited|ProcessState::Faulted))
    .or_else(||m.processes.iter().find(|p|p.parent==Some(parent)&&p.orphan))?
  }else{m.processes.iter().find(|p|p.pid==pid&&p.parent==Some(parent))?};
  match child.state {ProcessState::Exited|ProcessState::Faulted=>Some(ChildStatus::Done(child.pid,child.exit_code.unwrap_or(-1))),_=>Some(ChildStatus::Running)}
 })
}
pub fn reap_child(pid:ProcessId)->Result<(ProcessId,i32),&'static str>{
 let (mut child,info)=with_manager(|m|{
  let parent=m.active().ok_or("wait without parent")?.pid;
  let index=m.processes.iter().position(|p|(p.pid==pid||pid.0==0&&p.orphan)&&p.parent==Some(parent)&&matches!(p.state,ProcessState::Exited|ProcessState::Faulted)).ok_or("child not exited")?;
  Ok::<_,&'static str>((m.processes.swap_remove(index),m.boot_info))
 })?;
 let code=child.exit_code.unwrap_or(-1);
 crate::scheduler::reap(child.pid);
 // SAFETY: NovaBootInfo lives in reserved loader pages throughout kernel lifetime;
 // the address is installed during one-shot process-manager initialization.
 child.address_space.take().ok_or("child without address space")?.destroy(unsafe{&*info})?;
 if child.orphan{crate::nova_log!(Info,"PROC","orphan PID {} reaped by init",child.pid.0)}
 Ok((child.pid,code))
}
pub fn terminate_child(pid:ProcessId,code:i32)->Result<(),i64>{
 if pid.0<=1{return Err(-1)}
 let (files,adopted)=with_manager(|m|{
  let parent=m.active().ok_or(-3i64)?.pid;
  let index=m.processes.iter().position(|p|p.pid==pid&&p.parent==Some(parent)).ok_or(-3i64)?;
  if matches!(m.processes[index].state,ProcessState::Exited|ProcessState::Faulted){return Err(-3i64)}
  let child=&mut m.processes[index];child.state=ProcessState::Exited;child.exit_code=Some(code);
  let files=core::mem::take(&mut child.files);
  let mut adopted=false;
  for descendant in &mut m.processes {if descendant.parent==Some(pid){descendant.parent=Some(ProcessId(1));descendant.orphan=true;adopted=true}}
  Ok((files,adopted))
 })?;
 crate::graphics::release_if_owner(pid);
 crate::input::release_if_owner(pid);
 drop(files);
 crate::scheduler::kill_process_threads(pid);
 if adopted{crate::scheduler::wake(ProcessId(1))}
 if let Some(parent)=current_pid(){crate::scheduler::wake(parent)}
 Ok(())
}
pub fn change_cwd(path:String){with_manager(|m|m.active_mut().expect("chdir without process").cwd=path)}
pub fn destroy_unstarted(space:AddressSpace)->Result<(),&'static str>{
 let info=with_manager(|m|m.boot_info);
 // SAFETY: the boot handoff resides in reserved loader pages until shutdown.
 space.destroy(unsafe{&*info})
}
const MAP_BASE:u64=crate::memory::address_space::USER_BASE+0x2000_0000;
const MAP_END:u64=MAP_BASE+0x1000_0000;
pub fn map_anonymous(length:u64,prot:u64,flags:u64)->Result<u64,i64>{
 if length==0||length>16*1024*1024||flags!=0||prot&!7!=0||prot&1==0||prot&6==6{return Err(-22)}
 let len=length.checked_add(4095).ok_or(-22i64)?&!4095;
 with_manager(|m|{
  let p=m.active_mut().ok_or(-22i64)?;
  let mut start=MAP_BASE;
  loop {
   let end=start.checked_add(len).filter(|&x|x<=MAP_END).ok_or(-12i64)?;
   if let Some((_,next))=p.mappings.iter().filter(|(a,n)|start<*a+*n&&end>*a).map(|(a,n)|(*a,*a+*n)).min_by_key(|(a,_)|*a){start=next;continue}
   let mut bits=crate::memory::paging::PageFlags::USER.0;
   if prot&2!=0{bits|=crate::memory::paging::PageFlags::WRITABLE.0}
   if prot&4==0&&crate::memory::paging::nx_enabled(){bits|=crate::memory::paging::PageFlags::NO_EXECUTE.0}
   let space=p.address_space.as_ref().ok_or(-22i64)?;
   let mut mapped=0;
   while mapped<len {
    if space.map_user(start+mapped,crate::memory::paging::PageFlags(bits)).is_err(){
     for off in (0..mapped/4096).map(|i|i*4096){let _=space.unmap_user(start+off);}
     return Err(-12)
    }
    mapped+=4096;
   }
   p.mappings.push((start,len));return Ok(start)
  }
 })
}
pub fn unmap_anonymous(start:u64,length:u64)->Result<(),i64>{
 let len=length.checked_add(4095).ok_or(-22i64)?&!4095;
 if length==0||start&4095!=0{return Err(-22)}
 with_manager(|m|{
  let p=m.active_mut().ok_or(-22i64)?;
  let index=p.mappings.iter().position(|&(a,n)|a==start&&n==len).ok_or(-22i64)?;
  let space=p.address_space.as_ref().ok_or(-22i64)?;
  for off in (0..len/4096).map(|i|i*4096){space.unmap_user(start+off).map_err(|_|-5i64)?;}
  p.mappings.swap_remove(index);Ok(())
 })
}
pub fn set_sleep(deadline:u64){with_manager(|m|m.active_mut().expect("sleep without process").sleep_until=Some(deadline))}
pub fn wake_sleepers(now:u64){
 let ready=with_manager(|m|{
  let mut result=Vec::new();
  for p in m.processes.iter_mut(){
   if p.state==ProcessState::Blocked&&p.sleep_until.is_some_and(|deadline|now>=deadline){p.sleep_until=None;result.push(p.pid)}
  }result
 });
 for pid in ready{crate::scheduler::wake(pid)}
}
