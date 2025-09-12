.code32
.section .bss, "aw", @nobits
.align 4096
.global boot_pml4
boot_pml4:
    .skip 0x1000
.global boot_pdpt
boot_pdpt:
    .skip 0x1000
boot_idmap_pdpt:
    .skip 0x1000
boot_heap_pdpt:
    .skip 0x1000
.global boot_pdt
boot_pdt:
    .skip 0x1000
boot_idmap_pdt:
    .skip 0x1000
boot_heap_pdt:
    .skip 0x1000
.global boot_pt
boot_pt:
    .skip 0x1000

# This is unused, and at the wrong address.
# This cannot be static H/W memory mapping depends on the machine we run on.
.global heap_bottom
heap_bottom:

.section .bootstrap_stack, "aw", @nobits
.global boot_stack_top
boot_stack_top:
    .skip 0x2000
.global boot_stack_bottom
boot_stack_bottom:

.code64
.section .low.text, "ax"
.global _low_start
.type _low_start, @function
_low_start:
    mov %rcx, %rdi
    cli
    # identity-map 0x100000
    # pml4
    movq $(boot_pml4 - 0xFFFFFFFF80000000), %r8
    movq $(boot_idmap_pdpt - 0xFFFFFFFF80000000), %r9
    mov $0x0000FFFFFFFFF000, %r12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq %r9, 0(%r8) # offset in pml4 are determined from bits 47:39 in linear address, which is 0


    # pdpt
    movq $(boot_idmap_pdpt - 0xFFFFFFFF80000000), %r8
    movq $(boot_idmap_pdt - 0xFFFFFFFF80000000), %r9
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq %r9, 0(%r8) # offset in pdpt are determined from bits 38:30 in linear address, which is 0


    # pdt
    movq $(boot_idmap_pdt - 0xFFFFFFFF80000000), %r8
    movq $_low_start, %r9
    movq $0x0000FFFFFFE00000, %r12 # mask of 47:21
    andq %r12, %r9
    orq $0x0000000000000083, %r9
    movq %r9, 0(%r8) # offset in pdt are determined from bits 29:21 in linear address, which is 0

    # Identity-map 0xBE4D0000
    # This is for numerous things, such as UEFI allocated heap and ACPI tables.
    # pml4
     movq $(boot_pml4 - 0xFFFFFFFF80000000), %r8 # base of pml4
     movq $(boot_idmap_pdpt - 0xFFFFFFFF80000000), %r9 # phys addr of pdpt; do not reuse pdpt because boot heap is in
                                                 # another 512GiB region than higher-half kernel.
     mov $0x0000FFFFFFFFF000, %r12 # mask for bits 47:12
     andq %r12, %r9
     orq $0x0000000000000003, %r9
     movq $0xBE4D0000, %r11 # linear address
     movq $0x0000FF8000000000, %rcx #  mask for 47:39
     andq %rcx, %r11 # offset in pml4 are determined from bits 47:39 in linear address
     shrq $36, %r11 # bits 47:39 of linear address are bits 11:3 of pdpte
     addq %r11, %r8 # add offset into pml4 to pml4 base addr
     movq %r9, 0(%r8)

    # pdpt
    movq $(boot_idmap_pdpt - 0xFFFFFFFF80000000), %r8
    movq $0xBE4D0000, %r9
    movq $0x0000FFFFC0000000, %r12 # mask for 47:30
    andq %r12, %r9
    orq $0x0000000000000083, %r9 # map 1 Gib, since identity mapping gets teared down right after this.
    movq $0xBE4D0000, %r11 # linear address
    movq $0x0000007FC0000000, %rcx #  mask for 38:30
    andq %rcx, %r11 # offset in pdt are determined from bits 38:30 in linear address
    shrq $27, %r11 # bits 38:30 of linear address are bits 11:3 of pdpte
    addq %r11, %r8 # add offset into pdt to pdpt base addr
    movq %r9, 0(%r8)
    # end identity-map 0xBE4D0000

    # Identity-map 0xC0000000
    # This is for numerous things, such as UEFI allocated heap and ACPI tables.
    # pml4
     movq $(boot_pml4 - 0xFFFFFFFF80000000), %r8 # base of pml4
     movq $(boot_idmap_pdpt - 0xFFFFFFFF80000000), %r9 # phys addr of pdpt; do not reuse pdpt because boot heap is in
                                                 # another 512GiB region than higher-half kernel.
     mov $0x0000FFFFFFFFF000, %r12 # mask for bits 47:12
     andq %r12, %r9
     orq $0x0000000000000003, %r9
     movq $0xC0000000, %r11 # linear address
     movq $0x0000FF8000000000, %rcx #  mask for 47:39
     andq %rcx, %r11 # offset in pml4 are determined from bits 47:39 in linear address
     shrq $36, %r11 # bits 47:39 of linear address are bits 11:3 of pdpte
     addq %r11, %r8 # add offset into pml4 to pml4 base addr
     movq %r9, 0(%r8)

    # pdpt
    movq $(boot_idmap_pdpt - 0xFFFFFFFF80000000), %r8
    movq $0xC0000000, %r9
    movq $0x0000FFFFC0000000, %r12 # mask for 47:30
    andq %r12, %r9
    orq $0x0000000000000083, %r9 # map 1 Gib, since identity mapping gets teared down right after this.
    movq $0xC0000000, %r11 # linear address
    movq $0x0000007FC0000000, %rcx #  mask for 38:30
    andq %rcx, %r11 # offset in pdt are determined from bits 38:30 in linear address
    shrq $27, %r11 # bits 38:30 of linear address are bits 11:3 of pdpte
    addq %r11, %r8 # add offset into pdt to pdpt base addr
    movq %r9, 0(%r8)
    # end identity-map 0xBE4D0000

    # Map bottom 1 GB of physical memory to 0xFFFFFFFF80000000
    # The remaining 1 GiB will be handled in rust code.
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
    mov $boot_stack_bottom, %rsp

    # Call rust kernel entrypoint.
    # First parameter for start(boot_data: *const bootloader::boot_types::BootData); should be address to BootData struct.
    # This was conveniently passed as first argument of _low_start by the bootloader.
    addq $0xFFFFFFFF80000000, %rdi
    call start
