use crate::{process,scheduler,fs::{vfs,fat32},memory::address_space::{USER_BASE,USER_END}};
use alloc::{vec::Vec,string::String};

use nova_core::abi;
const SYS_WRITE:u64=abi::WRITE;const SYS_EXIT:u64=abi::EXIT;const SYS_GETPID:u64=abi::GETPID;const SYS_YIELD:u64=abi::YIELD;
const SYS_SPAWN:u64=abi::SPAWN;const SYS_WAIT:u64=abi::WAIT;const SYS_GETCWD:u64=abi::GETCWD;const SYS_CHDIR:u64=abi::CHDIR;
const SYS_CLOCK_GET:u64=abi::CLOCK_GET;const SYS_SLEEP:u64=abi::SLEEP;
const SYS_MAP:u64=abi::MAP;const SYS_UNMAP:u64=abi::UNMAP;
const SYS_OPEN:u64=abi::OPEN;const SYS_CLOSE:u64=abi::CLOSE;const SYS_READ:u64=abi::READ;const SYS_SEEK:u64=abi::SEEK;const SYS_STAT:u64=abi::STAT;const SYS_READDIR:u64=abi::READDIR;
const SYS_PIPE:u64=abi::PIPE;const SYS_CHANNEL_CREATE:u64=abi::CHANNEL_CREATE;const SYS_CHANNEL_SEND:u64=abi::CHANNEL_SEND;const SYS_CHANNEL_RECEIVE:u64=abi::CHANNEL_RECEIVE;
const EBADF:i64=-9;const EFAULT:i64=-14;const EINVAL:i64=-22;const ENOENT:i64=-2;const ENOTSUP:i64=-95;
fn fs_error(error:vfs::Error)->i64 {match error {
 vfs::Error::Path=>EINVAL,vfs::Error::NotMounted=>-5,vfs::Error::Unsupported=>ENOTSUP,
 vfs::Error::Fs(e)=>match e {
  fat32::Error::NotFound=>ENOENT,fat32::Error::NotDirectory=>-20,fat32::Error::IsDirectory=>-21,
  fat32::Error::TooLarge=>-27,fat32::Error::Exists=>-17,fat32::Error::NoSpace=>-28,
  fat32::Error::UnsupportedName=>ENOTSUP,fat32::Error::NotEmpty=>-39,
  fat32::Error::Io|fat32::Error::InvalidVolume|fat32::Error::Corrupt=>-5,
 }}
}

