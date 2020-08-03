ENTRY(start);
OUTPUT_FORMAT("elf64-x86-64");

EXTERN(_low_start);

SECTIONS
{
	. = 0x00100000;
	.low.text : {
		KEEP( *(.low.text))
	}

	. = 0xFFFFFFFF80100000;
	_kernel_start = .;

	.text ALIGN (4K) : AT (ADDR (.text) - 0xFFFFFFFF80000000) {
		*(.text)
		*(.text.*)
	}
	.data ALIGN(4K) : AT (ADDR (.data) - 0xFFFFFFFF80000000) {
		*(.data)
		*(.data.*)
	}
	.rodata ALIGN(4K) : AT (ADDR (.rodata) - 0xFFFFFFFF80000000) {
	    *(.rodata)
	    *(.rodata.*)
	}
	.bss ALIGN(4K) : AT (ADDR (.bss) - 0xFFFFFFFF80000000) {
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
