ENTRY(start);
OUTPUT_FORMAT("elf64-x86-64");

EXTERN(_low_start);

SECTIONS
{
	. = 0x00100000;
	.low.text : {
		KEEP( *(.low.text))
	}

	. = 0xc0100000;
	_kernel_start = .;

	.text ALIGN (4K) : AT (ADDR (.text) - 0xc0000000) {
		*(.text)
		*(.text.*)
	}
	.data ALIGN(4K) : AT (ADDR (.data) - 0xc0000000) {
		*(.data)
		*(.data.*)
	}
	.rodata ALIGN(4K) : AT (ADDR (.rodata) - 0xc0000000) {
	    *(.rodata)
	    *(.rodata.*)
	}
	.bss ALIGN(4K) : AT (ADDR (.bss) - 0xc0000000) {
		*(COMMON)
		KEEP( *(.bss) )
		KEEP( *(.bss.*) )
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
