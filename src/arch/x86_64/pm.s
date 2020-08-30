.code64
.section .text
.global reload_cs
reload_cs:
    lgdt 0(%rdx)
    movq 0x10, %r8
    movq %r8, %ds
    movq %r8, %es
    movq %r8, %fs

    popq %r9 # pop ret addr
    movq $0x8, %r10 # push segment selector
    pushq %r10 # push segment selector
    pushq %r9 # push ret addr

    lretq # long return to new segment
