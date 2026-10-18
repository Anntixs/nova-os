#![no_std]
#![no_main]

use core::{arch::asm, ffi::c_void, mem::size_of, panic::PanicInfo, ptr};
use nova_core::{Framebuffer, NovaBootInfo, BOOT_MAGIC, BOOT_VERSION};

type Status = usize;
type Handle = *mut c_void;
const SUCCESS: Status = 0;
const ALLOCATE_ANY: u32 = 0;
const ALLOCATE_MAX: u32 = 1;
const ALLOCATE_ADDRESS: u32 = 2;
const LOADER_DATA: u32 = 2;
const MAX_KERNEL: usize = 8 * 1024 * 1024;
const MAP_CAPACITY: usize = 256 * 1024;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Guid { a: u32, b: u16, c: u16, d: [u8; 8] }
const FILE_SYSTEM: Guid = Guid { a: 0x964e5b22, b: 0x6459, c: 0x11d2, d: [0x8e,0x39,0x00,0xa0,0xc9,0x69,0x72,0x3b] };
const LOADED_IMAGE: Guid = Guid { a: 0x5b1b31a1, b: 0x9562, c: 0x11d2, d: [0x8e,0x3f,0x00,0xa0,0xc9,0x69,0x72,0x3b] };
const GOP: Guid = Guid { a: 0x9042a9de, b: 0x23dc, c: 0x4a38, d: [0x96,0xfb,0x7a,0xde,0xd0,0x80,0x51,0x6a] };
const ACPI2: Guid = Guid { a: 0x8868e871, b: 0xe4f1, c: 0x11d3, d: [0xbc,0x22,0x00,0x80,0xc7,0x3c,0x88,0x81] };
const ACPI1: Guid = Guid { a: 0xeb9d2d30, b: 0x2d88, c: 0x11d3, d: [0x9a,0x16,0x00,0x90,0x27,0x3f,0xc1,0x4d] };
#[repr(C)] struct ConfigEntry { guid: Guid, table: *const c_void }
#[repr(C)] struct SystemTable {
    hdr: [u8;24], vendor: *const u16, revision: u32, _pad: u32,
    stdin_handle: Handle, stdin: usize, stdout_handle: Handle, stdout: usize,
    stderr_handle: Handle, stderr: usize, runtime: usize, boot: *mut BootServices,
    config_count: usize, config: *const ConfigEntry,
}
#[repr(C)] struct BootServices { hdr: [u8;24], fnptr: [usize; 45] }
impl BootServices {
    unsafe fn allocate_pages(&self, kind: u32, pages: usize, address: &mut u64) -> Status {
        let f: extern "efiapi" fn(u32,u32,usize,*mut u64)->Status = core::mem::transmute(self.fnptr[2]);
        f(kind, LOADER_DATA, pages, address)
    }
    unsafe fn locate(&self, guid: &Guid, interface: &mut *mut c_void) -> Status {
        let f: extern "efiapi" fn(*const Guid,*const c_void,*mut *mut c_void)->Status = core::mem::transmute(self.fnptr[37]);
        f(guid, ptr::null(), interface)
    }
    unsafe fn handle_protocol(&self, handle: Handle, guid: &Guid, interface: &mut *mut c_void) -> Status {
        // SAFETY: UEFI BootServices slot 16 has the HandleProtocol ABI. The caller
        // supplies a firmware handle and a live output pointer before ExitBootServices.
        let f: extern "efiapi" fn(Handle,*const Guid,*mut *mut c_void)->Status = core::mem::transmute(self.fnptr[16]);
        f(handle,guid,interface)
    }
    unsafe fn memory_map(&self, size: &mut usize, buffer: *mut c_void, key: &mut usize, desc_size: &mut usize, version: &mut u32) -> Status {
        let f: extern "efiapi" fn(*mut usize,*mut c_void,*mut usize,*mut usize,*mut u32)->Status = core::mem::transmute(self.fnptr[4]);
        f(size, buffer, key, desc_size, version)
    }
    unsafe fn exit(&self, image: Handle, key: usize) -> Status {
        let f: extern "efiapi" fn(Handle,usize)->Status = core::mem::transmute(self.fnptr[26]);
        f(image,key)
    }
}
#[repr(C)] struct FileSystem { revision: u64, open_volume: extern "efiapi" fn(*mut FileSystem,*mut *mut File)->Status }
#[repr(C)] struct LoadedImage {revision:u32,_pad:u32,parent:Handle,system_table:*mut SystemTable,device:Handle}
#[repr(C)] struct File {
    revision: u64,
    open: extern "efiapi" fn(*mut File,*mut *mut File,*const u16,u64,u64)->Status,
    close: extern "efiapi" fn(*mut File)->Status,
    delete: usize,
    read: extern "efiapi" fn(*mut File,*mut usize,*mut c_void)->Status,
}
#[repr(C)] struct Gop { query_mode: usize, set_mode: usize, blt: usize, mode: *const GopMode }
#[repr(C)] struct GopMode { max_mode: u32, current_mode: u32, info: *const GopInfo, info_size: usize, base: u64, size: usize }
#[repr(C)] struct GopInfo { version: u32, width: u32, height: u32, format: u32, masks: [u32;4], stride: u32 }

