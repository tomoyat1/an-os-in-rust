.macro pusha
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
.endm

.macro popa
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
.endm


.code64
.section .text

# 13 General Protection Fault
.global general_protection_fault_isr
general_protection_fault_isr:
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call general_protection_fault_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    # we need to pop the CPU-pushed error code here.
    addq %rsp, 8
    iretq

# 14 Page Fault
.global page_fault_isr
page_fault_isr:
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call page_fault_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    iretq

.global ps2_keyboard_isr
ps2_keyboard_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call ps2_keyboard_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.global pit_isr
pit_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call pit_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.global com0_isr
com0_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    call com0_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.global rtl8139_isr
rtl8139_isr:
    cli
    pushq %rax
    pushq %rcx
    pushq %rdx
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rsp
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    cld
    movq $0x26, %rdi # vector: u64
    call rtl8139_handler
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsp
    popq %rbp
    popq %rsi
    popq %rdi
    popq %rdx
    popq %rcx
    popq %rax
    sti
    iretq

.macro gen_device_isrs from=0, to
    pushq $\from /* vector: u64,  96(%rsp) */
    jmp device_isr_common
    .if \to-\from
    gen_device_isrs "(\from+1)", \to
    .endif
.endm

.global device_isr_entries
device_isr_entries:
gen_device_isrs to=63
.fill 128, 1, 0xcc /* To ensure near jump is used */

device_isr_common:
    cli
    pusha
    cld
    mov 88(%rsp), %rdi
    call device_handler
    popa
    addq $8, %rsp /* pop vector */
    sti
    iretq


.global reload_idt
reload_idt:
    lidt 0(%rdi)
    ret
