/*
 * console_hello.c — os-console Phase H1 seL4 rootserver, hello milestone.
 *
 * Bare-metal AArch64 seL4 rootserver. Prints a greeting via SysDebugPutChar
 * then spins with SysYield. Requires KernelPrinting=ON in the seL4 build.
 *
 * This C file validates the full moonshot-toolkit pipeline end-to-end
 * (TOML spec → kernel + elfloader → QEMU boot → PD runs). A Rust PD using
 * moonshot_sel4_vmm::debug::puts() is the Phase H1 follow-on once
 * moonshot-toolkit gains CompileRustPd support.
 *
 * seL4 AArch64 syscall ABI: x7 = syscall number, x0 = arg0, svc #0.
 * SysDebugPutChar = -9 (requires KernelPrinting=ON).
 * SysYield        = -7
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

static void puts_pd(const char *s)
{
    while (*s)
        sel4_debug_putchar(*s++);
}

static const char banner[] =
    "\r\n"
    "=== os-console seL4 rootserver ===\r\n"
    "Hello from os-console seL4 PD\r\n"
    "moonshot-toolkit Phase H1 milestone\r\n"
    "Geometric Protection: this PD is isolated by seL4 capability tokens\r\n"
    "===================================\r\n"
    "\r\n";

void __attribute__((noreturn)) _start(void)
{
    puts_pd(banner);
    for (;;)
        sel4_yield();
}
