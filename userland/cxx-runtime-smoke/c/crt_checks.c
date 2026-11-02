/* SPDX-License-Identifier: MPL-2.0 */
/*
 * OT-61 C3a: the C half of cxx-runtime-smoke. Each crt_check_* returns 0 on
 * success or the number of the first failed item, and prints what failed.
 *
 * The expected values are an independent oracle, not neo-crt's own output:
 * format strings follow C11 7.21.6.1 by hand; the double bit patterns come
 * from CPython's correctly rounded float parser and repr (David Gay's
 * algorithms, a different implementation from musl's) and, for libm, from
 * glibc through CPython's math module, compared within one ulp because the
 * C standard does not require correct rounding there. Where IEEE 754 does
 * require an exact result (sqrt, fma, the rounding functions, exact
 * conversions), the comparison is exact. (fma(0.1, 10, -1) is 2^-54: 0.1 is
 * 0x1.999999999999Ap-4, and ten times that is exactly 1 + 2^-54.)
 */
#include <errno.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

static int fail(int n, const char *what, const char *got, const char *want)
{
	printf("[ crt-smoke ] FAIL c%d %s: got \"%s\" want \"%s\"\n", n, what, got, want);
	return n;
}

static uint64_t bits(double d)
{
	uint64_t u;
	memcpy(&u, &d, sizeof u);
	return u;
}

static int bits_eq(int n, const char *what, double got, uint64_t want)
{
	if (bits(got) == want) return 0;
	printf("[ crt-smoke ] FAIL c%d %s: got 0x%016llX want 0x%016llX\n", n, what,
	       (unsigned long long)bits(got), (unsigned long long)want);
	return n;
}

static int within_ulp(int n, const char *what, double got, uint64_t want)
{
	uint64_t g = bits(got);
	uint64_t d = g > want ? g - want : want - g;
	if (d <= 1) return 0;
	printf("[ crt-smoke ] FAIL c%d %s: got 0x%016llX want 0x%016llX (+-1 ulp)\n", n,
	       what, (unsigned long long)g, (unsigned long long)want);
	return n;
}

#define STR_EQ(n, what, got, want) \
	do { if (strcmp((got), (want))) return fail((n), (what), (got), (want)); } while (0)
#define CHECK(n, what, cond) \
	do { if (!(cond)) { printf("[ crt-smoke ] FAIL c%d %s\n", (n), (what)); return (n); } } while (0)

/* printf family: every conversion's output written out by hand. */
int crt_check_format(void)
{
	char b[128];
	int r;

	snprintf(b, sizeof b, "%d|%5d|%-5d|%05d|%x|%X|%o|%s|%c|%%", 42, -7, 3, 99, 0xbeef,
		 0xBEEF, 8, "neo", 'Z');
	STR_EQ(1, "integers", b, "42|   -7|3    |00099|beef|BEEF|10|neo|Z|%");
	snprintf(b, sizeof b, "%lld %zu", (long long)INT64_MIN, (size_t)SIZE_MAX);
	STR_EQ(2, "64-bit extremes", b, "-9223372036854775808 18446744073709551615");
	snprintf(b, sizeof b, "%.3f %e %g %g", 3.14159, 12345.678, 0.0001234, 1e20);
	STR_EQ(3, "f e g", b, "3.142 1.234568e+04 0.0001234 1e+20");
	snprintf(b, sizeof b, "%.17g %.20f", 0.1, 0.1);
	STR_EQ(4, "0.1 exactly", b, "0.10000000000000001 0.10000000000000000555");
	snprintf(b, sizeof b, "%a %f %10.4s|", 1.0, 1e15, "abcdefg");
	STR_EQ(5, "a f precision", b, "0x1p+0 1000000000000000.000000       abcd|");
	r = snprintf(b, 5, "%s", "abcdefgh");
	CHECK(6, "truncation returns the full length", r == 8);
	STR_EQ(6, "truncation", b, "abcd");
	snprintf(b, sizeof b, "%g %g %g", INFINITY, -INFINITY, NAN);
	STR_EQ(7, "inf nan", b, "inf -inf nan");
	return 0;
}

