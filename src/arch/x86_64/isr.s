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
    # Just halt
    hlt
    pusha
    cld
    call general_protection_fault_handler
    popa
    # we need to pop the CPU-pushed error code here.
    addq %rsp, 8
    iretq

# 14 Page Fault
.global page_fault_isr
page_fault_isr:
    pusha
    cld
    mov 88(%rsp), %rdi
    mov %cr2, %rsi
    call page_fault_handler
    jmp isr_exit

.global ps2_keyboard_isr
ps2_keyboard_isr:
    pushq $0x21 /* vector: u64,  88(%rsp) */
    pusha
    cld
    call ps2_keyboard_handler
    jmp isr_exit

.global pit_isr
pit_isr:
    pushq $0x20 /* vector: u64,  88(%rsp) */
    pusha
    cld
    call pit_handler
    jmp isr_exit


.global com0_isr
com0_isr:
    pushq $0x24 /* vector: u64,  88(%rsp) */
    pusha
    cld
    call com0_handler
    jmp isr_exit

.global hpet_isr
hpet_isr:
    pushq $0x20 /* vector: u64,  88(%rsp) */
    pusha
    cld
    call hpet_handler
    jmp isr_exit

.global syscall_isr
syscall_isr:
    pushq $0x80 /* vector: u64,  88(%rsp) */
    pusha
    movq 80(%rsp), %rdi /* syscall_number */
    movq 56(%rsp), %rsi /* %rdi: arg0 */
    movq 48(%rsp), %rdx /* %rsi: arg1 */
    movq 64(%rsp), %rcx /* %rdx: arg2 */
    movq 8(%rsp),  %r8  /* %r10: arg3 */
    movq 24(%rsp), %r9  /* %r8: arg4 */
    cld
    call syscall_handler
    jmp isr_exit

.macro gen_device_isrs from=0, to
    pushq $\from /* vector: u64,  88(%rsp) */
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
    pusha
    cld
    mov 88(%rsp), %rdi
    call device_handler
    jmp isr_exit

.global isr_exit
isr_exit:
    call check_runtime
    popa
    addq $8, %rsp /* pop vector */
    iretq


.global reload_idt
reload_idt:
    lidt 0(%rdi)
    ret
