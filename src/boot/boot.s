.code32
.section .bss, "aw", @nobits
.align 4096
boot_pml4:
    .skip 0x1000
boot_pdpt:
    .skip 0x1000
boot_pdt:
    .skip 0x1000
boot_pt:
    .skip 0x1000

.section .bootstrap_stack, "aw", @nobits
    stack_bottom:
.skip 16384
    stack_top:

.code64
.section .low.text, "ax"
.global _low_start
.type _low_start, @function
_low_start:
    cli
    # identity-map 0x100000
    # pml4
    movq $(boot_pml4 - 0xFFFFFFFF80000000), %r8
    movq $(boot_pdpt - 0xFFFFFFFF80000000), %r9
    mov $0x0000FFFFFFFFF000, %r12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq %r9, 0(%r8) # offset in pml4 are determined from bits 47:39 in linear address, which is 0


    # pdpt
    movq $(boot_pdpt - 0xFFFFFFFF80000000), %r8
    movq $(boot_pdt - 0xFFFFFFFF80000000), %r9
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq %r9, 0(%r8) # offset in pdpt are determined from bits 38:30 in linear address, which is 0


    # pdt
    movq $(boot_pdt - 0xFFFFFFFF80000000), %r8
    movq $(boot_pt - 0xFFFFFFFF80000000), %r9
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq %r9, 0(%r8) # offset in pdt are determined from bits 29:21 in linear address, which is 0


    # pt
    movq $(boot_pt - 0xFFFFFFFF80000000), %r8
    movq $_low_start, %r9
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq $_low_start, %r11
    shrq $9, %r11 # bits 20:12 of linear address are bits 11:3 of pte
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pdt are determined from bits 20:12 in linear address, which is in %r11
    # end identity map of .low.text


    # map bottom 2 GB of physical memory to 0xFFFFFFFF80000000 and above
    movq $(boot_pml4 - 0xFFFFFFFF80000000), %r8
    movq $(boot_pdpt - 0xFFFFFFFF80000000), %r9
    mov $0x0000FFFFFFFFF000, %r12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq $0xFFFFFFFF80000000, %r11
    movq $0x0000FF8000000000, %rcx #  mask for 47:39
    andq %rcx, %r11
    shrq $36, %r11 # bits 47:39 of linear address are bits 11:3 of pdpte
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pml4 are determined from bits 47:39 in linear address, which is in %r11 mask: 0000FF8000000000

    movq $(boot_pdpt - 0xFFFFFFFF80000000), %r8
    xorq %r9, %r9 # 0 page of phys memory
    movq $0x00000FFFC0000000, %r12 # mask for 47:30
    andq %r12, %r9
    movq $0x0000000000000083, %rdx
    orq %rdx, %r9
    movq $0xFFFFFFFF80000000, %r11
    movq $0x0000007FC0000000, %rcx #  mask for 38:30
    andq %rcx, %r11
    shrq $27, %r11 # bits 38:30 of linear address are bits 11:3 of pte
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pml4 are determined from bits 38:30 in linear address, which is in %r11 mask: 0000FF8000000000

    # set cr3 to boot_pml4
    movq $(boot_pml4 - 0xFFFFFFFF80000000), %r11
    movq %r11, %cr3

    # far jmp to high kernel
    movabs $4f, %rcx
    jmp *%rcx

.section .text
4:
    xorl %edx, %edx
    movl $0xdeadbeef, %edx
    cli
    hlt
    # movl $0, boot_page_directory + 0
    mov $stack_top, %rsp

    # call rust kernel entrypoint
    call start
