#![no_std]

pub mod abi;

pub const BOOT_MAGIC: u64 = 0x4e4f_5641_424f_4f54;
pub const BOOT_VERSION: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Framebuffer {
    pub base: u64,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    /// 0 = RGBX, 1 = BGRX; all other values are unsupported.
    pub format: u32,
}

#[repr(C)]
pub struct NovaBootInfo {
    pub magic: u64,
    pub version: u32,
    pub size: u32,
    pub framebuffer: Framebuffer,
    pub memory_map: u64,
    pub memory_map_size: u64,
    pub memory_descriptor_size: u64,
    pub memory_descriptor_version: u32,
    pub _reserved: u32,
    pub rsdp: u64,
    pub kernel_base: u64,
    pub kernel_size: u64,
    /// UEFI LoaderData page below 1 MiB reserved for an optional AP SIPI trampoline.
    /// Zero means the firmware had no suitable page; the BSP can still boot.
    pub ap_trampoline: u64,
}
