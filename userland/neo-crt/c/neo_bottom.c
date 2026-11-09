/* SPDX-License-Identifier: MIT */
/*
 * OT-61 C3a: neo-crt's C-side bottom half, under musl's stdio and libc.
 *
 * Derived from musl 1.2.5 (src/stdio/__stdio_write.c, __stdout_write.c,
 * __stdio_close.c, __stdio_seek.c, src/internal/libc.c, src/exit/exit.c),
 * Copyright © 2005-2020 Rich Felker, et al., MIT licence
 * (third_party/ot61-crt/vendor/musl/COPYRIGHT). Changes: writes go to
 * __neo_crt_write (Rust, over std's stdout/stderr) instead of SYS_writev;
 * there are no fds to close or seek; exit flushes the two standard streams
 * and ends the process through std.
 */
#include "stdio_impl.h"
#include "locale_impl.h"
#include "pthread_impl.h"
#include "libc.h"
#include <sys/mman.h>
#include <time.h>

/* Rust (src/lib.rs). */
long __neo_crt_write(int fd, const unsigned char *buf, size_t len);
_Noreturn void __neo_crt_exit(int code);
_Noreturn void abort(void);

struct __libc __libc;

/* The Itanium C++ ABI's handle for this module; __cxa_atexit registrations
 * name it. A static program is one module. */
void *__dso_handle = &__dso_handle;
size_t __hwcap;
char *__progname = 0, *__progname_full = 0;

/* One read-only record for every thread: neo-crt has only the global
 * locale (override/pthread_impl.h). */
static struct pthread neo_crt_thread = { .locale = &__libc.global_locale };

struct pthread *__neo_crt_self(void)
{
	return &neo_crt_thread;
}

static size_t write_all(int fd, const unsigned char *p, size_t n)
{
	size_t done = 0;
	while (done < n) {
		long r = __neo_crt_write(fd, p + done, n - done);
		if (r <= 0) break;
		done += (size_t)r;
	}
	return done;
}

/* musl's __stdio_write contract: write the buffered bytes, then buf; on
 * success reset the write window, on failure set F_ERR and report how
 * much of buf went out. */
size_t __stdio_write(FILE *f, const unsigned char *buf, size_t len)
{
	size_t pending = f->wpos - f->wbase;
	if (pending && write_all(f->fd, f->wbase, pending) != pending) {
		f->wpos = f->wbase = f->wend = 0;
		f->flags |= F_ERR;
		return 0;
	}
	size_t n = write_all(f->fd, buf, len);
	if (n != len) {
		f->wpos = f->wbase = f->wend = 0;
		f->flags |= F_ERR;
		return n;
	}
	f->wend = f->buf + f->buf_size;
	f->wpos = f->wbase = f->buf;
	return len;
}

/* musl's stdout.c/stderr.c reference this so that exit flushes them; our
 * exit (below) flushes both directly. */
void __stdio_exit_needed(void)
{
}

/* No terminal query here: stdout stays line-buffered. */
size_t __stdout_write(FILE *f, const unsigned char *buf, size_t len)
{
	f->write = __stdio_write;
	return __stdio_write(f, buf, len);
}

/* The standard streams are never closed or seeked; nothing else has an fd. */
int __stdio_close(FILE *f)
{
	(void)f;
	return 0;
}

off_t __stdio_seek(FILE *f, off_t off, int whence)
{
	(void)f; (void)off; (void)whence;
	errno = ESPIPE;
	return -1;
}

/* stdout and stderr are locked always (c/stdout.c, c/stderr.c: lock = 0),
 * and their __lockfile/__unlockfile are Rust (src/lib.rs, CP-106). The
 * on-stack FILEs of vsnprintf and vsscanf keep lock = -1; there is no
 * fopen/fdopen. musl's futex wake is used only by its own __lockfile and
 * pthread code, neither compiled here, so reaching it is a bug to stop on. */
void __wake(volatile void *addr, int cnt, int priv)
{
	(void)addr; (void)cnt; (void)priv;
	static const char msg[] = "neo-crt: musl's futex wake was reached (nothing should use it)\n";
	__neo_crt_write(2, (const unsigned char *)msg, sizeof msg - 1);
	abort();
}

/* musl's locale code allocates through these internal names so a program's
 * own malloc can't interpose; here there is only neo-crt's (src/lib.rs). */
void *__libc_malloc(size_t n)
{
	return malloc(n);
}

void __libc_free(void *p)
{
	free(p);
}

/* locale_map.c unmaps a locale file it rejected; __map_file never maps one,
 * so this is never called. */
int __munmap(void *addr, size_t len)
{
	(void)addr; (void)len;
	return 0;
}

/* locale_map.c maps a locale file only when MUSL_LOCPATH is set, and getenv
 * always returns NULL here, so this is never called. It exists so the
 * reference links; if it were reached, no file is mapped. */
const unsigned char *__map_file(const char *path, size_t *size)
{
	(void)path; (void)size;
	return 0;
}

/* strftime's %Z. The bundle has no time zones (no tz database, no TZ), so
 * every struct tm is UTC. musl's own (src/time/__tz.c) reads TZ and
 * /etc/localtime. */
const char *__tm_to_tzname(const struct tm *tm)
{
	(void)tm;
	return "UTC";
}

_Noreturn void exit(int code)
{
	fflush(stdout);
	fflush(stderr);
	__neo_crt_exit(code);
}

_Noreturn void _Exit(int code)
{
	__neo_crt_exit(code);
}