fn serial_byte(b: u8) { // SAFETY: serial I/O uses the standard COM1 port, single-threaded before handoff.
    unsafe { while (inp(0x3fd) & 0x20) == 0 {} out(0x3f8,b); }
}
fn log(s: &str) { for b in s.bytes() { if b == b'\n' { serial_byte(b'\r'); } serial_byte(b); } serial_byte(b'\r'); serial_byte(b'\n'); }
unsafe fn out(p: u16, v: u8) { asm!("out dx, al", in("dx") p, in("al") v, options(nomem,nostack)); }
unsafe fn inp(p: u16) -> u8 { let v: u8; asm!("in al, dx", in("dx") p, out("al") v, options(nomem,nostack)); v }
fn die(s: &str) -> ! { log(s); loop { // SAFETY: fatal boot failure; stop this core.
    unsafe { asm!("cli; hlt", options(nomem,nostack)); }
} }
fn check(status: Status, message: &str) { if status != SUCCESS { die(message); } }
fn pages(bytes: usize) -> usize { (bytes + 4095) / 4096 }
fn read_u16(b: &[u8], at: usize) -> u16 { u16::from_le_bytes([b[at],b[at+1]]) }
fn read_u32(b: &[u8], at: usize) -> u32 { u32::from_le_bytes(b[at..at+4].try_into().unwrap()) }
fn read_u64(b: &[u8], at: usize) -> u64 { u64::from_le_bytes(b[at..at+8].try_into().unwrap()) }

