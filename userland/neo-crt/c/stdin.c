/* SPDX-License-Identifier: MIT */
/*
 * OT-61 C3c-1: musl's stdin, for libc++'s std::cin (iostream.cpp).
 *
 * Derived from musl 1.2.5 (src/stdio/stdin.c), Copyright © 2005-2020 Rich
 * Felker, et al., MIT licence (third_party/ot61-crt/vendor/musl/COPYRIGHT).
 * Two changes:
 * - `.lock = 0`, as c/stdout.c: locked always (CP-106).
 * - `.read` is neo-crt's own: the bundle has no standard input, so every
 *   read is end-of-file. musl's __stdio_read would issue readv.
 */
#include "stdio_impl.h"

#undef stdin

static size_t neo_stdin_read(FILE *f, unsigned char *buf, size_t len)
{
	(void)buf; (void)len;
	f->flags |= F_EOF;
	return 0;
}

static unsigned char buf[BUFSIZ+UNGET];
hidden FILE __stdin_FILE = {
	.buf = buf+UNGET,
	.buf_size = sizeof buf-UNGET,
	.fd = 0,
	.flags = F_PERM | F_NOWR,
	.read = neo_stdin_read,
	.seek = __stdio_seek,
	.close = __stdio_close,
	.lock = 0,
};
FILE *const stdin = &__stdin_FILE;
FILE *volatile __stdin_used = &__stdin_FILE;
