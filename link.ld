ENTRY(_low_start);
OUTPUT_FORMAT("elf64-x86-64");

SECTIONS
{
	. = 0x00100000;
	.low.text : {
		KEEP( *(.low.text))
	}

	. = 0xFFFFFFFF80200000;
	_kernel_start = .;

	.text ALIGN (8K) : AT (ADDR (.text) - 0xFFFFFFFF80000000) {
		*(.text)
	}
	.data ALIGN(8K) : AT (ADDR (.data) - 0xFFFFFFFF80000000) {
		*(.data)
	}
	.rodata ALIGN(8K) : AT (ADDR (.rodata) - 0xFFFFFFFF80000000) {
	    *(.rodata)
	}
	.bss ALIGN(8K) : AT (ADDR (.bss) - 0xFFFFFFFF80000000) {
		*(COMMON)
		*(.bss)
		*(.bootstrap_stack)
	}
	_kernel_end = .;

	/DISCARD/ : {
	    *(.dynsym)
	    *(dynamic)
	    *(.dynstr)
	    *(.hash)
	    *(.gnu.hash)
	}
}
