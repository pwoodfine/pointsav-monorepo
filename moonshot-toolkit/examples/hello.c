/*
 * hello.c — Phase 1C minimal bare-metal AArch64 seL4 protection domain.
 *
 * Compiled with:
 *   aarch64-linux-gnu-gcc -nostdlib -nostartfiles -ffreestanding
 *     -march=armv8-a -mgeneral-regs-only hello.c -o build/hello.elf
 *
 * No libc, no crt0. _start is the ELF entry point.
 *
 * seL4 syscall convention (AArch64): x7 = syscall number, x0 = arg0,
 * svc #0. SysDebugPutChar (-9) requires KernelPrinting=ON.
 */

typedef unsigned long word_t;

static inline void sel4_debug_putchar(char c)
{
    register word_t scno asm("x7") = (word_t)-9; /* SysDebugPutChar */
    register word_t arg0 asm("x0") = (word_t)(unsigned char)c;
    asm volatile(
        "svc #0"
        : "+r"(arg0)
        : "r"(scno)
        : "x1", "x2", "x3", "x4", "x5", "x6", "memory"
    );
}

static inline void sel4_yield(void)
{
    register word_t scno asm("x7") = (word_t)-7; /* SysYield */
    asm volatile(
        "svc #0"
        :
        : "r"(scno)
        : "x0", "x1", "x2", "x3", "x4", "x5", "x6", "memory"
    );
}

static const char message[] = "hello from seL4 rootserver\r\n";

void __attribute__((noreturn)) _start(void)
{
    for (int i = 0; message[i] != '\0'; i++) {
        sel4_debug_putchar(message[i]);
    }
    for (;;) {
        sel4_yield();
    }
}
