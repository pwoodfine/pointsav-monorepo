/*
 * pd_stub_microkit.c — os-totebox Phase H1 protection-domain stub.
 *
 * Microkit-API-compatible version of pd_stub.c.
 * Uses init() entry point and microkit_dbg_puts() instead of raw seL4 svc.
 * All seven os-totebox PDs share this stub source for Phase H1.
 *
 * Compiled per-PD with:
 *   aarch64-linux-gnu-gcc -nostdlib -ffreestanding -g -O3 -Wall -mstrict-align
 *     -I$MICROKIT_SDK/board/qemu_virt_aarch64/debug/include
 *     pd_stub_microkit.c -o build/<pd-name>.o
 *   aarch64-linux-gnu-ld -L$MICROKIT_SDK/board/qemu_virt_aarch64/debug/lib
 *     build/<pd-name>.o -lmicrokit -Tmicrokit.ld -o build/<pd-name>.elf
 *
 * Phase H2: each PD gets a dedicated Rust PdEntry compiled via CompileRustPd.
 */

#include <microkit.h>

/* Injected at link time via -DPD_NAME=watchdog-pd (etc) */
#ifndef PD_NAME
#define PD_NAME "os-totebox-pd"
#endif

void
init(void)
{
    microkit_dbg_puts("\r\n");
    microkit_dbg_puts("=== os-totebox PD: Capability Geometry boot ===\r\n");
    microkit_dbg_puts("PD: " PD_NAME "\r\n");
    microkit_dbg_puts("Protection domain isolated by seL4 capability DAG\r\n");
    microkit_dbg_puts("Phase H1 stub — Phase H2 replaces with Rust PdEntry\r\n");
    microkit_dbg_puts("===============================================\r\n");
    microkit_dbg_puts("\r\n");
}

void
notified(microkit_channel ch)
{
    (void)ch;
}
