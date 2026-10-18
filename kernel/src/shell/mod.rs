use crate::{
    arch::x86_64::{
        cpu,
        cpuid::{self, CpuInfo},
    },
    input::{self, KeyEvent, CTRL},
    fs::vfs,
    interrupts::timer,
    memory::address_space::AddressSpace,
    memory::physical,
    process, scheduler,
    terminal::Terminal,
    userspace,
};
use core::fmt::Write;
use alloc::string::String;
use nova_core::{Framebuffer, NovaBootInfo};

struct Command {
    name: &'static str,
    description: &'static str,
    handler: fn(&mut Shell<'_>, &str),
}
const COMMANDS: &[Command] = &[
    Command {
        name: "run",
        description: "Run userspace ELF from FAT32",
        handler: run_cmd,
    },
    Command {name:"pwd",description:"Print working directory",handler:pwd_cmd},
    Command {name:"ls",description:"List directory",handler:ls_cmd},
    Command {name:"cd",description:"Change directory",handler:cd_cmd},
    Command {name:"cat",description:"Read file",handler:cat_cmd},
    Command {name:"touch",description:"Create an empty file",handler:touch_cmd},
    Command {name:"mkdir",description:"Create directory",handler:mkdir_cmd},
    Command {name:"rm",description:"Remove file or empty directory",handler:rm_cmd},
    Command {name:"write",description:"Write text: write <file> <text>",handler:write_cmd},
    Command {
        name: "ps",
        description: "Show active process",
        handler: ps_cmd,
    },
    Command {
        name: "help",
        description: "List commands",
        handler: help,
    },
    Command {
        name: "clear",
        description: "Clear terminal",
        handler: clear,
    },
    Command {
        name: "echo",
        description: "Print text",
        handler: echo,
    },
    Command {
        name: "version",
        description: "Nova version",
        handler: version,
    },
    Command {
        name: "uname",
        description: "System name",
        handler: uname,
    },
    Command {
        name: "cpu",
        description: "Detected CPU information",
        handler: cpu_cmd,
    },
    Command {
        name: "mem",
        description: "Physical memory statistics",
        handler: mem_cmd,
    },
    Command {
        name: "uptime",
        description: "Kernel uptime",
        handler: uptime_cmd,
    },
];
pub struct Shell<'a> {
    term: Terminal,
    info: &'a NovaBootInfo,
    cpu: CpuInfo,
    line: [u8; 128],
    len: usize,
    cwd: String,
}
impl<'a> Shell<'a> {
    fn prompt(&mut self) {
        let _ = write!(self.term, "nova> ");
    }
    fn event(&mut self, event: KeyEvent) {
        if !event.pressed {
            return;
        }
        if event.modifiers & CTRL != 0 && event.key == b'l' {
            self.term.clear();
            self.prompt();
            return;
        }
        match event.key {
            b'\n' => {
                self.term.char(b'\n');
                self.execute();
                self.len = 0;
                self.prompt();
            }
            8 => {
                if self.len > 0 {
                    self.len -= 1;
                    self.term.backspace();
                }
            }
            c if (32..127).contains(&c) => {
                if self.len < self.line.len() {
                    self.line[self.len] = c;
                    self.len += 1;
                    self.term.char(c);
                }
            }
            _ => {}
        }
    }
    fn execute(&mut self) {
        let mut copy = [0u8; 128];
        copy[..self.len].copy_from_slice(&self.line[..self.len]);
        let line = core::str::from_utf8(&copy[..self.len]).unwrap_or("").trim();
        if line.is_empty() {
            return;
        }
        let (name, args) = match line.split_once(' ') {
            Some((n, a)) => (n, a.trim_start()),
            None => (line, ""),
        };
        if let Some(c) = COMMANDS.iter().find(|c| c.name == name) {
            (c.handler)(self, args);
        } else if name.starts_with("/") || name.starts_with("./") || !name.contains("/") {
            run_cmd(self, name);
        } else {
            let _ = writeln!(self.term, "Unknown command: {} (type help)", name);
        }
    }
}
pub fn run(info: &NovaBootInfo, cpu_info: CpuInfo, fb: Framebuffer) -> ! {
    let mut shell = Shell {
        term: Terminal::new(fb),
        info,
        cpu: cpu_info,
        line: [0; 128],
        len: 0,
        cwd: String::from("/"),
    };
    let _ = writeln!(
        shell.term,
        concat!("\nNova OS ",env!("CARGO_PKG_VERSION"),"\nKernel initialized successfully.\nType help for available commands.\n")
    );
    shell.prompt();
    crate::nova_log!(Info, "CORE", "Kernel initialization complete");
    cpu::enable_interrupts();
    let mut dropped = 0;
    loop {
        while let Some(event) = input::pop() {
            shell.event(event);
        }
        let now = input::dropped();
        if now != dropped {
            crate::nova_log!(Warn, "INPUT", "Key event queue overflow");
            dropped = now;
        }
        cpu::halt();
    }
}
fn help(s: &mut Shell<'_>, _: &str) {
    for c in COMMANDS {
        let _ = writeln!(s.term, "{:<9} {}", c.name, c.description);
    }
}
fn clear(s: &mut Shell<'_>, _: &str) {
    s.term.clear();
}
fn echo(s: &mut Shell<'_>, args: &str) {
    let _ = writeln!(s.term, "{}", args);
}
fn version(s: &mut Shell<'_>, _: &str) {
    let _ = writeln!(s.term, "Nova OS {} | Kernel {} | x86_64",env!("CARGO_PKG_VERSION"),env!("CARGO_PKG_VERSION"));
}
fn uname(s: &mut Shell<'_>, _: &str) {
    let _ = writeln!(s.term, "Nova OS x86_64");
}
fn cpu_cmd(s: &mut Shell<'_>, _: &str) {
    let vendor = core::str::from_utf8(&s.cpu.vendor).unwrap_or("Unavailable");
    let brand = core::str::from_utf8(&s.cpu.brand)
        .unwrap_or("")
        .trim_matches(char::from(0))
        .trim();
    let _ = writeln!(
        s.term,
        "Vendor: {}\nModel: {}\nFamily: {} Model: {} Stepping: {}",
        vendor,
        if brand.is_empty() {
            "Unavailable"
        } else {
            brand
        },
        s.cpu.family,
        s.cpu.model,
        s.cpu.stepping
    );
    let f = s.cpu.features;
    let _ = write!(s.term, "Detected:");
    for (mask, name) in [
        (cpuid::feature::SSE2, " SSE2"),
        (cpuid::feature::SSE42, " SSE4.2"),
        (cpuid::feature::AVX, " AVX"),
        (cpuid::feature::AVX2, " AVX2"),
        (cpuid::feature::NX, " NX"),
        (cpuid::feature::APIC, " APIC"),
        (cpuid::feature::AES, " AES"),
        (cpuid::feature::SMEP, " SMEP"),
        (cpuid::feature::SMAP, " SMAP"),
    ] {
        if f & mask != 0 {
            let _ = write!(s.term, "{}", name);
        }
    }
    let _ = writeln!(s.term, "\nActive Nova CPUs: 1");
}
fn mem_cmd(s: &mut Shell<'_>, _: &str) {
    let m = physical::stats();
    let _ = writeln!(
        s.term,
        "Physical: {} MiB\nUsable: {} MiB\nUsed: {} KiB\nFree: {} MiB\nReserved: {} MiB",
        m.total_bytes / 1048576,
        m.usable_bytes / 1048576,
        m.used_bytes / 1024,
        m.free_bytes / 1048576,
        m.reserved_bytes / 1048576
    );
    let _ = s.info;
}
fn uptime_cmd(s: &mut Shell<'_>, _: &str) {
    let ms = timer::uptime_ms();
    let _ = writeln!(
        s.term,
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3600000,
        (ms / 60000) % 60,
        (ms / 1000) % 60,
        ms % 1000
    );
}
fn run_cmd(s: &mut Shell<'_>, args: &str) {
    if args.is_empty(){let _=writeln!(s.term,"Usage: run <path>");return;}
    let path=if args.starts_with('/') || args.starts_with("./") {vfs::normalize(&s.cwd,args)}
        else {vfs::normalize("/bin",args)};
    let path=match path {Ok(p)=>p,Err(e)=>{let _=writeln!(s.term,"Path: {:?}",e);return;}};
    let image=match vfs::read(&path) {Ok(b)=>b,Err(e)=>{let _=writeln!(s.term,"Program unavailable: {} ({:?})",path,e);return;}};
    crate::nova_log!(Info, "ELF", "loading {} from FAT32", path);
    let space = match AddressSpace::new() {
        Ok(x) => x,
        Err(e) => {
            let _ = writeln!(s.term, "Address space: {}", e);
            return;
        }
    };
    let nx = s.cpu.features & cpuid::feature::NX != 0;
    let entry = match userspace::elf::load(&image, &space, nx) {
        Ok(x) => x,
        Err(e) => {
            let _ = space.destroy(s.info);
            let _ = writeln!(s.term, "ELF: {}", e);
            return;
        }
    };
    let args=alloc::vec![path.clone()];
    let env=alloc::vec![String::from("PATH=/bin"),String::from("HOME=/home"),String::from("SHELL=/bin/nsh")];
    let stack=match userspace::stack::build(&space,&args,&env){Ok(x)=>x,Err(e)=>{let _=space.destroy(s.info);let _=writeln!(s.term,"Stack: {}",e);return;}};
    let pid = process::create("user", space, &mut s.term);
    crate::nova_log!(Info, "PROC", "created PID {}", pid.0);
    process::configure_current(entry, stack);
    scheduler::spawn(pid, userspace::thread_entry);
    crate::nova_log!(
        Info,
        "USER",
        "entering Ring 3 PID {} entry {:#x}",
        pid.0,
        entry
    );
    scheduler::run();
    let fault = process::fault_details();
    match process::finish(s.info) {
        Ok((id, process::ProcessState::Exited, code)) => {
            crate::nova_log!(Info, "PROC", "PID {} exited with code {}", id.0, code);
            let _ = writeln!(s.term, "Process {} exited with code {}", id.0, code);
        }
        Ok((id, process::ProcessState::Faulted, code)) => {
            let (vector, address) = fault.unwrap_or((-code as u64, 0));
            let name = match vector {
                14 => "PAGE FAULT",
                6 => "INVALID OPCODE",
                13 => "GENERAL PROTECTION FAULT",
                _ => "CPU EXCEPTION",
            };
            let _ = writeln!(
                s.term,
                "Process {} faulted.\nException: {}\nAddress: {:#x}\nProcess terminated.",
                id.0, name, address
            );
        }
        Ok((id, state, _)) => {
            let _ = writeln!(s.term, "Process {} unexpected state {:?}", id.0, state);
        }
        Err(e) => {
            let _ = writeln!(s.term, "Process cleanup failed: {}", e);
        }
    }
}
fn ps_cmd(s: &mut Shell<'_>, _: &str) {
    let _ = writeln!(s.term, "PID  STATE    NAME\n0    RUNNING  kernel");
    if let Some((pid, state, name)) = process::snapshot() {
        let _ = writeln!(s.term, "{}    {:?}   {}", pid.0, state, name);
    }
}

