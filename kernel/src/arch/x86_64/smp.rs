//! Sequential AP bring-up probe. APs enter long-mode Nova code, load private
//! GDT/TSS/IST state, acknowledge, then enter a kernel-only scheduler host loop.
use core::{arch::{asm,global_asm},ptr,sync::atomic::{AtomicBool,AtomicU64,Ordering}};
use crate::{platform::acpi::Topology,interrupts::timer};
use super::{apic,cpu,gdt,idt,percpu};

global_asm!(include_str!("smp.S"));
unsafe extern "C" {
    static nova_ap_start:u8; static nova_ap_end:u8;
    static nova_ap_gdtr_base:u8; static nova_ap_gdt:u8;
    static nova_ap_jump32_target:u8; static nova_ap_pm32:u8;
    static nova_ap_jump64_target:u8; static nova_ap_lm64:u8;
    static nova_ap_base_imm:u8; static nova_ap_cr3:u8;
    static nova_ap_stack:u8; static nova_ap_logical:u8; static nova_ap_entry:u8;
}
#[repr(align(16))]
#[derive(Clone,Copy)]
struct ApStack([u8;16384]);
static mut AP_STACKS:[ApStack;8]=[ApStack([0;16384]);8];
static REACHED:AtomicU64=AtomicU64::new(0);
static SCHED_START:AtomicBool=AtomicBool::new(false);
#[cfg(feature="smptest")]
static STRESS_DONE:AtomicU64=AtomicU64::new(0);
#[cfg(feature="smptest")]
static STRESS_GO:AtomicBool=AtomicBool::new(false);
#[cfg(feature="smptest")]
static STRESS_FAILED:AtomicU64=AtomicU64::new(0);

#[cfg(feature="smptest")]
fn ap_allocator_stress(logical:usize){
    use alloc::vec::Vec;
    use crate::memory::physical;
    percpu::set_state(logical,percpu::State::Running);
    for iteration in 0..256u64 {
        let frame=match physical::allocate_frame(){Ok(frame)=>frame,Err(_)=>{STRESS_FAILED.fetch_or(1<<logical,Ordering::Release);break}};
        // SAFETY: allocator returned a distinct identity-mapped physical page
        // owned by this AP until it is returned below.
        unsafe{(frame.address() as *mut u64).write_volatile((logical as u64)<<32|iteration)}
        let mut bytes=Vec::with_capacity(128);
        for value in 0..128 {bytes.push((value as u8)^(logical as u8));}
        if bytes.iter().enumerate().any(|(i,&byte)|byte!=((i as u8)^(logical as u8))){
            STRESS_FAILED.fetch_or(1<<logical,Ordering::Release);
        }
        // SAFETY: the frame is still owned by this AP and has not been freed.
        if unsafe{(frame.address() as *const u64).read_volatile()}!=((logical as u64)<<32|iteration){
            STRESS_FAILED.fetch_or(1<<logical,Ordering::Release);
        }
        if physical::free_allocated_frame(frame).is_err(){STRESS_FAILED.fetch_or(1<<logical,Ordering::Release);}
    }
    percpu::set_state(logical,percpu::State::Idle);
    STRESS_DONE.fetch_or(1<<logical,Ordering::Release);
}

