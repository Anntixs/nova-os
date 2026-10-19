.intel_syntax noprefix
.global nova_syscall_entry
nova_syscall_entry:
    // SYSCALL leaves the user RSP active; FMASK has cleared IF.
    // SWAPGS installs the trusted kernel GS base before looking up the stack.
    swapgs
    mov gs:[{user_rsp_off}], rsp
    mov rsp, gs:[{kernel_stack_off}]
    push qword ptr gs:[{user_rsp_off}]
    push rcx
    push r11
    push rdi
    push rsi
    push rdx
    push r10
    push r8
    push r9
    sub rsp, 8
    // RSP is 16-byte aligned. Preserve user x87/SSE state across Rust code.
    sub rsp, 512
    fxsave64 [rsp]
    mov rcx, rdx
    mov rdx, rsi
    mov rsi, rdi
    mov rdi, rax
    call nova_syscall_dispatch
    fxrstor64 [rsp]
    add rsp, 512
    add rsp, 8
    pop r9
    pop r8
    pop r10
    pop rdx
    pop rsi
    pop rdi
    pop r11
    pop rcx
    mov rsp, [rsp]
    swapgs
    sysretq
