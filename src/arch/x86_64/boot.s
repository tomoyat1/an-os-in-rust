.code32
.section .boot_paging_structures, "aw", @nobits
.global boot_paging_structures_start
boot_paging_structures_start:
.global boot_pml4
boot_pml4:
    .skip 0x1000
.global boot_pdpt
boot_pdpt:
    .skip 0x1000
.global boot_pdt
boot_pdt:
    .skip 0x1000
boot_idmap_pdpt:
    .skip 0x1000
boot_idmap_pdt:
    .skip 0x1000
.global boot_pt
boot_pt:
    .skip 0x1000
.global boot_paging_structures_end
boot_paging_structures_end:

.align 0x1000
.section .bootstrap_stack, "aw", @nobits
.global boot_stack_top
boot_stack_top:
    .skip 0x2000
.global boot_stack_bottom
boot_stack_bottom:

.section .bss, "aw", @nobits

.code64
.section .low.text, "ax"
.equ KERNEL_BASE, 0xFFFF800000000000
.global _low_start
.type _low_start, @function
_low_start:
    mov %rcx, %rdi
    cli
    # identity-map [0x0, 0x1FFFFF]
    # pml4
    movabsq $(boot_pml4 - KERNEL_BASE), %r8
    movabsq $(boot_idmap_pdpt - KERNEL_BASE), %r9
    mov $0x0000FFFFFFFFF000, %r12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq %r9, 0(%r8) # offset in pml4 are determined from bits 47:39 in linear address, which is 0


    # pdpt
    movabsq $(boot_idmap_pdpt - KERNEL_BASE), %r8
    movabsq $(boot_idmap_pdt - KERNEL_BASE), %r9
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq %r9, 0(%r8) # offset in pdpt are determined from bits 38:30 in linear address, which is 0


    # pdt
    movabsq $(boot_idmap_pdt - KERNEL_BASE), %r8
    movabsq $_low_start, %r9
    movabsq $0x0000FFFFFFE00000, %r12 # mask of 47:21
    andq %r12, %r9
    orq $0x0000000000000083, %r9
    movq %r9, 0(%r8) # offset in pdt are determined from bits 29:21 in linear address, which is 0

    # Identity-map 0xBE4D0000
    # This is for numerous things, such as UEFI allocated heap and ACPI tables.
    # TODO: Map this in Rust code, at offset PML4[510]
    # pml4
    movabsq $(boot_pml4 - KERNEL_BASE), %r8 # base of pml4
    movabsq $(boot_idmap_pdpt - KERNEL_BASE), %r9 # phys addr of pdpt; do not reuse pdpt because boot heap is in
                                                  # another 512GiB region than higher-half kernel.
    movabsq $0x0000FFFFFFFFF000, %r12 # mask for bits 47:12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq $0xBE4D0000, %r11 # linear address
    movabsq $0x0000FF8000000000, %rcx #  mask for 47:39
    andq %rcx, %r11 # offset in pml4 are determined from bits 47:39 in linear address
    shrq $36, %r11 # bits 47:39 of linear address are bits 11:3 of pdpte
    addq %r11, %r8 # add offset into pml4 to pml4 base addr
    movq %r9, 0(%r8)

    # pdpt
    movabsq $(boot_idmap_pdpt - KERNEL_BASE), %r8
    movq $0xBE4D0000, %r9
    movabsq $0x0000FFFFC0000000, %r12 # mask for 47:30
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
    # TODO: Map this in Rust code, at offset 0xffff_ff00_0000_0000 (PML4[510])
    # pml4
    movabsq $(boot_pml4 - KERNEL_BASE), %r8 # base of pml4
    movabsq $(boot_idmap_pdpt - KERNEL_BASE), %r9 # phys addr of pdpt; do not reuse pdpt because boot heap is in
                                                # another 512GiB region than higher-half kernel.
    movq $0x0000FFFFFFFFF000, %r12 # mask for bits 47:12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq $0xC0000000, %r11 # linear address
    movq $0x0000FF8000000000, %rcx #  mask for 47:39
    andq %rcx, %r11 # offset in pml4 are determined from bits 47:39 in linear address
    shrq $36, %r11 # bits 47:39 of linear address are bits 11:3 of pdpte
    addq %r11, %r8 # add offset into pml4 to pml4 base addr
    movq %r9, 0(%r8)

    # pdpt
    movabsq $(boot_idmap_pdpt - KERNEL_BASE), %r8
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
    # end identity-map 0xC0000000

    # Map bottom 6 MiB of physical memory to KERNEL_BASE
    movabsq $(boot_pml4 - KERNEL_BASE), %r8
    movabsq $(boot_pdpt - KERNEL_BASE), %r9
    mov $0x0000FFFFFFFFF000, %r12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq $KERNEL_BASE, %r11
    movq $0x0000FF8000000000, %rcx #  mask for 47:39
    andq %rcx, %r11
    shrq $36, %r11 # bits 47:39 of linear address are bits 11:3 of pml4e address (offset)
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pml4 are determined from bits 47:39 in linear address, which is in %r11 mask: 0000FF8000000000

    movabsq $(boot_pdpt - KERNEL_BASE), %r8
    movabsq $(boot_pdt - KERNEL_BASE), %r9
    mov $0x0000FFFFFFFFF000, %r12 # mask for 47:12
    andq %r12, %r9
    orq $0x0000000000000003, %r9
    movq $KERNEL_BASE, %r11
    movq $0x0000007FC0000000, %rcx #  mask for 38:30
    andq %rcx, %r11
    shrq $27, %r11 # bits 38:30 of linear address are bits 11:3 of pdpte
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pdpt are determined from bits 38:30 in linear address

    movabsq $(boot_pdt - KERNEL_BASE), %r8
    xorq %r9, %r9 # 0 page of phys memory
    movq $0x0000FFFFFFE00000, %r12 # mask for 47:21
    andq %r12, %r9
    movq $0x0000000000000083, %rdx
    orq %rdx, %r9
    movq $KERNEL_BASE, %r11
    movq $0x000000003FE00000, %rcx #  mask for 29:21
    andq %rcx, %r11
    shrq $18, %r11 # bits 29:20 of linear address are bits 11:3 of pde
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pdt are determined from bits 29:21 in linear address

    movabsq $(boot_pdt - KERNEL_BASE), %r8
    movq $0x0000000000200000, %r9 # 2MiB page at phys 0x200000
    movq $0x0000FFFFFFE00000, %r12 # mask for 47:21
    andq %r12, %r9
    movq $0x0000000000000083, %rdx
    orq %rdx, %r9
    movq $KERNEL_BASE, %r11
    addq $0x200000, %r11 # 2MiB offset
    movq $0x000000003FE00000, %rcx #  mask for 29:21
    andq %rcx, %r11
    shrq $18, %r11 # bits 38:30 of linear address are bits 11:3 of pde
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pdt are determined from bits 29:21 in linear address

    movabsq $(boot_pdt - KERNEL_BASE), %r8
    movq $0x0000000000400000, %r9 # 2MiB page at phys 0x400000
    movq $0x0000FFFFFFE00000, %r12 # mask for 47:21
    andq %r12, %r9
    movq $0x0000000000000083, %rdx
    orq %rdx, %r9
    movq $KERNEL_BASE, %r11
    addq $0x400000, %r11 # 4MiB offset
    movq $0x000000003FE00000, %rcx #  mask for 29:21
    andq %rcx, %r11
    shrq $18, %r11 # bits 38:30 of linear address are bits 11:3 of pde
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pdt are determined from bits 29:21 in linear address

    # Map next 2 Mib to KERNEL_BASE + 512MiB, for initial heap
    movabsq $(boot_pdt - KERNEL_BASE), %r8
    movq $0x0000000000600000, %r9 #  2MiB page at phys 0x600000
    movq $0x0000FFFFFFE00000, %r12 # mask for 47:21
    andq %r12, %r9
    movq $0x0000000000000083, %rdx
    orq %rdx, %r9
    movq $KERNEL_BASE, %r11
    addq $0x20000000, %r11 # 512MiB offset
    movq $0x000000003FE00000, %rcx #  mask for 29:21
    andq %rcx, %r11
    shrq $18, %r11 # bits 29:21 of linear address are bits 11:3 of pde
    addq %r11, %r8
    movq %r9, 0(%r8) # offset in pdt are determined from bits 29:21 in linear address

    # set cr3 to boot_pml4
    movabsq $(boot_pml4 - KERNEL_BASE), %r11
    movq %r11, %cr3

    # far jmp to high kernel
    movabs $4f, %rcx
    jmp *%rcx

.section .text
4:
    movabsq $boot_stack_bottom, %rsp

    # Call rust kernel entrypoint.
    # First parameter for start(boot_data: *const bootloader::boot_types::BootData); should be address to BootData struct.
    # This was conveniently passed as first argument of _low_start by the bootloader.
    movabs $KERNEL_BASE, %rcx
    addq %rcx, %rdi
    call start
