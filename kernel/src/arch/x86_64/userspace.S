.intel_syntax noprefix
.global nova_enter_user
nova_enter_user:
    // Do not expose kernel or previous-process x87/SSE register contents.
    fninit
    ldmxcsr [rip + nova_default_mxcsr]
    pxor xmm0, xmm0
    pxor xmm1, xmm1
    pxor xmm2, xmm2
    pxor xmm3, xmm3
    pxor xmm4, xmm4
    pxor xmm5, xmm5
    pxor xmm6, xmm6
    pxor xmm7, xmm7
    pxor xmm8, xmm8
    pxor xmm9, xmm9
    pxor xmm10, xmm10
    pxor xmm11, xmm11
    pxor xmm12, xmm12
    pxor xmm13, xmm13
    pxor xmm14, xmm14
    pxor xmm15, xmm15
    mov ax, 0x1b
    mov ds, ax
    mov es, ax
    push 0x1b
    push rsi
    push 0x202
    push 0x23
    push rdi
    iretq

.section .rodata
.align 4
nova_default_mxcsr:
    .long 0x1f80