#[no_mangle]
extern "C" fn nova_syscall_dispatch(number:u64,a:u64,b:u64,c:u64)->i64 {
 match number {
  SYS_WRITE=>write(a,b,c),
  SYS_EXIT=>{process::exit(a as i32);scheduler::kill_process_threads(process::current_pid().unwrap());scheduler::exit_current()},
  SYS_GETPID=>process::current_pid().map_or(-1,|p|p.0 as i64),
  SYS_YIELD=>{scheduler::yield_current();0},
  SYS_OPEN=>open(a,b),
  SYS_CLOSE=>process::close_file(a).map_or(EBADF,|_|0),
  SYS_READ=>read(a,b,c),
  SYS_SEEK=>process::seek_file(a,b as i64,c).map_or(EINVAL,|x|x as i64),
  SYS_STAT=>stat(a,b),
  SYS_READDIR=>readdir(a,b,c),
  SYS_SPAWN=>spawn(a,b,c),
  SYS_WAIT=>wait(a,b),
  SYS_GETCWD=>getcwd(a,b),
  SYS_CHDIR=>chdir(a),
  SYS_MAP=>process::map_anonymous(a,b,c).map_or_else(|e|e,|v|v as i64),
  SYS_UNMAP=>process::unmap_anonymous(a,b).map_or_else(|e|e,|_|0),
  SYS_CLOCK_GET=>crate::interrupts::timer::uptime_ms() as i64,
  SYS_SLEEP=>sleep(a),
  SYS_PIPE=>pipe(a),
  SYS_CHANNEL_CREATE=>channel_create(a),
  SYS_CHANNEL_SEND=>channel_send(a,b,c),
  SYS_CHANNEL_RECEIVE=>channel_receive(a,b,c),
  abi::THREAD_CREATE=>thread_create(a,b),
  abi::THREAD_EXIT=>thread_exit(),
  abi::THREAD_JOIN=>scheduler::join(crate::thread::ThreadId(a)).map_or_else(|e|e,|_|0),
  abi::FB_INFO=>fb_info(a),
  abi::FB_ACQUIRE=>process::current_pid().ok_or(-1i64).and_then(crate::graphics::acquire).map_or_else(|e|e,|_|0),
  abi::FB_RELEASE=>process::current_pid().ok_or(-1i64).and_then(crate::graphics::release).map_or_else(|e|e,|_|0),
  abi::FB_FILL=>fb_fill(a),
  abi::FB_BLIT=>fb_blit(a),
  abi::TTY_CLEAR=>{process::clear_output();0},
  abi::KILL=>process::terminate_child(process::ProcessId(a),b as i32).map_or_else(|e|e,|_|0),
  abi::WAIT_TRY=>wait_try(a,b),
  abi::INPUT_ACQUIRE=>process::current_pid().ok_or(-1i64).and_then(crate::input::acquire).map_or_else(|e|e,|_|0),
  abi::INPUT_RELEASE=>process::current_pid().ok_or(-1i64).and_then(crate::input::release).map_or_else(|e|e,|_|0),
  _=>-38,
 }
}
fn range(addr:u64,len:u64,write:bool)->Result<(),i64>{
 if len==0{return Ok(())}
 let end=addr.checked_add(len).filter(|&x|x<=USER_END).ok_or(EFAULT)?;
 if addr<USER_BASE{return Err(EFAULT)}
 let mut page=addr&!4095;
 while page<end{if process::translate_user(page,write).is_none(){return Err(EFAULT)}page=page.checked_add(4096).ok_or(EFAULT)?;}
 Ok(())
}
fn copy_in(addr:u64,len:usize)->Result<Vec<u8>,i64>{
 range(addr,len as u64,false)?;let mut data=Vec::with_capacity(len);
 for i in 0..len {let phys=process::translate_user(addr+i as u64,false).ok_or(EFAULT)?;
  // SAFETY: all pages were validated as user readable and are identity mapped on this CPU.
  data.push(unsafe{(phys as *const u8).read()});}
 Ok(data)
}
fn copy_out(addr:u64,data:&[u8])->Result<(),i64>{
 range(addr,data.len() as u64,true)?;
 for (i,byte) in data.iter().enumerate(){let phys=process::translate_user(addr+i as u64,true).ok_or(EFAULT)?;
  // SAFETY: all pages were validated as user writable and are identity mapped on this CPU.
  unsafe{(phys as *mut u8).write(*byte)};}
 Ok(())
}
fn path(addr:u64)->Result<String,i64>{
 let mut bytes=Vec::new();for i in 0..256u64{let phys=process::translate_user(addr.checked_add(i).ok_or(EFAULT)?,false).ok_or(EFAULT)?;
  // SAFETY: the byte address was checked through the active user page tables.
  let byte=unsafe{(phys as *const u8).read()};if byte==0{return String::from_utf8(bytes).map_err(|_|EINVAL)}bytes.push(byte);
 }Err(EINVAL)
}
fn open(addr:u64,flags:u64)->i64{
 if flags&!(0x300|3)!=0||flags&3==3{return EINVAL}
 let path=match path(addr).and_then(|p|vfs::normalize(&process::current_cwd().unwrap_or_else(||String::from("/")),&p).map_err(fs_error)){Ok(x)=>x,Err(e)=>return e};
 if flags&0x100!=0 {match vfs::lookup(&path){Ok(_)=>{},Err(vfs::Error::Fs(fat32::Error::NotFound))=>if let Err(e)=vfs::create(&path,false){return fs_error(e)},Err(e)=>return fs_error(e)}}
 if flags&0x200!=0 {if flags&3==0{return EINVAL}if let Err(e)=vfs::write(&path,0,&[],true){return fs_error(e)}}
 if let Some(kind)=crate::device::identify(&path){
  if flags&0x300!=0{return EINVAL}
  if let Err(e)=vfs::lookup(&path){return fs_error(e)}
  return process::open_descriptor(process::Descriptor::Device(kind)).map_or(-24,|fd|fd as i64)
 }
 let mut file=match vfs::open(&path){Ok(f)=>f,Err(e)=>return fs_error(e)};
 file.flags=flags;
 process::open_file(file).map_or(-24,|fd|fd as i64)
}
fn read(fd:u64,addr:u64,count:u64)->i64{
 if count>65536{return EINVAL}if let Err(e)=range(addr,count,true){return e}
 if fd==0 {return read_tty(addr,count)}
 if let Some(process::Descriptor::Device(kind))=process::descriptor(fd){
  return match kind {
   crate::device::Kind::Null=>0,
   crate::device::Kind::Zero=>copy_out(addr,&alloc::vec![0u8;count as usize]).map_or_else(|e|e,|_|count as i64),
   crate::device::Kind::Console=>read_tty(addr,count),
   crate::device::Kind::Framebuffer=>match crate::device::framebuffer_description(){Some(bytes)=>{
    let n=(count as usize).min(bytes.len());copy_out(addr,&bytes[..n]).map_or_else(|e|e,|_|n as i64)},None=>-19},
   crate::device::Kind::Keyboard=>read_keyboard(addr,count),
  }
 }
 if let Some(process::Descriptor::Pipe(end))=process::descriptor(fd) {
  if !end.can_read(){return EBADF}
  if count==0{return 0}
  let Some(pid)=process::current_pid()else{return EBADF};
  loop {match end.read(count as usize,pid) {
   crate::ipc::PipeRead::Data(bytes)=>return copy_out(addr,&bytes).map_or_else(|e|e,|_|bytes.len() as i64),
   crate::ipc::PipeRead::Eof=>return 0,
   crate::ipc::PipeRead::Block=>scheduler::block_current(),
  }}
 }
 let bytes=match process::read_file(fd,count as usize){Ok(x)=>x,Err(_)=>return EBADF};
 if let Err(e)=copy_out(addr,&bytes){return e}bytes.len() as i64
}
fn read_tty(addr:u64,count:u64)->i64{
  if count==0{return 0}
  loop {
   let Some(pid)=process::current_pid()else{return EBADF};
   match crate::tty::take_or_wait(count as usize,pid){
    Ok(Some(bytes))=>return copy_out(addr,&bytes).map_or_else(|e|e,|_|bytes.len() as i64),
    Ok(None)=>{},
    Err(())=>return -16,
   }
   scheduler::block_current();
  }
}
fn read_keyboard(addr:u64,count:u64)->i64{
 if count<16{return EINVAL}
 let Some(pid)=process::current_pid()else{return EBADF};
 loop {
  if !crate::input::owned_by(pid){return -1}
  match crate::input::pop_raw_or_wait(pid){
  Ok(Some(event))=>{
   let mut data=[0u8;16];data[0]=if event.pressed{1}else{2};data[1]=event.modifiers;
   data[2..4].copy_from_slice(&event.code.to_le_bytes());data[4]=event.key;
   data[8..16].copy_from_slice(&event.timestamp_ms.to_le_bytes());
   return copy_out(addr,&data).map_or_else(|e|e,|_|16)
  },
  Ok(None)=>{},
  Err(())=>return -16,
  }
  scheduler::block_current();
 }
}
fn write(fd:u64,addr:u64,len:u64)->i64{
 if len>65536{return EINVAL}
 let bytes=match copy_in(addr,len as usize){Ok(x)=>x,Err(e)=>return e};
 if fd==1||fd==2 {for byte in bytes{process::output_byte(byte)}len as i64}
 else if let Some(process::Descriptor::Device(kind))=process::descriptor(fd){match kind {
  crate::device::Kind::Null|crate::device::Kind::Zero=>len as i64,
  crate::device::Kind::Console=>{for byte in bytes{process::output_byte(byte)}len as i64},
  crate::device::Kind::Framebuffer|crate::device::Kind::Keyboard=>ENOTSUP,
 }}
 else if let Some(process::Descriptor::Pipe(end))=process::descriptor(fd) {
  if !end.can_write(){return EBADF}
  let Some(pid)=process::current_pid()else{return EBADF};
  if bytes.is_empty(){return 0}
  loop {match end.write(&bytes,pid) {
   crate::ipc::PipeWrite::Done(n)=>return n as i64,
   crate::ipc::PipeWrite::Closed=>return abi::EPIPE,
   crate::ipc::PipeWrite::Block=>scheduler::block_current(),
  }}
 }
 else if fd>=3 {process::write_file(fd,&bytes).map_or_else(|e|match e {process::FileWriteError::BadFd=>EBADF,process::FileWriteError::Fs(e)=>fs_error(e)},|n|n as i64)}else{EBADF}
}
fn pipe(addr:u64)->i64 {
 if let Err(e)=range(addr,16,true){return e}
 let (read,write)=crate::ipc::pipe();
 let first=match process::open_descriptor(process::Descriptor::Pipe(read)){Ok(fd)=>fd,Err(_)=>return -24};
 let second=match process::open_descriptor(process::Descriptor::Pipe(write)){Ok(fd)=>fd,Err(_)=>{let _=process::close_file(first);return -24}};
 let mut data=[0u8;16];data[..8].copy_from_slice(&first.to_le_bytes());data[8..].copy_from_slice(&second.to_le_bytes());
 if let Err(e)=copy_out(addr,&data){let _=process::close_file(first);let _=process::close_file(second);return e}0
}
fn channel_create(flags:u64)->i64 {
 if flags>1{return EINVAL}
 process::open_descriptor(process::Descriptor::Channel(crate::ipc::channel(flags==1))).map_or(-24,|fd|fd as i64)
}
fn channel_send(fd:u64,addr:u64,len:u64)->i64 {
 if len>4096{return EINVAL}
 let bytes=match copy_in(addr,len as usize){Ok(x)=>x,Err(e)=>return e};
 let Some(process::Descriptor::Channel(channel))=process::descriptor(fd)else{return EBADF};
 let Some(pid)=process::current_pid()else{return EBADF};
 loop {match channel.send(&bytes,pid){
  Ok(crate::ipc::MessageWrite::Done)=>return len as i64,
  Ok(crate::ipc::MessageWrite::Again)=>return abi::EAGAIN,
  Ok(crate::ipc::MessageWrite::Block)=>scheduler::block_current(),
  Err(e)=>return e,
 }}
}
fn channel_receive(fd:u64,addr:u64,capacity:u64)->i64 {
 if capacity>4096{return EINVAL}if let Err(e)=range(addr,capacity,true){return e}
 let Some(process::Descriptor::Channel(channel))=process::descriptor(fd)else{return EBADF};
 let Some(pid)=process::current_pid()else{return EBADF};
 loop {match channel.receive(capacity as usize,pid){
  crate::ipc::MessageRead::Data(bytes)=>return copy_out(addr,&bytes).map_or_else(|e|e,|_|bytes.len() as i64),
  crate::ipc::MessageRead::TooSmall=>return abi::EMSGSIZE,
  crate::ipc::MessageRead::Again=>return abi::EAGAIN,
  crate::ipc::MessageRead::Block=>scheduler::block_current(),
 }}
}
fn thread_create(entry:u64,stack_top:u64)->i64 {
 if entry<USER_BASE||entry>=USER_END||process::translate_user(entry,false).is_none(){return EFAULT}
 let stack=(stack_top&!15).checked_sub(8).unwrap_or(0);
 if stack<USER_BASE||stack>=USER_END||process::translate_user(stack,true).is_none(){return EFAULT}
 let Some(pid)=process::current_pid()else{return EINVAL};
 scheduler::spawn_user(pid,entry,stack).0 as i64
}
fn thread_exit()->! {
 let pid=process::current_pid().expect("thread exit without process");
 if !scheduler::other_live_threads(pid){process::exit(0);scheduler::exit_current()}
 scheduler::exit_thread()
}
fn word(bytes:&[u8],offset:usize)->u32{u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap())}
fn fb_info(addr:u64)->i64{
 let Some(data)=crate::graphics::info()else{return -19};
 copy_out(addr,&data).map_or_else(|e|e,|_|0)
}
fn fb_fill(addr:u64)->i64{
 let data=match copy_in(addr,20){Ok(x)=>x,Err(e)=>return e};
 let rect=crate::graphics::Rect{x:word(&data,0),y:word(&data,4),width:word(&data,8),height:word(&data,12)};
 let Some(pid)=process::current_pid()else{return -1};
 crate::graphics::fill(pid,rect,word(&data,16)).map_or_else(|e|e,|_|0)
}
fn fb_blit(addr:u64)->i64{
 let data=match copy_in(addr,32){Ok(x)=>x,Err(e)=>return e};
 let rect=crate::graphics::Rect{x:word(&data,0),y:word(&data,4),width:word(&data,8),height:word(&data,12)};
 let stride=word(&data,16);
 let pointer=u64::from_le_bytes(data[24..32].try_into().unwrap());
 if rect.height==0||rect.width==0||stride<rect.width{return EINVAL}
 let total=(rect.height as u64-1).checked_mul(stride as u64)
  .and_then(|v|v.checked_add(rect.width as u64)).and_then(|v|v.checked_mul(4)).filter(|&v|v<=16*1024*1024).ok_or(EINVAL);
 let total=match total{Ok(v)=>v,Err(e)=>return e};
 if let Err(e)=range(pointer,total,false){return e}
 let Some(pid)=process::current_pid()else{return -1};
 crate::graphics::blit(pid,rect,stride,|offset|{
  let mut bytes=[0u8;4];for (i,byte) in bytes.iter_mut().enumerate(){
   let phys=process::translate_user(pointer+offset+i as u64,false).ok_or(EFAULT)?;
   // SAFETY: full user buffer was validated, and physical RAM is identity
   // mapped on the BSP. Byte reads cannot cross a physical page boundary.
   *byte=unsafe{(phys as *const u8).read()};
  }Ok(u32::from_le_bytes(bytes))
 }).map_or_else(|e|e,|_|0)
}
fn stat(fd:u64,addr:u64)->i64{
 if let Some(process::Descriptor::Device(kind))=process::descriptor(fd){
  let mut data=[0u8;16];data[..4].copy_from_slice(&3u32.to_le_bytes());
  let size=if kind==crate::device::Kind::Framebuffer{20u64}else{0};
  data[8..].copy_from_slice(&size.to_le_bytes());
  return copy_out(addr,&data).map_or_else(|e|e,|_|0)
 }
 let (directory,size)=match process::stat_file(fd){Ok(x)=>x,Err(_)=>return EBADF};
 let mut data=[0u8;16];data[..4].copy_from_slice(&(if directory{2u32}else{1u32}).to_le_bytes());data[8..].copy_from_slice(&size.to_le_bytes());
 copy_out(addr,&data).map_or_else(|e|e,|_|0)
}
fn readdir(fd:u64,addr:u64,len:u64)->i64{
 if len<272{return EINVAL}if let Err(e)=range(addr,272,true){return e}
 let entry=match process::next_dir_entry(fd){Ok(Some(e))=>e,Ok(None)=>return 0,Err(_)=>return EBADF};
 let name=entry.name.as_bytes();if name.len()>255{return EINVAL}
 let mut data=[0u8;272];data[..4].copy_from_slice(&(if entry.directory{2u32}else{1u32}).to_le_bytes());
 data[4..8].copy_from_slice(&(name.len() as u32).to_le_bytes());data[8..8+name.len()].copy_from_slice(name);
 copy_out(addr,&data).map_or_else(|e|e,|_|1)
}