/* strtod/strtol and sscanf. */
int crt_check_scan(void)
{
	int e;
	if ((e = bits_eq(10, "strtod 0.1", strtod("0.1", 0), 0x3FB999999999999AULL))) return e;
	if ((e = bits_eq(11, "strtod subnormal 1e-310", strtod("1e-310", 0),
			 0x000012688B70E62BULL))) return e;
	if ((e = bits_eq(12, "strtod 2.2250738585072011e-308",
			 strtod("2.2250738585072011e-308", 0), 0x000FFFFFFFFFFFFFULL))) return e;
	if ((e = bits_eq(13, "strtod hex float", strtod("0x1.8p1", 0), 0x4008000000000000ULL)))
		return e;

	char *end;
	CHECK(14, "strtol base 16", strtol("0x7fffffffffffffff", &end, 16) == LONG_MAX && *end == 0);
	errno = 0;
	CHECK(15, "strtol overflow -> LONG_MAX, ERANGE",
	      strtol("99999999999999999999", 0, 10) == LONG_MAX && errno == ERANGE);
	CHECK(16, "strtoul base 2", strtoul("101101", 0, 2) == 45);

	int i = 0;
	char s[8] = { 0 };
	double d = 0;
	int n = sscanf("12 abcdef 3.5", "%d %3s%*s %lf", &i, s, &d);
	CHECK(17, "sscanf", n == 3 && i == 12 && !strcmp(s, "abc") && d == 3.5);
	return 0;
}

/* libm: exact where IEEE 754 is exact, within an ulp elsewhere. */
int crt_check_math(void)
{
	int e;
	volatile double two = 2.0, one = 1.0, ten = 10.0, half = 0.5;
	if ((e = bits_eq(20, "sqrt(2)", sqrt(two), 0x3FF6A09E667F3BCDULL))) return e;
	if ((e = bits_eq(21, "fma(0.1, 10, -1)", fma(0.1, ten, -one), 0x3C90000000000000ULL)))
		return e;
	CHECK(22, "rounding functions",
	      floor(-2.5) == -3.0 && ceil(-2.5) == -2.0 && trunc(-2.5) == -2.0 &&
	      round(-2.5) == -3.0 && rint(2.5) == 2.0 && nearbyint(3.5) == 4.0);
	CHECK(23, "exact helpers",
	      fmod(10.0, 3.0) == 1.0 && hypot(3.0, 4.0) == 5.0 && cbrt(27.0) == 3.0 &&
	      ldexp(0.75, 4) == 12.0 && pow(2.0, 10.0) == 1024.0 && exp(0.0) == 1.0 &&
	      log(1.0) == 0.0);
	int x;
	CHECK(24, "frexp", frexp(48.0, &x) == 0.75 && x == 6);
	if ((e = within_ulp(25, "sin(1)", sin(one), 0x3FEAED548F090CEEULL))) return e;
	if ((e = within_ulp(26, "exp(1)", exp(one), 0x4005BF0A8B145769ULL))) return e;
	if ((e = within_ulp(27, "log(10)", log(ten), 0x40026BB1BBB55516ULL))) return e;
	if ((e = within_ulp(28, "pow(2, 0.5)", pow(two, half), 0x3FF6A09E667F3BCDULL))) return e;
	if ((e = within_ulp(29, "atan2(1, 2)", atan2(one, two), 0x3FDDAC670561BB4FULL))) return e;
	if ((e = within_ulp(30, "tanh(0.5)", tanh(half), 0x3FDD9353D7568AF3ULL))) return e;
	CHECK(31, "isnan/isinf/signbit",
	      isnan(NAN) && isinf(-INFINITY) && signbit(-0.0) && !signbit(0.0));
	return 0;
}

static int cmp_int(const void *a, const void *b)
{
	int x = *(const int *)a, y = *(const int *)b;
	return (x > y) - (x < y);
}

