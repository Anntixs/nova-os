use super::cpu;
use core::arch::{asm, global_asm};
use core::fmt::Write;

global_asm!(include_str!("stubs.S"));
global_asm!(include_str!("irq.S"));
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Gate {
    low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    mid: u16,
    high: u32,
    zero: u32,
}
impl Gate {
    const EMPTY: Self = Self {
        low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        mid: 0,
        high: 0,
        zero: 0,
    };
    fn new(address: u64, ist: u8) -> Self {
        Self {
            low: address as u16,
            selector: 8,
            ist,
            attributes: 0x8e,
            mid: (address >> 16) as u16,
            high: (address >> 32) as u32,
            zero: 0,
        }
    }
}
#[repr(C, packed)]
struct Descriptor {
    limit: u16,
    base: u64,
}
static mut IDT: [Gate; 256] = [Gate::EMPTY; 256];
extern "C" {
    static nova_isr_unknown: u8;
    static nova_irq_0: u8;
    static nova_irq_1: u8;
    static nova_irq_spurious: u8;
}
macro_rules! stubs { ($($name:ident),*) => { extern "C" { $(static $name:u8;)* } }; }
stubs!(
    nova_isr_0,
    nova_isr_1,
    nova_isr_2,
    nova_isr_3,
    nova_isr_4,
    nova_isr_5,
    nova_isr_6,
    nova_isr_7,
    nova_isr_8,
    nova_isr_9,
    nova_isr_10,
    nova_isr_11,
    nova_isr_12,
    nova_isr_13,
    nova_isr_14,
    nova_isr_15,
    nova_isr_16,
    nova_isr_17,
    nova_isr_18,
    nova_isr_19,
    nova_isr_20,
    nova_isr_21,
    nova_isr_22,
    nova_isr_23,
    nova_isr_24,
    nova_isr_25,
    nova_isr_26,
    nova_isr_27,
    nova_isr_28,
    nova_isr_29,
    nova_isr_30,
    nova_isr_31
);
const STUBS: [*const u8; 32] = [
    &raw const nova_isr_0,
    &raw const nova_isr_1,
    &raw const nova_isr_2,
    &raw const nova_isr_3,
    &raw const nova_isr_4,
    &raw const nova_isr_5,
    &raw const nova_isr_6,
    &raw const nova_isr_7,
    &raw const nova_isr_8,
    &raw const nova_isr_9,
    &raw const nova_isr_10,
    &raw const nova_isr_11,
    &raw const nova_isr_12,
    &raw const nova_isr_13,
    &raw const nova_isr_14,
    &raw const nova_isr_15,
    &raw const nova_isr_16,
    &raw const nova_isr_17,
    &raw const nova_isr_18,
    &raw const nova_isr_19,
    &raw const nova_isr_20,
    &raw const nova_isr_21,
    &raw const nova_isr_22,
    &raw const nova_isr_23,
    &raw const nova_isr_24,
    &raw const nova_isr_25,
    &raw const nova_isr_26,
    &raw const nova_isr_27,
    &raw const nova_isr_28,
    &raw const nova_isr_29,
    &raw const nova_isr_30,
    &raw const nova_isr_31,
];

pub fn init() {
    // SAFETY: called once with IF clear; the IDT is static and all entries point to static code.
    unsafe {
        let unknown = &raw const nova_isr_unknown as u64;
        for i in 0..256 {
            IDT[i] = Gate::new(unknown, 0);
        }
        for i in 0..32 {
            IDT[i] = Gate::new(
                STUBS[i] as u64,
                match i {
                    8 => 1,
                    2 => 2,
                    18 => 3,
                    _ => 0,
                },
            );
        }
        IDT[32] = Gate::new(&raw const nova_irq_0 as u64, 0);
        IDT[33] = Gate::new(&raw const nova_irq_1 as u64, 0);
        IDT[255] = Gate::new(&raw const nova_irq_spurious as u64, 0);
    }
    load();
}
pub fn load() {
    // SAFETY: BSP completed all IDT writes before AP startup. The table remains
    // immutable afterward, and LIDT changes only the calling CPU's IDTR.
    unsafe {
        let ptr = Descriptor {
            limit: (core::mem::size_of::<[Gate; 256]>() - 1) as u16,
            base: &raw const IDT as u64,
        };
        asm!("lidt [{}]",in(reg)&ptr,options(readonly,nostack));
    }
}
#[repr(C)]
struct FaultFrame {
    vector: u64,
    error: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
}
#[no_mangle]
extern "C" fn nova_exception_dispatch(frame: *const FaultFrame) -> ! {
    cpu::disable_interrupts();
    // SAFETY: assembly stub places these five words on a live exception stack before calling us.
    let f = unsafe { &*frame };
    // NMI is a CPU/platform event, not a fault caused by the interrupted
    // userspace instruction. Treat it as fatal until Nova has an NMI handler.
    if f.cs & 3 == 3 && f.vector != 2 && f.vector != 8 && f.vector != 18 {
        let address=if f.vector==14 {cpu::read_cr2()} else {f.rip};
        crate::process::fault(f.vector,address);
        if let Some(pid)=crate::process::current_pid(){crate::scheduler::kill_process_threads(pid)}
        crate::scheduler::exit_current();
    }
    let names = [
        "DIVIDE ERROR",
        "DEBUG",
        "NMI",
        "BREAKPOINT",
        "OVERFLOW",
        "BOUND RANGE",
        "INVALID OPCODE",
        "DEVICE NOT AVAILABLE",
        "DOUBLE FAULT",
        "COPROCESSOR",
        "INVALID TSS",
        "SEGMENT NOT PRESENT",
        "STACK FAULT",
        "GENERAL PROTECTION FAULT",
        "PAGE FAULT",
        "RESERVED",
        "X87 FLOAT",
        "ALIGNMENT CHECK",
        "MACHINE CHECK",
        "SIMD FLOAT",
        "VIRTUALIZATION",
        "CONTROL PROTECTION",
    ];
    let name = names
        .get(f.vector as usize)
        .copied()
        .unwrap_or("UNKNOWN EXCEPTION");
    crate::begin_fatal_screen();
    let mut out = crate::Diagnostic;
    let _=write!(out,"\n========================================\nNOVA KERNEL EXCEPTION\nException: {}\nVector: {}\nInstruction Pointer: {:#018x}\nCS: {:#06x} RFLAGS: {:#018x}\nError Code: {:#018x}\n",name,f.vector,f.rip,f.cs,f.rflags,f.error);
    if f.vector == 14 {
        let address = cpu::read_cr2();
        let _ = write!(
            out,
            "Fault Address: {:#018x}\nReason: {} | {} | {}\n",
            address,
            if f.error & 2 != 0 { "WRITE" } else { "READ" },
            if f.error & 4 != 0 {
                "USER"
            } else {
                "SUPERVISOR"
            },
            if f.error & 1 != 0 {
                "PRESENT"
            } else {
                "NOT PRESENT"
            }
        );
    }
    let cpu_id=super::apic::try_local_id();
    let _ = write!(
        out,
        "PID: {:?} TID: {:?}\nCR3: {:#018x}\nLocal APIC ID: {:?}\nSystem halted.\n========================================\n",
        crate::process::try_current_pid().map(|p|p.0),crate::scheduler::try_current_tid().map(|t|t.0),cpu::read_cr3(),cpu_id
    );
    cpu::halt_forever()
}
