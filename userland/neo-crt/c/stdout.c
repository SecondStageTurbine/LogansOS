/* SPDX-License-Identifier: MIT */
/*
 * OT-61 C3a: musl's stdout, with its lock always on.
 *
 * Derived from musl 1.2.5 (src/stdio/stdout.c), Copyright © 2005-2020 Rich
 * Felker, et al., MIT licence (third_party/ot61-crt/vendor/musl/COPYRIGHT).
 * The one change: `.lock = 0` instead of `-1`. musl leaves the standard
 * streams unlocked until pthread_create turns locking on, which neo-crt
 * doesn't compile; any std thread can call into C, so two threads writing
 * at once could overrun the static buffer in __fwritex. With the lock on,
 * every FLOCK takes neo-crt's __lockfile (src/lib.rs). CP-100.
 */
#include "stdio_impl.h"

#undef stdout

static unsigned char buf[BUFSIZ+UNGET];
hidden FILE __stdout_FILE = {
	.buf = buf+UNGET,
	.buf_size = sizeof buf-UNGET,
	.fd = 1,
	.flags = F_PERM | F_NORD,
	.lbf = '\n',
	.write = __stdout_write,
	.seek = __stdio_seek,
	.close = __stdio_close,
	.lock = 0,
};
FILE *const stdout = &__stdout_FILE;
FILE *volatile __stdout_used = &__stdout_FILE;
