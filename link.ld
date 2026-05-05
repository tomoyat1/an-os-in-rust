ENTRY(_low_start);
OUTPUT_FORMAT("elf64-x86-64");

SECTIONS
{
	. = 0x00100000;
	.low.text : {
		KEEP( *(.low.text))
	}

	. += 0xFFFF800000000000;
	_kernel_start = .;

	.text ALIGN (4K) : AT (ADDR (.text) - 0xFFFF800000000000) {
		*(.text)
	}
	.data ALIGN(4K) : AT (ADDR (.data) - 0xFFFF800000000000) {
		*(.data)
	}
	.rodata ALIGN(4K) : AT (ADDR (.rodata) - 0xFFFF800000000000) {
	    *(.rodata)
	    *(.rodata.*)
	}
	.bss ALIGN(4K) : AT (ADDR (.bss) - 0xFFFF800000000000) {
		*(COMMON)
		. = ALIGN(0x2000);
		*(.bootstrap_stack)
		. = ALIGN(0x200000);
		*(.boot_paging_structures)
		*(.bss)
	}
	_kernel_end = .;
}