/* strings, qsort/bsearch, the PRNG, strerror. */
int crt_check_strings(void)
{
	char b[64];
	CHECK(40, "strstr/strchr/strrchr/memchr",
	      !strcmp(strstr("hello world", "o w"), "o world") &&
	      !strcmp(strchr("a.b.c", '.'), ".b.c") && !strcmp(strrchr("a.b.c", '.'), ".c") &&
	      memchr("abc", 'c', 3) != 0 && memchr("abc", 'd', 3) == 0);
	CHECK(41, "strspn/strcspn/strcasecmp/strncmp",
	      strspn("aabbc", "ab") == 4 && strcspn("hello", "lo") == 2 &&
	      strcasecmp("NeO", "nEo") == 0 && strncmp("abcx", "abcy", 3) == 0);
	strcpy(b, "x,y,,z");
	char *save, *t1 = strtok_r(b, ",", &save), *t2 = strtok_r(0, ",", &save),
		    *t3 = strtok_r(0, ",", &save), *t4 = strtok_r(0, ",", &save);
	CHECK(42, "strtok_r", !strcmp(t1, "x") && !strcmp(t2, "y") && !strcmp(t3, "z") && !t4);
	char *dup = strdup("duplicate");
	CHECK(43, "strdup", dup && !strcmp(dup, "duplicate"));
	free(dup);
	memset(b, 'Q', sizeof b);
	strncpy(b, "ab", 6);
	CHECK(44, "strncpy pads with NULs", !memcmp(b, "ab\0\0\0\0Q", 7));
	STR_EQ(45, "strerror(ERANGE)", strerror(ERANGE), "Result not representable");

	enum { N = 1000 };
	static int v[N];
	srand48(0x5eed);
	for (int i = 0; i < N; i++) v[i] = (int)(lrand48() % 100000);
	qsort(v, N, sizeof v[0], cmp_int);
	for (int i = 1; i < N; i++) CHECK(46, "qsort order", v[i - 1] <= v[i]);
	int key = v[N / 2];
	int *hit = bsearch(&key, v, N, sizeof v[0], cmp_int);
	CHECK(47, "bsearch", hit && *hit == key);
	/* rand48 is specified exactly (POSIX): X(n+1) = (a X(n) + c) mod 2^48,
	 * a = 0x5DEECE66D, c = 0xB, seed48 high 32 bits = the srand48 argument,
	 * low 16 = 0x330E; lrand48 returns bits 47..17. */
	srand48(1);
	uint64_t X = ((uint64_t)1 << 16) | 0x330E;
	for (int i = 0; i < 4; i++) {
		X = (0x5DEECE66DULL * X + 0xB) & ((1ULL << 48) - 1);
		CHECK(48, "lrand48 sequence", lrand48() == (long)(X >> 17));
	}
	return 0;
}

/* The allocator: sizes, growth, alignment, zeroing, bad arguments. */
int crt_check_alloc(void)
{
	for (size_t n = 1; n <= 8192; n = n * 3 + 1) {
		unsigned char *p = malloc(n);
		CHECK(50, "malloc", p != 0 && ((uintptr_t)p & 15) == 0);
		memset(p, 0xA5, n);
		unsigned char *q = realloc(p, n * 2 + 7);
		CHECK(51, "realloc grows", q != 0);
		for (size_t i = 0; i < n; i++) CHECK(51, "realloc keeps the bytes", q[i] == 0xA5);
		q = realloc(q, n / 2 + 1);
		CHECK(52, "realloc shrinks", q != 0 && q[0] == 0xA5);
		free(q);
	}
	unsigned char *z = calloc(1000, 3);
	CHECK(53, "calloc", z != 0);
	for (int i = 0; i < 3000; i++) CHECK(53, "calloc zeroes", z[i] == 0);
	free(z);
	/* volatile, twice: clang knows calloc, may drop an unused call and fold
	 * its NULL test, and models it as writing no other memory, so a plain
	 * read of errno after it folds to the 0 stored before (musl's
	 * __errno_location is const). The runtime sets ENOMEM either way. */
	volatile size_t huge = SIZE_MAX / 2;
	errno = 0;
	void *volatile none = calloc(huge, 3);
	int err = *(volatile int *)&errno;
	CHECK(54, "calloc overflow -> NULL, ENOMEM", none == 0 && err == ENOMEM);
	void *a64 = aligned_alloc(64, 100), *a4k = aligned_alloc(4096, 4096);
	CHECK(55, "aligned_alloc",
	      a64 && a4k && ((uintptr_t)a64 & 63) == 0 && ((uintptr_t)a4k & 4095) == 0);
	free(a64);
	free(a4k);
	/* Chad #219: alignment 0 and a non-power-of-two alignment are refused
	 * (NULL, EINVAL); a size that isn't a multiple of the alignment is
	 * accepted (C17/C23), aligned and writable to its last byte. errno read
	 * through volatile, as above. */
	volatile size_t align0 = 0, align48 = 48;
	errno = 0;
	void *volatile r0 = aligned_alloc(align0, 16);
	int e0 = *(volatile int *)&errno;
	CHECK(58, "aligned_alloc(0, 16) -> NULL, EINVAL", r0 == 0 && e0 == EINVAL);
	errno = 0;
	void *volatile r48 = aligned_alloc(align48, 96);
	int e48 = *(volatile int *)&errno;
	CHECK(59, "aligned_alloc(48, 96) -> NULL, EINVAL", r48 == 0 && e48 == EINVAL);
	unsigned char *odd = aligned_alloc(32, 33);
	CHECK(60, "aligned_alloc(32, 33): a size off the multiple is accepted",
	      odd != 0 && ((uintptr_t)odd & 31) == 0);
	memset(odd, 0x5A, 33);
	CHECK(60, "aligned_alloc(32, 33): all 33 bytes usable", odd[0] == 0x5A && odd[32] == 0x5A);
	free(odd);
	void *pm = 0;
	CHECK(56, "posix_memalign", posix_memalign(&pm, 256, 10) == 0 && ((uintptr_t)pm & 255) == 0);
	free(pm);
	CHECK(57, "posix_memalign bad alignment -> EINVAL", posix_memalign(&pm, 24, 10) == EINVAL);
	free(0);
	return 0;
}