#[no_mangle]
extern "efiapi" fn efi_main(image: Handle, st: *mut SystemTable) -> Status {
    // SAFETY: UEFI invokes this entry with a valid system table and firmware-owned protocols.
    unsafe { boot(image, st) }
}
unsafe fn boot(image: Handle, st: *mut SystemTable) -> ! {
    if st.is_null() { die("[NovaBoot] null system table"); }
    out(0x3f9,0); out(0x3fb,0x80); out(0x3f8,3); out(0x3f9,0); out(0x3fb,3); out(0x3fa,0xc7); out(0x3fc,0x0b);
    log("[NovaBoot] UEFI application started");
    let bs = &*(*st).boot;
    // SAFETY: image is the UEFI supplied handle; LoadedImage.DeviceHandle identifies
    // the filesystem that contains this bootloader, even when other FAT disks exist.
    let mut loaded = ptr::null_mut();
    check(bs.handle_protocol(image,&LOADED_IMAGE,&mut loaded),"[NovaBoot] loaded image unavailable");
    let mut fs_ptr = ptr::null_mut();
    // SAFETY: successful HandleProtocol returns a valid LoadedImage pointer owned by UEFI.
    check(bs.handle_protocol((*(loaded as *const LoadedImage)).device,&FILE_SYSTEM,&mut fs_ptr), "[NovaBoot] boot file system unavailable");
    let fs = &mut *(fs_ptr as *mut FileSystem);
    let mut root = ptr::null_mut();
    check((fs.open_volume)(fs,&mut root), "[NovaBoot] open volume failed");
    let path: [u16; 16] = [b'\\' as u16,b'E' as u16,b'F' as u16,b'I' as u16,b'\\' as u16,b'N' as u16,b'O' as u16,b'V' as u16,b'A' as u16,b'\\' as u16,b'K' as u16,b'E' as u16,b'R' as u16,b'N' as u16,b'E' as u16,b'L' as u16];
    let mut full_path = [0u16; 21];
    full_path[..16].copy_from_slice(&path);
    full_path[16..20].copy_from_slice(&[b'.' as u16,b'E' as u16,b'L' as u16,b'F' as u16]);
    let mut file = ptr::null_mut();
    check(((*root).open)(root,&mut file,full_path.as_ptr(),1,0), "[NovaBoot] KERNEL.ELF missing");
    let mut image_base = 0u64;
    check(bs.allocate_pages(ALLOCATE_ANY,pages(MAX_KERNEL),&mut image_base), "[NovaBoot] ELF buffer allocation failed");
    let mut image_size = MAX_KERNEL;
    check(((*file).read)(file,&mut image_size,image_base as *mut c_void), "[NovaBoot] kernel read failed");
    check(((*file).close)(file), "[NovaBoot] kernel close failed");
    check(((*root).close)(root), "[NovaBoot] volume close failed");
    if image_size < 64 { die("[NovaBoot] short ELF"); }
    let elf = core::slice::from_raw_parts(image_base as *const u8,image_size);
    if &elf[0..4] != b"\x7fELF" || elf[4] != 2 || elf[5] != 1 || read_u16(elf,16) != 2 || read_u16(elf,18) != 62 { die("[NovaBoot] unsupported ELF"); }
    let entry = read_u64(elf,24);
    let phoff = read_u64(elf,32) as usize;
    let phsize = read_u16(elf,54) as usize;
    let phcount = read_u16(elf,56) as usize;
    if phsize < 56 || phcount == 0 || phoff.checked_add(phsize.saturating_mul(phcount)).map_or(true,|x| x > image_size) { die("[NovaBoot] corrupt ELF headers"); }
    let mut low = u64::MAX;
    let mut high = 0u64;
    for i in 0..phcount {
        let p = phoff + i*phsize;
        if read_u32(elf,p) != 1 { continue; }
        let offset = read_u64(elf,p+8) as usize;
        let vaddr = read_u64(elf,p+16);
        let paddr = read_u64(elf,p+24);
        let filesz = read_u64(elf,p+32) as usize;
        let memsz = read_u64(elf,p+40) as usize;
        if vaddr != paddr || paddr < 0x200000 || paddr & 4095 != 0 || memsz == 0 || filesz > memsz || offset.checked_add(filesz).map_or(true,|x| x > image_size) || paddr.checked_add(memsz as u64).is_none() { die("[NovaBoot] invalid load segment"); }
        let mut target = paddr;
        check(bs.allocate_pages(ALLOCATE_ADDRESS,pages(memsz),&mut target), "[NovaBoot] segment allocation failed");
        if target != paddr { die("[NovaBoot] segment address mismatch"); }
        ptr::write_bytes(paddr as *mut u8,0,pages(memsz)*4096);
        ptr::copy_nonoverlapping(elf.as_ptr().add(offset),paddr as *mut u8,filesz);
        low = low.min(paddr); high = high.max(paddr + memsz as u64);
    }
    if low == u64::MAX || entry < low || entry >= high { die("[NovaBoot] invalid kernel entry"); }
    log("[NovaBoot] ELF segments loaded");
    let mut gop_ptr = ptr::null_mut();
    check(bs.locate(&GOP,&mut gop_ptr), "[NovaBoot] GOP unavailable");
    let mode = &*(*(gop_ptr as *const Gop)).mode;
    let g = &*mode.info;
    let format = match g.format { 0 => 0, 1 => 1, _ => die("[NovaBoot] unsupported GOP pixel format") };
    let framebuffer = Framebuffer { base: mode.base, size: mode.size as u64, width: g.width, height: g.height, stride: g.stride, format };
    log("[NovaBoot] GOP framebuffer found");
    let mut rsdp = 0;
    if (*st).config_count != 0 {
        if (*st).config.is_null() { die("[NovaBoot] invalid configuration table"); }
        let entries = core::slice::from_raw_parts((*st).config,(*st).config_count);
        for e in entries { if e.guid == ACPI2 { rsdp = e.table as u64; break; } if e.guid == ACPI1 { rsdp = e.table as u64; } }
    }
    let mut info_page = 0u64;
    check(bs.allocate_pages(ALLOCATE_ANY,1,&mut info_page), "[NovaBoot] BootInfo allocation failed");
    let mut map_page = 0u64;
    check(bs.allocate_pages(ALLOCATE_ANY,pages(MAP_CAPACITY),&mut map_page), "[NovaBoot] memory map allocation failed");
    let mut stack_page = 0u64;
    check(bs.allocate_pages(ALLOCATE_ANY,16,&mut stack_page), "[NovaBoot] stack allocation failed");
    let mut ap_trampoline = 0x000f_ffffu64;
    if bs.allocate_pages(ALLOCATE_MAX,1,&mut ap_trampoline) != SUCCESS || ap_trampoline < 0x1000 || ap_trampoline >= 0x100000 || ap_trampoline & 4095 != 0 {
        ap_trampoline = 0;
        log("[NovaBoot] no low AP trampoline page; SMP startup unavailable");
    }
    let info = &mut *(info_page as *mut NovaBootInfo);
    *info = NovaBootInfo { magic: BOOT_MAGIC, version: BOOT_VERSION, size: size_of::<NovaBootInfo>() as u32, framebuffer, memory_map: map_page, memory_map_size: 0, memory_descriptor_size: 0, memory_descriptor_version: 0, _reserved: 0, rsdp, kernel_base: low, kernel_size: high-low, ap_trampoline };
    for _ in 0..8 {
        let mut map_size = MAP_CAPACITY;
        let mut key = 0usize;
        let mut desc_size = 0usize;
        let mut version = 0u32;
        check(bs.memory_map(&mut map_size,map_page as *mut c_void,&mut key,&mut desc_size,&mut version), "[NovaBoot] memory map failed or too large");
        info.memory_map_size = map_size as u64;
        info.memory_descriptor_size = desc_size as u64;
        info.memory_descriptor_version = version;
        if bs.exit(image,key) == SUCCESS {
            // SAFETY: allocated loader pages stay mapped after ExitBootServices. The dedicated
            // stack is 16-byte aligned; the kernel entry uses SysV and never returns.
            asm!("mov rsp, {stack}", "xor rbp, rbp", "jmp {entry}", stack = in(reg) (stack_page + 16*4096), entry = in(reg) entry, in("rdi") info_page, options(noreturn));
        }
    }
    die("[NovaBoot] ExitBootServices failed repeatedly")
}
#[panic_handler]
fn panic(_: &PanicInfo) -> ! { die("[NovaBoot] panic") }
