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
    movq 16(%rsi), %rbx
    movq %rbx, %cr3

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx # -24..-16
    popq %rbp

    # Copy third parameter fo _do_switch() to return value
    mov %rdx, %rax

    retq

.global _task_entry
.type _task_entry, @function
_task_entry:
    movq %rbp, %rdi
    movq %rbx, %rsi
    movq %rax, %rdx
    call task_entry
