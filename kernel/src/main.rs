#![no_std]
#![no_main]

use core::{
    fmt::{self, Write},
    panic::PanicInfo,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
mod arch;
mod block;
mod drivers;
mod device;
mod fs;
mod graphics;
mod input;
mod ipc;
mod interrupts;
mod logging;
mod memory;
mod shell;
extern crate alloc;
mod platform;
mod process;
mod scheduler;
mod syscall;
mod thread;
mod userspace;
mod terminal;
mod tty;
use arch::x86_64::cpu;
use nova_core::{NovaBootInfo, BOOT_MAGIC, BOOT_VERSION};

struct Serial;
impl Serial {
    fn init() {
        // SAFETY: COM1 is the conventional QEMU 16550 port; only this bootstrap core uses it.
        {
            cpu::port_out(0x3f9, 0x00);
            cpu::port_out(0x3fb, 0x80);
            cpu::port_out(0x3f8, 0x03);
            cpu::port_out(0x3f9, 0x00);
            cpu::port_out(0x3fb, 0x03);
            cpu::port_out(0x3fa, 0xc7);
            cpu::port_out(0x3fc, 0x0b);
        }
    }
    fn byte(b: u8) {
        // SAFETY: polling COM1 and writing its data register is valid after Serial::init.
        while (cpu::port_in(0x3fd) & 0x20) == 0 {}
        cpu::port_out(0x3f8, b);
    }
    fn line(s: &str) {
        for b in s.bytes() {
            if b == b'\n' {
                Self::byte(b'\r');
            }
            Self::byte(b);
        }
        Self::byte(b'\r');
        Self::byte(b'\n');
    }
}

mod font;
use font::glyph;

pub(crate) fn pixel(f: &nova_core::Framebuffer, x: usize, y: usize, rgb: u32) {
    if x >= f.width as usize || y >= f.height as usize {
        return;
    }
    let offset = (y * f.stride as usize + x) * 4;
    if offset + 4 > f.size as usize {
        return;
    }
    let value = match f.format {
        0 => ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff),
        1 => rgb,
        _ => return,
    };
    // SAFETY: GOP supplied the mapped framebuffer and checked offset is within its byte size.
    unsafe {
        (f.base as *mut u32).add(offset / 4).write_volatile(value);
    }
}
fn fill(f: &nova_core::Framebuffer, rgb: u32) {
    for y in 0..f.height as usize {
        for x in 0..f.width as usize {
            pixel(f, x, y, rgb);
        }
    }
}
pub(crate) fn draw(f: &nova_core::Framebuffer, x: usize, y: usize, s: &str, rgb: u32) {
    for (i, c) in s.bytes().enumerate() {
        let rows = glyph(c);
        for (dy, row) in rows.iter().enumerate() {
            for dx in 0..8 {
                if row & (0x80 >> dx) != 0 {
                    pixel(f, x + i * 9 + dx, y + dy, rgb);
                }
            }
        }
    }
}

