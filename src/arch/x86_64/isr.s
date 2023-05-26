.code64
.section .text

# 13 General Protection Fault
.global general_protection_fault_isr
general_protection_fault_isr:
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call general_protection_fault_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    # we need to pop the CPU-pushed error code here.
    addq %rsp, 8
    iretq

# 14 Page Fault
.global page_fault_isr
page_fault_isr:
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call page_fault_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    iretq

.global ps2_keyboard_isr
ps2_keyboard_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call ps2_keyboard_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.global pit_isr
pit_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call pit_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.global com0_isr
com0_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call com0_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.global rtl8139_isr
rtl8139_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    movq $0x26, %rdi # vector: u64
    call rtl8139_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.global reload_idt
reload_idt:
    lidt 0(%rdi)
    ret