fn path(s:&Shell<'_>,arg:&str)->Result<String,vfs::Error>{vfs::normalize(&s.cwd,if arg.is_empty(){"."}else{arg})}
fn pwd_cmd(s:&mut Shell<'_>,_:&str){let _=writeln!(s.term,"{}",s.cwd);}
fn ls_cmd(s:&mut Shell<'_>,arg:&str){
 let result=path(s,arg).and_then(|p|vfs::readdir(&p));
 match result {Ok(entries)=>for e in entries {if e.name=="."||e.name==".."{continue}let _=writeln!(s.term,"{}{}",e.name,if e.directory{"/"}else{""});},Err(e)=>{let _=writeln!(s.term,"ls: {:?}",e);}}
}
fn cd_cmd(s:&mut Shell<'_>,arg:&str){
 let result=path(s,if arg.is_empty(){"/"}else{arg});
 match result.and_then(|p|{let e=vfs::lookup(&p)?;if e.directory{Ok(p)}else{Err(vfs::Error::Fs(crate::fs::fat32::Error::NotDirectory))}}){Ok(p)=>s.cwd=p,Err(e)=>{let _=writeln!(s.term,"cd: {:?}",e);}}
}
fn cat_cmd(s:&mut Shell<'_>,arg:&str){
 if arg.is_empty(){let _=writeln!(s.term,"Usage: cat <path>");return}
 match path(s,arg).and_then(|p|vfs::read(&p)) {Ok(data)=>{for b in data{s.term.char(b)}},Err(e)=>{let _=writeln!(s.term,"cat: {:?}",e);}}
}

fn touch_cmd(s:&mut Shell<'_>,arg:&str){
 let result=path(s,arg).and_then(|p|match vfs::lookup(&p){Ok(node) if !node.directory=>Ok(()),Ok(_)=>Err(vfs::Error::Fs(crate::fs::fat32::Error::IsDirectory)),Err(vfs::Error::Fs(crate::fs::fat32::Error::NotFound))=>vfs::create(&p,false),Err(e)=>Err(e)});
 if let Err(e)=result{let _=writeln!(s.term,"touch: {:?}",e);}
}
fn mkdir_cmd(s:&mut Shell<'_>,arg:&str){
 match path(s,arg).and_then(|p|vfs::create(&p,true)){Ok(())=>{},Err(e)=>{let _=writeln!(s.term,"mkdir: {:?}",e);}}
}
fn rm_cmd(s:&mut Shell<'_>,arg:&str){
 match path(s,arg).and_then(|p|vfs::remove(&p)){Ok(())=>{},Err(e)=>{let _=writeln!(s.term,"rm: {:?}",e);}}
}
fn write_cmd(s:&mut Shell<'_>,arg:&str){
 let Some((name,text))=arg.split_once(' ')else{let _=writeln!(s.term,"Usage: write <file> <text>");return;};
 match path(s,name).and_then(|p|vfs::write(&p,0,text.as_bytes(),true)){Ok(_)=>{},Err(e)=>{let _=writeln!(s.term,"write: {:?}",e);}}
}
