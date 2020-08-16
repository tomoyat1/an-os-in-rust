ENTRY(_low_start);
OUTPUT_FORMAT("elf64-x86-64");

SECTIONS
{
	. = 0x00100000;
	.low.text : {
		KEEP( *(.low.text))
	}

	. += 0xFFFFFFFF80000000;
	_kernel_start = .;

	.text ALIGN (4K) : AT (ADDR (.text) - 0xFFFFFFFF80000000) {
		*(.text)
	}
	.data ALIGN(4K) : AT (ADDR (.data) - 0xFFFFFFFF80000000) {
		*(.data)
	}
	.rodata ALIGN(4K) : AT (ADDR (.rodata) - 0xFFFFFFFF80000000) {
	    *(.rodata)
	}
	.bss ALIGN(4K) : AT (ADDR (.bss) - 0xFFFFFFFF80000000) {
		*(COMMON)
		*(.bootstrap_stack)
		*(.bss)
	}
	_kernel_end = .;
}
