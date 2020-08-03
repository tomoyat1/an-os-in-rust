.section .bss, "aw", @nobits
.align 4096
boot_page_directory:
    .skip 4096
boot_page_table_1:
    .skip 4096

.section .low.text
.global _low_start
.type _start @function
_low_start:
    movl $(boot_page_table_1 - 0xc0000000), %edi
    movl $0, %esi