/* clock_gettime: a monotonic clock, and no wall clock. */
int crt_check_clock(void)
{
	struct timespec a, b;
	CHECK(60, "CLOCK_MONOTONIC", clock_gettime(CLOCK_MONOTONIC, &a) == 0);
	for (volatile int i = 0; i < 100000; i++) {}
	CHECK(61, "CLOCK_MONOTONIC again", clock_gettime(CLOCK_MONOTONIC, &b) == 0);
	CHECK(62, "monotonic does not go back",
	      b.tv_sec > a.tv_sec || (b.tv_sec == a.tv_sec && b.tv_nsec >= a.tv_nsec));
	CHECK(63, "tv_nsec in range", b.tv_nsec >= 0 && b.tv_nsec < 1000000000);
	errno = 0;
	CHECK(64, "CLOCK_REALTIME refused (EINVAL)",
	      clock_gettime(CLOCK_REALTIME, &a) == -1 && errno == EINVAL);
	return 0;
}

/* stdout through musl's FILE and neo-crt's write path. */
int crt_check_stdout(void)
{
	int n = printf("[ crt-smoke ] stdout via musl printf: %s %d\n", "ok", 7);
	fflush(stdout);
	CHECK(70, "printf returns the byte count", n == 43);
	return 0;
}

/* CP-106: called from a second thread while the first holds stdout's lock
 * (from Rust, through neo-crt's __lockfile); it must not return before the
 * lock is released. */
int crt_stdout_marker(void)
{
	int r = fputs("[ crt-smoke ] stdout marker, after the holder let go\n", stdout);
	fflush(stdout);
	return r >= 0;
}

/* CP-106, the bounce's repro: several threads print 600-byte runs with no
 * newline at once, so every printf goes through __fwritex's static buffer.
 * Each run is one letter per thread. */
int crt_stdio_hammer(int id)
{
	char run[601];
	memset(run, 'a' + id, 600);
	run[600] = 0;
	for (int i = 0; i < 4; i++)
		if (printf("%s", run) != 600)
			return 0;
	return 1;
}

/* Ends the hammer's line. */
void crt_stdio_end_runs(void)
{
	putchar('\n');
	fflush(stdout);
}

/* Run on several threads at once (from Rust): errno must be per thread. */
int crt_errno_probe(int id)
{
	for (int round = 0; round < 2000; round++) {
		errno = 1000 + id;
		for (volatile int i = 0; i < 50; i++) {}
		if (errno != 1000 + id) {
			printf("[ crt-smoke ] FAIL errno thread %d saw %d\n", id, errno);
			return 0;
		}
	}
	return 1;
}

/* Run on several threads at once (from Rust): allocator stress, each block
 * filled with a per-thread pattern and checked before it is freed. */
int crt_malloc_stress(int id)
{
	enum { SLOTS = 64 };
	unsigned char *slot[SLOTS] = { 0 };
	size_t len[SLOTS] = { 0 };
	unsigned seed = 0x9e3779b9u * (unsigned)(id + 1);
	for (int step = 0; step < 4000; step++) {
		seed = seed * 1103515245u + 12345u;
		int k = (seed >> 8) % SLOTS;
		if (slot[k]) {
			for (size_t i = 0; i < len[k]; i++)
				if (slot[k][i] != (unsigned char)(id ^ i)) {
					printf("[ crt-smoke ] FAIL malloc thread %d slot %d corrupted\n", id, k);
					return 0;
				}
			free(slot[k]);
			slot[k] = 0;
		} else {
			len[k] = 1 + (seed >> 16) % 2048;
			slot[k] = malloc(len[k]);
			if (!slot[k]) return 0;
			for (size_t i = 0; i < len[k]; i++) slot[k][i] = (unsigned char)(id ^ i);
		}
	}
	for (int k = 0; k < SLOTS; k++) free(slot[k]);
	return 1;
}
