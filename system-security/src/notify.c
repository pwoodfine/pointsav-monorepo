// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

#include <microkit.h>

// Expose the inline function as a standard symbol
void do_notify(microkit_channel ch) {
    microkit_notify(ch);
}
