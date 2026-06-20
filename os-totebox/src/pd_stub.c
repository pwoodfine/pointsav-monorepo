/*
 * pd_stub.c — os-totebox Phase H1 protection-domain stub.
 *
 * Bare-metal AArch64 rootserver. Prints the PD boot banner via
 * SysDebugPutChar, then spins with SysYield.
 *
 * Compiled by moonshot-toolkit CompilePd with:
 *   aarch64-linux-gnu-gcc -nostdlib -nostartfiles -ffreestanding
 *     -march=armv8-a -mgeneral-regs-only pd_stub.c -o build/<pd-name>.elf
 *
 * All seven os-totebox PDs share this stub source for Phase H1.
 * Phase H2 replaces each stub with a dedicated Rust binary compiled via
 * moonshot-toolkit CompileRustPd, implementing PdEntry from
 * moonshot-sel4-vmm/src/pd.rs.
 *
 * seL4 AArch64 syscall ABI: x7 = syscall number, x0 = arg0, svc #0.
 *   SysDebugPutChar = -9  (requires KernelPrinting=ON)
 *   SysYield        = -7
 */

typedef unsigned long word_t;

static inline void sel4_debug_putchar(char c)
{
    register word_t scno asm("x7") = (word_t)-9;
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
    register word_t scno asm("x7") = (word_t)-7;
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
    "=== os-totebox PD: Capability Geometry boot ===\r\n"
    "Protection domain isolated by seL4 capability DAG\r\n"
    "Phase H1 stub — Phase H2 replaces with Rust PdEntry\r\n"
    "===============================================\r\n"
    "\r\n";

void __attribute__((noreturn)) _start(void)
{
    puts_pd(banner);
    for (;;)
        sel4_yield();
}
