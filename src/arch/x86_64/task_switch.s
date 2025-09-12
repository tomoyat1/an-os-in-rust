.global _do_switch
.type _do_switch, @function
_do_switch:
    pushq %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    movq %rsp, 8(%rdi)
    movq 8(%rsi), %rsp

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp

    mov %rdx, %rax

    retq

.global _task_entry
.type _task_entry, @function
_task_entry:
    movq %rbp, %rdi
    movq %rax, %rsi
    call task_entry
