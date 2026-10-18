.global nova_irq_0
nova_irq_0:
    push 32
    jmp nova_irq_common
.global nova_irq_1
nova_irq_1:
    push 33
    jmp nova_irq_common
.global nova_irq_spurious
nova_irq_spurious:
    push 255
    jmp nova_irq_common
nova_irq_common:
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov r12, rsp
    mov rdi, [r12 + 120]
    and rsp, -16
    sub rsp, 512
    fxsave64 [rsp]
    call nova_irq_dispatch
    fxrstor64 [rsp]
    mov rsp, r12
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    add rsp, 8
    iretq
