pub mod cpu;
pub mod cpuid;
pub mod gdt;
pub mod idt;
pub mod context;
pub mod userspace;
pub mod syscall;

pub fn init() {
    gdt::init();
    crate::nova_log!(Info, "CPU", "GDT initialized");
    crate::nova_log!(Info, "CPU", "TSS initialized");
    idt::init();
    crate::nova_log!(Info, "CPU", "IDT initialized");
}
pub mod apic;
