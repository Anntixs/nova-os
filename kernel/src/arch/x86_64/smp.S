.section .rodata.ap_trampoline,"a"
.global nova_ap_start
.global nova_ap_end
.global nova_ap_gdtr_base
.global nova_ap_jump32_target
.global nova_ap_jump64_target
.global nova_ap_base_imm
.global nova_ap_cr3
.global nova_ap_stack
.global nova_ap_logical
.global nova_ap_entry
.global nova_ap_pm32
.global nova_ap_lm64
.global nova_ap_gdt

.code16
nova_ap_start:
    cli
    cld
    mov ax, cs
    mov ds, ax
    lgdt [0x100]
    mov eax, cr0
    and eax, 0xfffffffb
    or eax, 3
    mov cr0, eax
    .byte 0x66, 0xea
nova_ap_jump32_target:
    .long 0
    .word 0x08

.code32
nova_ap_pm32:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    .byte 0xbb
nova_ap_base_imm:
    .long 0
    mov eax, cr4
    or eax, 0x620
    mov cr4, eax
    mov eax, DWORD PTR [ebx + 0x130]
    mov cr3, eax
    mov ecx, 0xc0000080
    rdmsr
    or eax, 0x100
    wrmsr
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax
    .byte 0xea
nova_ap_jump64_target:
    .long 0
    .word 0x18

.code64
nova_ap_lm64:
    mov rsp, QWORD PTR [rip + nova_ap_stack]
    and rsp, -16
    sub rsp, 8
    mov QWORD PTR [rsp], 0
    mov rdi, QWORD PTR [rip + nova_ap_logical]
    mov rax, QWORD PTR [rip + nova_ap_entry]
    jmp rax

.org 0x100
nova_ap_gdtr:
    .word 31
nova_ap_gdtr_base:
    .long 0
nova_ap_gdt:
    .quad 0
    .quad 0x00cf9a000000ffff
    .quad 0x00cf92000000ffff
    .quad 0x00af9a000000ffff
.org 0x130
nova_ap_cr3: .long 0
nova_ap_stack: .quad 0
nova_ap_logical: .quad 0
nova_ap_entry: .quad 0
nova_ap_end:
