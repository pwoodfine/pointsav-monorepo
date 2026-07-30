#include <microkit.h>

// Expose the inline function as a standard symbol
void do_notify(microkit_channel ch) {
    microkit_notify(ch);
}
