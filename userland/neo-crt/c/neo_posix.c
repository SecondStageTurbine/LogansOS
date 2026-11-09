/* SPDX-License-Identifier: MPL-2.0 */
/*
 * OT-61 C3c-1: the POSIX calls SpiderMonkey's objects reference that neo-crt
 * had no definition for (the generator's link probe, #87). The bundle has no
 * filesystem, no time zones and one address space per process, so most of
 * these are honest refusals; each says what it does.
 *
 * Time: every struct tm is UTC (strftime's %Z is "UTC" too, neo_bottom.c).
 * time() reads CLOCK_REALTIME, which neo-crt's clock_gettime answers once
 * C1.3's realtime clock is in (#86); until then it returns -1 as POSIX
 * allows when the time isn't available.
 */
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <time.h>
#include "stdio_impl.h"
#include "time_impl.h"

/* musl declares timegm only for _GNU_SOURCE/_BSD_SOURCE (src/time/timegm.c). */
time_t timegm(struct tm *);

/* musl's __tz.c defines this; neo-crt doesn't compile __tz.c (no TZ). */
const char __utc[] = "UTC";

time_t time(time_t *t)
{
	struct timespec ts;
	if (clock_gettime(CLOCK_REALTIME, &ts)) return (time_t)-1;
	if (t) *t = ts.tv_sec;
	return ts.tv_sec;
}

struct tm *localtime_r(const time_t *restrict t, struct tm *restrict tm)
{
	return gmtime_r(t, tm);
}

struct tm *localtime(const time_t *t)
{
	static struct tm tm;
	return localtime_r(t, &tm);
}

time_t mktime(struct tm *tm)
{
	return timegm(tm);
}

/* No filesystem: nothing can be opened or removed. */
FILE *fopen(const char *restrict path, const char *restrict mode)
{
	(void)path; (void)mode;
	errno = ENOENT;
	return 0;
}

int remove(const char *path)
{
	(void)path;
	errno = ENOENT;
	return -1;
}

/* Every FILE is one of the three standard streams (there is no fopen), and
 * they are permanent: closing one flushes it and leaves it usable, as musl
 * does for F_PERM streams. */
int fclose(FILE *f)
{
	return fflush(f);
}

/* No file descriptors beyond the standard streams' numbers, which aren't
 * open files either: these refuse every descriptor. */
int fstat(int fd, struct stat *st)
{
	(void)fd; (void)st;
	errno = EBADF;
	return -1;
}

int fcntl(int fd, int cmd, ...)
{
	(void)fd; (void)cmd;
	errno = EBADF;
	return -1;
}

int close(int fd)
{
	(void)fd;
	errno = EBADF;
	return -1;
}

int memfd_create(const char *name, unsigned flags)
{
	(void)name; (void)flags;
	errno = ENOSYS;
	return -1;
}

/* -fstack-protector-strong (SpiderMonkey's configure turns it on). On a
 * target with no TLS ABI, clang reads this global canary.
 * neo_crt::seed_stack_canary() replaces the fixed value with a random one,
 * low byte zero, before any C code runs; a program with stack-protected code
 * must call it. This value stands only if it doesn't. */
uintptr_t __stack_chk_guard = 0x595e9fbd94fda700;


_Noreturn void __stack_chk_fail(void)
{
	static const char msg[] = "neo-crt: stack smashing detected\n";
	fwrite(msg, 1, sizeof msg - 1, stderr);
	fflush(stderr);
	abort();
}
