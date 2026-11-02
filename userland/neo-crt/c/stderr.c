/* SPDX-License-Identifier: MIT */
/*
 * OT-61 C3a: musl's stderr, with its lock always on.
 *
 * Derived from musl 1.2.5 (src/stdio/stderr.c), Copyright © 2005-2020 Rich
 * Felker, et al., MIT licence (third_party/ot61-crt/vendor/musl/COPYRIGHT).
 * The one change: `.lock = 0` instead of `-1`. musl leaves the standard
 * streams unlocked until pthread_create turns locking on, which neo-crt
 * doesn't compile; any std thread can call into C, so two threads writing
 * at once could overrun the static buffer in __fwritex. With the lock on,
 * every FLOCK takes neo-crt's __lockfile (src/lib.rs). CP-106.
 */
#include "stdio_impl.h"

#undef stderr

static unsigned char buf[UNGET];
hidden FILE __stderr_FILE = {
	.buf = buf+UNGET,
	.buf_size = 0,
	.fd = 2,
	.flags = F_PERM | F_NORD,
	.lbf = -1,
	.write = __stdio_write,
	.seek = __stdio_seek,
	.close = __stdio_close,
	.lock = 0,
};
FILE *const stderr = &__stderr_FILE;
FILE *volatile __stderr_used = &__stderr_FILE;