fn pointer_list(addr:u64, fallback:Option<String>)->Result<Vec<String>,i64>{
 if addr==0{return Ok(fallback.into_iter().collect())}
 let mut strings=Vec::new();
 for i in 0..32u64 {
  let at=addr.checked_add(i*8).ok_or(EFAULT)?;
  let value=u64::from_le_bytes(copy_in(at,8)?.try_into().map_err(|_|EFAULT)?);
  if value==0{return Ok(strings)}
  strings.push(path(value)?);
 }
 Err(EINVAL)
}
fn spawn(path_ptr:u64,argv_ptr:u64,env_ptr:u64)->i64{
 let raw=match path(path_ptr){Ok(x)=>x,Err(e)=>return e};
 let cwd=process::current_cwd().unwrap_or_else(||String::from("/"));
 let target=match vfs::normalize(&cwd,&raw){Ok(x)=>x,Err(e)=>return fs_error(e)};
 let mut args=match pointer_list(argv_ptr,Some(raw.clone())){Ok(x)=>x,Err(e)=>return e};
 if args.is_empty(){args.push(raw)}
 let env=if env_ptr==0{process::current_env().unwrap_or_default()}else{match pointer_list(env_ptr,None){Ok(x)=>x,Err(e)=>return e}};
 let image=match vfs::read(&target){Ok(x)=>x,Err(e)=>return fs_error(e)};
 let space=match crate::memory::address_space::AddressSpace::new(){Ok(x)=>x,Err(_)=>return abi::ENOMEM};
 let nx=crate::arch::x86_64::cpuid::detect().features&crate::arch::x86_64::cpuid::feature::NX!=0;
 let entry=match crate::userspace::elf::load(&image,&space,nx){Ok(x)=>x,Err(_)=>{let _=process::destroy_unstarted(space);return -8}};
 let stack=match crate::userspace::stack::build(&space,&args,&env){Ok(x)=>x,Err(_)=>{let _=process::destroy_unstarted(space);return EINVAL}};
 let pid=match process::create_child("user",space,entry,stack,env){Ok(x)=>x,Err(_)=>return abi::ENOMEM};
 scheduler::spawn(pid,crate::userspace::thread_entry);
 crate::nova_log!(Info,"PROC","spawned PID {} from {}",pid.0,target);
 pid.0 as i64
}
fn wait(raw_pid:u64,status_addr:u64)->i64{
 if status_addr!=0 {if let Err(e)=range(status_addr,4,true){return e}}
 let pid=process::ProcessId(raw_pid);
 loop {match process::child_status(pid){None=>if pid.0==0 {scheduler::block_current()}else{return -10},Some(process::ChildStatus::Running)=>scheduler::block_current(),Some(process::ChildStatus::Done(_,code))=>{
  if status_addr!=0 {if let Err(e)=copy_out(status_addr,&code.to_le_bytes()){return e}}
  return process::reap_child(pid).map_or(-5,|(actual,_)|actual.0 as i64);
 }}}
}
fn wait_try(raw_pid:u64,status_addr:u64)->i64{
 if status_addr!=0 {if let Err(e)=range(status_addr,4,true){return e}}
 let pid=process::ProcessId(raw_pid);
 match process::child_status(pid){
  None=>-10,Some(process::ChildStatus::Running)=>abi::EAGAIN,
  Some(process::ChildStatus::Done(_,code))=>{
   if status_addr!=0{if let Err(e)=copy_out(status_addr,&code.to_le_bytes()){return e}}
   process::reap_child(pid).map_or(-5,|(actual,_)|actual.0 as i64)
  }
 }
}
fn getcwd(addr:u64,len:u64)->i64{
 let cwd=match process::current_cwd(){Some(x)=>x,None=>return EINVAL};
 if len<cwd.len() as u64+1{return EINVAL}
 let mut bytes=cwd.into_bytes();let n=bytes.len();bytes.push(0);
 copy_out(addr,&bytes).map_or_else(|e|e,|_|n as i64)
}
fn chdir(addr:u64)->i64{
 let raw=match path(addr){Ok(x)=>x,Err(e)=>return e};
 let cwd=process::current_cwd().unwrap_or_else(||String::from("/"));
 let dest=match vfs::normalize(&cwd,&raw){Ok(x)=>x,Err(e)=>return fs_error(e)};
 match vfs::lookup(&dest){Ok(node) if node.directory=>{process::change_cwd(dest);0},Ok(_)=>-20,Err(e)=>fs_error(e)}
}

fn sleep(ms:u64)->i64{
 if ms>86_400_000{return EINVAL}if ms==0{scheduler::yield_current();return 0}
 let now=crate::interrupts::timer::uptime_ms();let Some(deadline)=now.checked_add(ms)else{return EINVAL};
 process::set_sleep(deadline);scheduler::block_current();0
}
