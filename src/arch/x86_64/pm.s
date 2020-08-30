.code64
.section .text
.global reload_gdt
reload_gdt:
    lgdt 0(%rdi)
    movq $0x10, %r8
    movq %r8, %ds
    movq %r8, %es
    movq %r8, %fs

    pushq $0x8
    movabsq $reload_cs, %r9
    pushq %r9
    lretq

reload_cs:
    ret