pub(crate) fn log(s: &str) {
    Serial::line(s);
}
pub(crate) fn framebuffer()->Option<nova_core::Framebuffer>{
    // SAFETY: the GOP descriptor is copied once before interrupts and remains
    // immutable for the rest of boot. A copy contains only dimensions/address.
    unsafe { DIAG_FB }
}
static mut DIAG_FB: Option<nova_core::Framebuffer> = None;
static DIAG_COL: AtomicUsize = AtomicUsize::new(0);
static DIAG_ROW: AtomicUsize = AtomicUsize::new(0);
static DIAG_BASE_Y: AtomicUsize = AtomicUsize::new(272);
static PANICKING: AtomicBool = AtomicBool::new(false);
pub(crate) fn begin_fatal_screen(){
    DIAG_COL.store(0,Ordering::Relaxed);
    DIAG_ROW.store(0,Ordering::Relaxed);
    DIAG_BASE_Y.store(32,Ordering::Relaxed);
    if let Some(f)=framebuffer(){fill(&f,0x1b1014)}
}
pub(crate) fn serial_byte(b: u8) {
    Serial::byte(b);
}
pub(crate) struct Diagnostic;
impl Write for Diagnostic {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                Serial::byte(b'\r');
            }
            Serial::byte(b);
            if b==b'\n'{DIAG_COL.store(0,Ordering::Relaxed);DIAG_ROW.fetch_add(1,Ordering::Relaxed);continue}
            let col=DIAG_COL.fetch_add(1,Ordering::Relaxed);
            if col>=100 {DIAG_COL.store(1,Ordering::Relaxed);DIAG_ROW.fetch_add(1,Ordering::Relaxed);}
            let row=DIAG_ROW.load(Ordering::Relaxed);
            // SAFETY: the framebuffer is set once before interrupts are enabled. Fatal diagnostics
            // run with interrupts disabled; immutable copying cannot race with initialization.
            if let Some(f) = unsafe { DIAG_FB }.filter(|_| PANICKING.load(Ordering::Relaxed) || graphics::console_visible()) {
                if b != b'\n' && b >= 32 && b < 127 {
                    let x = (col % 100) * 9 + 48;
                    let y = row * 10 + DIAG_BASE_Y.load(Ordering::Relaxed);
                    if y + 8 < f.height as usize {
                        draw(
                            &f,
                            x,
                            y,
                            core::str::from_utf8(&[b]).unwrap_or("?"),
                            0xe5e9f0,
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
#[no_mangle]
pub extern "sysv64" fn _start(info: *const NovaBootInfo) -> ! {
    Serial::init();
    Serial::line("[NOVA] Nova Kernel starting");
    Serial::line("[NOVA] Architecture: x86_64");
    if info.is_null() {
        Serial::line("[NOVA] ERROR: null BootInfo");
        halt();
    }
    // SAFETY: NovaBoot passes a live, page-backed NovaBootInfo and never reclaims it.
    let info = unsafe { &*info };
    if info.magic != BOOT_MAGIC
        || info.version != BOOT_VERSION
        || info.size < core::mem::size_of::<NovaBootInfo>() as u32
    {
        Serial::line("[NOVA] ERROR: incompatible BootInfo");
        halt();
    }
    Serial::line("[NOVA] Boot protocol: NovaBoot");
    arch::x86_64::init();
    let cpu_info = arch::x86_64::cpuid::detect();
    let vendor = core::str::from_utf8(&cpu_info.vendor).unwrap_or("Unavailable");
    let mut diag = Diagnostic;
    let _ = writeln!(
        diag,
        "[NOVA][CPU] Vendor: {} family {} model {} stepping {}",
        vendor, cpu_info.family, cpu_info.model, cpu_info.stepping
    );
    Serial::line("[NOVA] Serial: OK");
    if info.memory_map == 0 || info.memory_map_size == 0 || info.memory_descriptor_size == 0 {
        Serial::line("[NOVA] ERROR: missing memory map");
        halt();
    }
    Serial::line("[NOVA] Memory Map: OK");
    let f = &info.framebuffer;
    if f.base == 0
        || f.width == 0
        || f.height == 0
        || f.stride < f.width
        || f.size < f.stride as u64 * f.height as u64 * 4
        || f.format > 1
    {
        Serial::line("[NOVA] ERROR: unsupported framebuffer");
        halt();
    }
    Serial::line("[NOVA] Framebuffer: OK");
    // SAFETY: valid GOP description is copied once, before IRQs are enabled.
    unsafe {
        DIAG_FB = Some(*f);
    }
    let stats =
        memory::physical::init(info).unwrap_or_else(|_| panic!("physical memory map invalid"));
    nova_log!(Info, "MEM", "Physical allocator initialized");
    let mut diag = Diagnostic;
    let _ = writeln!(
        diag,
        "[NOVA][MEM] {} MiB usable",
        stats.usable_bytes / 1048576
    );
    memory::paging::init().unwrap_or_else(|_| panic!("active paging tables inaccessible"));
    let _ = writeln!(diag, "[NOVA][MEM] CR3: {:#x}", cpu::read_cr3());
    let nx = memory::paging::enable_nx(cpu_info.features & arch::x86_64::cpuid::feature::NX != 0);
    nova_log!(Info, "MEM", "Paging initialized");
    memory::heap::init(nx).unwrap_or_else(|_| panic!("kernel heap mapping failed"));
    nova_log!(Info, "MEM", "Heap initialized");
    let pci_devices=drivers::pci::scan();
    block::init(&pci_devices).unwrap_or_else(|e|panic!("block device init: {:?}",e));
    let mut boot_sector=[0u8;1024];
    block::read(0,&mut boot_sector).unwrap_or_else(|e|panic!("block read: {:?}",e));
    if boot_sector[510]!=0x55 || boot_sector[511]!=0xaa {panic!("block boot sector signature missing");}
    nova_log!(Info,"BLOCK","sector 0 and 1 read successfully");
    fs::vfs::mount_root().unwrap_or_else(|e|panic!("root filesystem mount: {:?}",e));
    nova_log!(Info,"VFS","FAT32 mounted at /");
    device::mount().unwrap_or_else(|e|panic!("/dev mount: {:?}",e));
    fs::procfs::mount().unwrap_or_else(|e|panic!("/proc mount: {:?}",e));
    nova_log!(Info,"VFS","device and process filesystems mounted");
    scheduler::init();
    nova_log!(Info, "SCHED", "cooperative scheduler initialized");
    process::init(info);
    nova_log!(Info, "PROC", "process manager initialized");
    arch::x86_64::syscall::init();
    nova_log!(Info, "SYSCALL", "SYSCALL/SYSRET configured");
    #[cfg(feature = "selftest")]
    memory::address_space::selftest(info);
    #[cfg(feature = "selftest")]
    userspace::elf::selftest(info, nx);
    #[cfg(feature = "selftest")]
    memory::selftest::run(info);
    let topology=platform::acpi::discover(info).unwrap_or_else(|e|panic!("ACPI topology: {:?}",e));
    arch::x86_64::percpu::set_discovered(topology.processor_count);
    nova_log!(Info,"ACPI","MADT discovered {} enabled xAPIC processors; {} online",topology.processor_count,1);
    for processor in topology.processors[..topology.processor_count].iter() {
        let Some(processor)=processor else {continue};
        nova_log!(Info,"ACPI","processor ACPI ID {} APIC ID {}",processor.acpi_id,processor.apic_id);
    }
    arch::x86_64::apic::init(&topology, cpu_info.features)
        .unwrap_or_else(|_| panic!("APIC initialization failed"));
    arch::x86_64::percpu::init_current(0,arch::x86_64::apic::local_id() as u32,0);
    arch::x86_64::percpu::set_state(0,arch::x86_64::percpu::State::Running);
    #[cfg(feature = "selftest")]
    scheduler::selftest();
    nova_log!(Info, "INT", "Local APIC and IOAPIC initialized");
    interrupts::timer::init();
    nova_log!(Info, "TIME", "PIT timer initialized");
    drivers::ps2::init().unwrap_or_else(|_| panic!("PS/2 keyboard initialization failed"));
    nova_log!(Info, "INPUT", "PS/2 keyboard initialized");
    tty::init();
    nova_log!(Info,"TTY","terminal input initialized");
    cpu::enable_interrupts();
    arch::x86_64::smp::probe(&topology,info.ap_trampoline);
    cpu::disable_interrupts();
    fill(f, 0x10131b);
    let white = 0xe5e9f0;
    let blue = 0x6ba8e8;
    draw(f, 48, 48, "NOVA OS", blue);
    draw(f, 48, 72, concat!("Kernel ",env!("CARGO_PKG_VERSION")), white);
    draw(f, 48, 112, "Boot protocol     NovaBoot", white);
    draw(f, 48, 128, "Architecture      x86_64", white);
    draw(f, 48, 144, "Firmware          UEFI", white);
    draw(f, 48, 176, "Framebuffer       OK", white);
    draw(f, 48, 192, "Memory Map        OK", white);
    draw(f, 48, 224, "CPU GDT TSS IDT       OK", white);
    draw(f, 48, 240, "MEMORY PAGING HEAP   OK", white);
    draw(f, 48, 256, "INTERRUPTS TIMER KEYBOARD OK", white);
    draw(f, 48, 272, "SCHEDULER SYSCALL    OK", white);
    draw(f, 48, 288, "PCI BLOCK VFS FAT32   OK", white);
    draw(f, 48, 320, "Nova Kernel initialized.", blue);
    Serial::line("[NOVA] Boot screen displayed");
    #[cfg(feature = "exceptiontest")]
    {
        // SAFETY: diagnostic build deliberately raises Invalid Opcode to verify the IDT and reporter.
        unsafe {
            core::arch::asm!("ud2", options(noreturn));
        }
    }
    #[cfg(not(feature = "monitor"))]
    if let Err(error) = start_init(info, &cpu_info, *f) {
        let _ = writeln!(Diagnostic, "[NOVA][INIT] start failed: {}. Emergency monitor follows.\n", error);
    }
    shell::run(info, cpu_info, *f)
}
#[cfg(not(feature = "monitor"))]
fn start_init(info: &NovaBootInfo, cpu_info: &arch::x86_64::cpuid::CpuInfo, fb: nova_core::Framebuffer) -> Result<(), &'static str> {
    use alloc::{string::String, vec};
    let image = fs::vfs::read("/system/init").map_err(|_| "cannot read /system/init")?;
    let space = memory::address_space::AddressSpace::new()?;
    let nx = cpu_info.features & arch::x86_64::cpuid::feature::NX != 0;
    let entry = match userspace::elf::load(&image, &space, nx) {
        Ok(entry) => entry,
        Err(error) => { let _ = space.destroy(info); return Err(error); }
    };
    let args = vec![String::from("/system/init")];
    let env = vec![String::from("PATH=/bin"), String::from("HOME=/home"), String::from("SHELL=/bin/nsh")];
    let stack = match userspace::stack::build(&space, &args, &env) {
        Ok(stack) => stack,
        Err(error) => { let _ = space.destroy(info); return Err(error); }
    };
    let mut terminal = terminal::Terminal::new(fb);
    let pid = process::create("init", space, &mut terminal);
    process::configure_current(entry, stack);
    nova_log!(Info, "INIT", "launching /system/init as PID {}", pid.0);
    scheduler::spawn(pid, userspace::thread_entry);
    cpu::enable_interrupts();
    scheduler::run();
    let result = process::finish(info)?;
    let _ = writeln!(Diagnostic, "[NOVA][INIT] PID {} ended: {:?} code {}\n", result.0.0, result.1, result.2);
    Err("init exited")
}
fn halt() -> ! {
    cpu::halt_forever()
}
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cpu::disable_interrupts();
    if PANICKING.swap(true, Ordering::SeqCst) {
        cpu::halt_forever();
    }
    begin_fatal_screen();
    let mut out = Diagnostic;
    let _ = write!(out, "\nNOVA KERNEL PANIC\nReason: {}\n", info.message());
    if let Some(loc) = info.location() {
        let _ = write!(
            out,
            "Location: {}:{}:{}\n",
            loc.file(),
            loc.line(),
            loc.column()
        );
    }
    let rsp:u64;let rbp:u64;
    // SAFETY: reading current stack/frame pointers changes no CPU state and
    // both registers are valid to report even if frame-pointer chains are absent.
    unsafe{core::arch::asm!("mov {},rsp","mov {},rbp",out(reg)rsp,out(reg)rbp,options(nostack,preserves_flags))}
    let _=write!(out,"PID: {:?} TID: {:?}\nRSP: {:#018x} RBP: {:#018x}\nCR2: {:#018x} CR3: {:#018x}\n",
        process::try_current_pid().map(|p|p.0),scheduler::try_current_tid().map(|t|t.0),rsp,rbp,cpu::read_cr2(),cpu::read_cr3());
    let _ = write!(out, "CPU: 0 (bootstrap processor)\nSystem halted.\n");
    cpu::halt_forever()
}
