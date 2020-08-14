.code32
.section .bss, "aw", @nobits
.align 8192
boot_page_directory:
    .skip 8192
boot_page_table_1:
    .skip 8192

.section .bootstrap_stack, "aw", @nobits
    stack_bottom:
.skip 16384
    stack_top:

.code64
.section .low.text, "ax"
.global _low_start
.type _low_start, @function
_low_start:
    movq $0xdeadbeef, %rax
    cli
    hlt

    movl $(boot_page_table_1 - 0xFFFFFFFF80000000), %edi
    movl $0, %esi
    # TODO: initialize page table and whatnot

    movabs $4f, %rcx
    jmp *%rcx

.section .text
4:
    movl $0, boot_page_directory + 0
    mov $stack_top, %rsp

    # call rust kernel entrypoint
    call start
