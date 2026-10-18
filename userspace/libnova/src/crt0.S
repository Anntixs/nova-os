.intel_syntax noprefix
.global _start
.extern main
.extern exit
.extern nova_environ
_start:
    // Nova startup stack: argc, argv pointer, envp pointer.
    mov rdi, [rsp]
    mov rsi, [rsp+8]
    mov rdx, [rsp+16]
    mov [rip+nova_environ], rdx
    sub rsp, 8
    call main
    mov edi, eax
    call exit
1:  hlt
    jmp 1b
