.code64
.section .text
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
    iret

.global reload_idt
reload_idt:
    lidt 0(%rdi)
    ret
