.global nova_isr_0
nova_isr_0:
    push 0
    push 0
    jmp nova_isr_common
.global nova_isr_1
nova_isr_1:
    push 0
    push 1
    jmp nova_isr_common
.global nova_isr_2
nova_isr_2:
    push 0
    push 2
    jmp nova_isr_common
.global nova_isr_3
nova_isr_3:
    push 0
    push 3
    jmp nova_isr_common
.global nova_isr_4
nova_isr_4:
    push 0
    push 4
    jmp nova_isr_common
.global nova_isr_5
nova_isr_5:
    push 0
    push 5
    jmp nova_isr_common
.global nova_isr_6
nova_isr_6:
    push 0
    push 6
    jmp nova_isr_common
.global nova_isr_7
nova_isr_7:
    push 0
    push 7
    jmp nova_isr_common
.global nova_isr_8
nova_isr_8:
    push 8
    jmp nova_isr_common
.global nova_isr_9
nova_isr_9:
    push 0
    push 9
    jmp nova_isr_common
.global nova_isr_10
nova_isr_10:
    push 10
    jmp nova_isr_common
.global nova_isr_11
nova_isr_11:
    push 11
    jmp nova_isr_common
.global nova_isr_12
nova_isr_12:
    push 12
    jmp nova_isr_common
.global nova_isr_13
nova_isr_13:
    push 13
    jmp nova_isr_common
.global nova_isr_14
nova_isr_14:
    push 14
    jmp nova_isr_common
.global nova_isr_15
nova_isr_15:
    push 0
    push 15
    jmp nova_isr_common
.global nova_isr_16
nova_isr_16:
    push 0
    push 16
    jmp nova_isr_common
.global nova_isr_17
nova_isr_17:
    push 17
    jmp nova_isr_common
.global nova_isr_18
nova_isr_18:
    push 0
    push 18
    jmp nova_isr_common
.global nova_isr_19
nova_isr_19:
    push 0
    push 19
    jmp nova_isr_common
.global nova_isr_20
nova_isr_20:
    push 0
    push 20
    jmp nova_isr_common
.global nova_isr_21
nova_isr_21:
    push 21
    jmp nova_isr_common
.global nova_isr_22
nova_isr_22:
    push 0
    push 22
    jmp nova_isr_common
.global nova_isr_23
nova_isr_23:
    push 0
    push 23
    jmp nova_isr_common
.global nova_isr_24
nova_isr_24:
    push 0
    push 24
    jmp nova_isr_common
.global nova_isr_25
nova_isr_25:
    push 0
    push 25
    jmp nova_isr_common
.global nova_isr_26
nova_isr_26:
    push 0
    push 26
    jmp nova_isr_common
.global nova_isr_27
nova_isr_27:
    push 0
    push 27
    jmp nova_isr_common
.global nova_isr_28
nova_isr_28:
    push 0
    push 28
    jmp nova_isr_common
.global nova_isr_29
nova_isr_29:
    push 29
    jmp nova_isr_common
.global nova_isr_30
nova_isr_30:
    push 30
    jmp nova_isr_common
.global nova_isr_31
nova_isr_31:
    push 0
    push 31
    jmp nova_isr_common
.global nova_isr_unknown
nova_isr_unknown:
    push 0
    push 255
    jmp nova_isr_common
nova_isr_common:
    cld
    mov rdi, rsp
    and rsp, -16
    call nova_exception_dispatch
1:  cli
    hlt
    jmp 1b
