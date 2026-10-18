use core::{arch::asm,cell::UnsafeCell};

#[repr(C, packed)]
struct Descriptor {
    limit: u16,
    base: u64,
}
#[repr(C, packed)]
#[derive(Clone,Copy)]
struct Tss {
    reserved0: u32,
    rsp: [u64; 3],
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap: u16,
}
static mut TSS: Tss = Tss {
    reserved0: 0,
    rsp: [0; 3],
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    iomap: 104,
};
static mut GDT: [u64; 7] = [0; 7];
static mut DF_STACK: [u8; 16384] = [0; 16384];
static mut NMI_STACK: [u8; 8192] = [0; 8192];
static mut MC_STACK: [u8; 8192] = [0; 8192];

#[repr(align(16))]
struct ApTables {
    tss:Tss,
    gdt:[u64;7],
    df:[u8;16384],
    nmi:[u8;8192],
    mc:[u8;8192],
}
impl ApTables {
    const fn new()->Self {Self{tss:Tss{reserved0:0,rsp:[0;3],reserved1:0,ist:[0;7],reserved2:0,reserved3:0,iomap:104},gdt:[0;7],df:[0;16384],nmi:[0;8192],mc:[0;8192]}}
}
struct ApCell(UnsafeCell<ApTables>);
// SAFETY: only the AP assigned this logical index writes its cell. Every cell
// is fully initialized before that AP enables interrupts or publishes ready.
unsafe impl Sync for ApCell {}
static AP_TABLES:[ApCell;8]=[const{ApCell(UnsafeCell::new(ApTables::new()))};8];

pub fn init() {
    // SAFETY: called once on the bootstrap CPU with interrupts disabled. The static tables and
    // IST stacks remain pinned for the life of the kernel. No AP uses them yet.
    unsafe {
        let tss = &raw mut TSS as u64;
        TSS.ist[0] = (&raw mut DF_STACK as u64) + 16384;
        TSS.ist[1] = (&raw mut NMI_STACK as u64) + 8192;
        TSS.ist[2] = (&raw mut MC_STACK as u64) + 8192;
        GDT[1] = 0x00af9a000000ffff;
        GDT[2] = 0x00cf92000000ffff;
        // SYSRET derives SS=STAR.user_base+8 and CS=STAR.user_base+16.
        GDT[3] = 0x00cff2000000ffff; // user data (0x1b)
        GDT[4] = 0x00affa000000ffff; // user 64-bit code (0x23)
        let limit = (core::mem::size_of::<Tss>() - 1) as u64;
        GDT[5] = (limit & 0xffff)
            | ((tss & 0xffffff) << 16)
            | (0x89 << 40)
            | (((limit >> 16) & 0xf) << 48)
            | (((tss >> 24) & 0xff) << 56);
        GDT[6] = tss >> 32;
        let gdtr = Descriptor {
            limit: (core::mem::size_of::<[u64; 7]>() - 1) as u16,
            base: &raw const GDT as u64,
        };
        asm!("lgdt [{}]", in(reg) &gdtr, options(readonly,nostack));
        asm!("push 0x08", "lea rax, [rip + 2f]", "push rax", "retfq", "2:",
            "mov ax, 0x10", "mov ds, ax", "mov es, ax", "mov ss, ax", "mov fs, ax", "mov gs, ax",
            out("rax") _, options(preserves_flags));
        asm!("ltr ax", in("ax") 0x28u16, options(nostack));
    }
}
pub fn set_rsp0(top: u64) {
    // SAFETY: called on the bootstrap CPU for a live, pinned thread kernel stack.
    unsafe { TSS.rsp[0] = top; }
}
pub fn init_ap(logical:usize,stack_top:u64){
    assert!(logical<AP_TABLES.len());
    // SAFETY: BSP never mutates AP_TABLES; each AP owns a distinct logical slot.
    // Its stacks and descriptor table remain pinned for the kernel lifetime.
    unsafe {
        let table=&mut *AP_TABLES[logical].0.get();
        table.tss.rsp[0]=stack_top;
        table.tss.ist[0]=table.df.as_mut_ptr() as u64+table.df.len() as u64;
        table.tss.ist[1]=table.nmi.as_mut_ptr() as u64+table.nmi.len() as u64;
        table.tss.ist[2]=table.mc.as_mut_ptr() as u64+table.mc.len() as u64;
        table.gdt[1]=0x00af9a000000ffff;
        table.gdt[2]=0x00cf92000000ffff;
        table.gdt[3]=0x00cff2000000ffff;
        table.gdt[4]=0x00affa000000ffff;
        let tss=&raw const table.tss as u64;
        let limit=(core::mem::size_of::<Tss>()-1) as u64;
        table.gdt[5]=(limit&0xffff)|((tss&0xffffff)<<16)|(0x89<<40)|(((limit>>16)&0xf)<<48)|(((tss>>24)&0xff)<<56);
        table.gdt[6]=tss>>32;
        let gdtr=Descriptor{limit:(core::mem::size_of::<[u64;7]>()-1) as u16,base:table.gdt.as_ptr() as u64};
        asm!("lgdt [{}]",in(reg)&gdtr,options(readonly,nostack));
        asm!("push 0x08", "lea rax, [rip + 2f]", "push rax", "retfq", "2:",
            "mov ax, 0x10", "mov ds, ax", "mov es, ax", "mov ss, ax", "mov fs, ax", "mov gs, ax",
            out("rax") _, options(preserves_flags));
        asm!("ltr ax",in("ax")0x28u16,options(nostack));
    }
}
