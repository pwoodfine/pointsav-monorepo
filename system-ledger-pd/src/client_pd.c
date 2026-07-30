/*
 * client_pd.c — Microkit test PD for system-ledger-pd.
 *
 * Sends a minimal ConsultRequest to system_ledger on channel CONSULT_CH (1)
 * via PPC shared-memory ring, reads the ConsultResponse, and prints the verdict.
 *
 * BOOT SEQUENCE:
 *   1. Write 8-byte test frame (4-byte length prefix + 4-byte minimal payload)
 *      into CAP_REQUEST_MR. The payload is a postcard-encoded ConsultRequest
 *      with all empty fields — system_ledger will return Error{DECODE_CAP}.
 *   2. Issue microkit_ppcall(CONSULT_CH, empty_msginfo).
 *   3. Read ConsultResponse from CAP_RESPONSE_MR.
 *   4. Print verdict via microkit_dbg_puts.
 *
 * EXPECTED OUTPUT ON FIRST BOOT (before any apex is registered):
 *   CLIENT: sending ConsultRequest
 *   CLIENT: response[0] = 0x03 (Error, code=1 DECODE_CAP)
 *   CLIENT: done
 *
 * Wire format — postcard ConsultRequest with all-empty fields:
 *   cap_cbor:     varint(0)      = 0x00
 *   ckpt_wire:    varint(0)      = 0x00
 *   now_unix:     varint(0) u64  = 0x00
 *   witness_cbor: None           = 0x00
 *   Total payload: 4 bytes [0x00, 0x00, 0x00, 0x00]
 *   With 4-byte LE length prefix: [0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
 *
 * Postcard ConsultResponse::Error { code: 1 }:
 *   variant index 3 (varint) = 0x03
 *   code field u8            = 0x01
 *   Total: 2 bytes [0x03, 0x01]
 *   With 4-byte LE prefix:   [0x02, 0x00, 0x00, 0x00, 0x03, 0x01]
 */

#include <microkit.h>
#include <stdint.h>

/* Channel and memory region addresses — must match ledger.system. */
#define CONSULT_CH          1
#define CAP_REQUEST_ADDR    ((volatile uint8_t *)0x4001000)
#define CAP_RESPONSE_ADDR   ((volatile uint8_t *)0x4005000)

/* Minimal ConsultRequest: all empty fields (cap_cbor=[], ckpt_wire=[], now_unix=0, witness=None). */
static const uint8_t TEST_FRAME[] = {
    0x04, 0x00, 0x00, 0x00,   /* length prefix: 4 bytes (LE u32) */
    0x00,                     /* cap_cbor:    varint(0) = empty slice */
    0x00,                     /* ckpt_wire:   varint(0) = empty slice */
    0x00,                     /* now_unix:    varint(0) u64 */
    0x00,                     /* witness_cbor: None (0x00) */
};

static void client_run(void);

static void print(const char *s) {
    microkit_dbg_puts(s);
}

static void print_hex_byte(uint8_t b) {
    static const char hex[] = "0123456789abcdef";
    char buf[5] = "0x";
    buf[2] = hex[(b >> 4) & 0xF];
    buf[3] = hex[b & 0xF];
    buf[4] = '\0';
    microkit_dbg_puts(buf);
}

void init(void) {
    client_run();
}

void notified(microkit_channel ch) {
    (void)ch;
}

microkit_msginfo protected(microkit_channel ch, microkit_msginfo msginfo) {
    (void)ch;
    (void)msginfo;
    return seL4_MessageInfo_new(0, 0, 0, 0);
}

/*
 * The client initiates the test from init() is too early (system_ledger may
 * not be ready). Instead we use notified() triggered by a timer or we just
 * run the test from a passive spin after init(). For simplicity, we abuse the
 * Microkit passive PD pattern: call microkit_ppcall from within init().
 * This works because system_ledger's init() runs at higher priority (254)
 * first, so the server is ready before the client's init() fires.
 */
void client_run(void) {
    uint32_t payload_len;
    uint8_t resp_buf[16];
    int i;

    print("CLIENT: sending ConsultRequest\n");

    /* Write the test frame into CAP_REQUEST_MR. */
    for (i = 0; i < (int)sizeof(TEST_FRAME); i++) {
        CAP_REQUEST_ADDR[i] = TEST_FRAME[i];
    }

    /* Issue PPC — synchronously invokes system_ledger's protected(). */
    microkit_ppcall(CONSULT_CH, seL4_MessageInfo_new(0, 0, 0, 0));

    /* Read 4-byte length prefix from CAP_RESPONSE_MR. */
    payload_len = (uint32_t)CAP_RESPONSE_ADDR[0]
                | ((uint32_t)CAP_RESPONSE_ADDR[1] << 8)
                | ((uint32_t)CAP_RESPONSE_ADDR[2] << 16)
                | ((uint32_t)CAP_RESPONSE_ADDR[3] << 24);

    if (payload_len == 0 || payload_len > sizeof(resp_buf)) {
        print("CLIENT: invalid response length\n");
        return;
    }

    for (i = 0; i < (int)payload_len; i++) {
        resp_buf[i] = CAP_RESPONSE_ADDR[4 + i];
    }

    /* Decode the first byte: postcard ConsultResponse variant index. */
    print("CLIENT: response[0] = ");
    print_hex_byte(resp_buf[0]);

    switch (resp_buf[0]) {
        case 0x00:
            print(" (Allow)\n");
            break;
        case 0x01:
            print(" (Refuse, reason_code=");
            if (payload_len > 1) print_hex_byte(resp_buf[1]);
            print(")\n");
            break;
        case 0x02:
            print(" (ExtendThenAllow)\n");
            break;
        case 0x03:
            print(" (Error, code=");
            if (payload_len > 1) print_hex_byte(resp_buf[1]);
            print(")\n");
            break;
        default:
            print(" (unknown)\n");
            break;
    }

    print("CLIENT: done\n");
}