fn offset(symbol:*const u8)->usize {(symbol as usize)-((&raw const nova_ap_start) as usize)}
fn patch32(base:u64,symbol:*const u8,value:u32){
    // SAFETY: NovaBoot reserved this mapped low page, and offset was checked
    // against the 4 KiB trampoline size before any patch operation.
    unsafe{ptr::write_unaligned((base as usize+offset(symbol)) as *mut u32,value.to_le())}
}
fn patch64(base:u64,symbol:*const u8,value:u64){
    // SAFETY: same exclusive low-page ownership and checked symbol bounds.
    unsafe{ptr::write_unaligned((base as usize+offset(symbol)) as *mut u64,value.to_le())}
}
extern "sysv64" fn ap_enter(logical:u64)->!{
    // No shared allocator, process table, scheduler or serial access occurs here.
    if logical>=8 {cpu::halt_forever()}
    let stack=unsafe{(&raw mut AP_STACKS as *mut ApStack).add(logical as usize) as u64+16384};
    gdt::init_ap(logical as usize,stack);
    percpu::init_current(logical as usize,apic::local_id() as u32,stack);
    if percpu::current_logical()!=logical as usize {cpu::halt_forever()}
    // SAFETY: the trampoline enabled CR0.MP and CR4.OSFXSR before any AP IRQ
    // can use the shared FXSAVE-based interrupt stub.
    unsafe{asm!("fninit",options(nostack));}
    idt::load();
    if crate::memory::paging::nx_enabled(){crate::memory::paging::enable_nx(true);}
    apic::enable_local();
    // SYSCALL MSRs are local to each CPU. Prepare the same Nova ABI entry as
    // the BSP before this AP can ever be admitted to user-thread scheduling.
    super::syscall::init();
    percpu::set_state(logical as usize,percpu::State::Idle);
    REACHED.fetch_or(1u64<<logical,Ordering::Release);
    #[cfg(feature="smptest")]
    {
        while !STRESS_GO.load(Ordering::Acquire){core::hint::spin_loop()}
        ap_allocator_stress(logical as usize);
    }
    loop {
        if SCHED_START.load(Ordering::Acquire){
            percpu::set_state(logical as usize,percpu::State::Running);
            crate::scheduler::run_ap();
        }
        // SAFETY: this AP has its own GDT/TSS/IST and an immutable IDT. STI
        // immediately followed by HLT avoids losing the scheduler-start IPI.
        unsafe{asm!("sti; hlt",options(nomem,nostack));}
    }
}
/// Admit APs to the kernel-only host loop. User and ordinary kernel threads
/// remain pinned to the BSP until the wider SMP audit is complete.
pub fn start_scheduler()->Result<usize,&'static str>{
    if !cpu::interrupts_enabled(){return Err("AP scheduler start requires BSP interrupts")}
    SCHED_START.store(true,Ordering::Release);
    let mut started=0;
    for logical in 1..8 {
        if !matches!(percpu::state(logical),Some(percpu::State::Idle|percpu::State::Running)){continue}
        let id=percpu::apic_id(logical).ok_or("APIC ID missing")?;
        let id=u8::try_from(id).map_err(|_|"xAPIC ID out of range")?;
        apic::send_ipi(id,241).map_err(|_|"AP scheduler wake IPI failed")?;
        started+=1;
    }
    let deadline=timer::uptime_ms().saturating_add(1000);
    while crate::scheduler::kernel_online_cpus()<started as u32+1 && timer::uptime_ms()<deadline{cpu::halt()}
    if crate::scheduler::kernel_online_cpus()<started as u32+1{return Err("AP scheduler online acknowledgement timed out")}
    Ok(started)
}
fn wait_ms(milliseconds:u64){
    let deadline=timer::uptime_ms().saturating_add(milliseconds);
    while timer::uptime_ms()<deadline {cpu::halt();}
}
fn wait_reached(logical:usize,milliseconds:u64)->bool{
    let deadline=timer::uptime_ms().saturating_add(milliseconds);
    while timer::uptime_ms()<deadline {
        if REACHED.load(Ordering::Acquire)&(1u64<<logical)!=0{return true}
        cpu::halt();
    }
    REACHED.load(Ordering::Acquire)&(1u64<<logical)!=0
}
pub fn probe(topology:&Topology,page:u64){
    if topology.processor_count<=1 {return}
    if page<0x1000||page>=0x100000||page&4095!=0 {
        crate::nova_log!(Warn,"SMP","NovaBoot did not reserve a valid low trampoline page");return
    }
    let size=offset(&raw const nova_ap_end);
    let cr3=cpu::read_cr3();
    if size==0||size>4096||cr3>u32::MAX as u64 {
        crate::nova_log!(Warn,"SMP","trampoline size or CR3 unsupported");return
    }
    // SAFETY: NovaBoot reserved this page as LoaderData through ExitBootServices;
    // its identity mapping and source .rodata remain live for the kernel lifetime.
    unsafe{ptr::copy_nonoverlapping(&raw const nova_ap_start,page as *mut u8,size)}
    patch32(page,&raw const nova_ap_gdtr_base,(page+offset(&raw const nova_ap_gdt) as u64) as u32);
    patch32(page,&raw const nova_ap_jump32_target,(page+offset(&raw const nova_ap_pm32) as u64) as u32);
    patch32(page,&raw const nova_ap_jump64_target,(page+offset(&raw const nova_ap_lm64) as u64) as u32);
    patch32(page,&raw const nova_ap_base_imm,page as u32);
    patch32(page,&raw const nova_ap_cr3,cr3 as u32);
    patch64(page,&raw const nova_ap_entry,ap_enter as usize as u64);
    let bsp=apic::local_id();
    let vector=(page>>12) as u32;
    if topology.processor_count>8 {crate::nova_log!(Warn,"SMP","only 8 processors (including BSP) have prepared stacks")}
    let mut logical=1usize;
    #[cfg(feature="smptest")]
    let mut acknowledged=0u64;
    for processor in topology.processors[..topology.processor_count].iter().flatten() {
        if processor.apic_id==bsp {continue}
        if logical>=8 {break}
        percpu::set_apic_id(logical,processor.apic_id as u32);
        // SAFETY: logical is bounded by the validated 64-entry MADT topology.
        // Every AP gets a distinct pinned stack and startup is sequential.
        let stack=unsafe{(&raw mut AP_STACKS as *mut ApStack).add(logical) as u64+16384};
        patch64(page,&raw const nova_ap_stack,stack);
        patch64(page,&raw const nova_ap_logical,logical as u64);
        percpu::set_state(logical,percpu::State::Starting);
        if apic::send_ipi(processor.apic_id,0xc500).is_err(){
            percpu::set_state(logical,percpu::State::Failed);
            crate::nova_log!(Warn,"SMP","INIT IPI failed for APIC {}; stopping AP startup to preserve trampoline ownership",processor.apic_id);
            break
        }
        wait_ms(10);
        if apic::send_ipi(processor.apic_id,0x8500).is_err(){
            percpu::set_state(logical,percpu::State::Failed);
            crate::nova_log!(Warn,"SMP","INIT deassert failed for APIC {}; stopping AP startup",processor.apic_id);
            break
        }
        wait_ms(1);
        if apic::send_ipi(processor.apic_id,0x600|vector).is_err(){
            percpu::set_state(logical,percpu::State::Failed);
            crate::nova_log!(Warn,"SMP","SIPI failed for APIC {}; stopping AP startup to preserve trampoline ownership",processor.apic_id);
            break
        }
        if !wait_reached(logical,100) {
            if apic::send_ipi(processor.apic_id,0x600|vector).is_err(){
                percpu::set_state(logical,percpu::State::Failed);
                crate::nova_log!(Warn,"SMP","second SIPI failed for APIC {}; stopping AP startup",processor.apic_id);
                break
            }
        }
        if wait_reached(logical,100){
            #[cfg(feature="smptest")]
            {acknowledged|=1u64<<logical;}
            crate::nova_log!(Info,"SMP","APIC {} entered Nova long-mode code; user scheduling remains BSP-only",processor.apic_id);
        }else{
            percpu::set_state(logical,percpu::State::Failed);
            crate::nova_log!(Warn,"SMP","APIC {} failed to acknowledge startup",processor.apic_id);
            // An AP may arrive after our timeout. Keep its patched stack and
            // logical index intact; rewriting this shared low page for another
            // AP could send both CPUs through the same mutable TSS/stack.
            break
        }
        logical+=1;
    }
    #[cfg(feature="smptest")]
    {
        STRESS_GO.store(true,Ordering::Release);
        let expected=acknowledged;
        let deadline=timer::uptime_ms().saturating_add(3000);
        while STRESS_DONE.load(Ordering::Acquire)&expected!=expected&&timer::uptime_ms()<deadline {cpu::halt()}
        if STRESS_DONE.load(Ordering::Acquire)&expected==expected&&STRESS_FAILED.load(Ordering::Acquire)==0 {
            crate::nova_log!(Info,"TEST","SMP allocator stress passed on {} APs",expected.count_ones());
        }else{
            crate::nova_log!(Error,"TEST","SMP allocator stress failed or timed out: done={:#x} failed={:#x}",STRESS_DONE.load(Ordering::Acquire),STRESS_FAILED.load(Ordering::Acquire));
        }
    }
}
