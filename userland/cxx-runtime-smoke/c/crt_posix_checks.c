/* SPDX-License-Identifier: MPL-2.0 */
/*
 * OT-61 C3c-1: the POSIX calls neo-crt gained for SpiderMonkey
 * (userland/neo-crt/c/neo_posix.c). Expected values are worked by hand:
 * 31,536,000 s is exactly 365 days after the epoch, 1971-01-01 00:00:00 UTC
 * (1970 is not a leap year), a Friday (1970-01-01 was a Thursday; 365 days
 * is 52 weeks and 1 day).
 */
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define CHECK(n, what, cond)                                                   \
  do {                                                                         \
    if (!(cond)) {                                                             \
      printf("[ crt-smoke ] FAIL x%d %s\n", (n), (what));                     \
      return (n);                                                              \
    }                                                                          \
  } while (0)

extern uintptr_t __stack_chk_guard;

int crt_check_posix(void) {
  time_t t = 31536000;
  struct tm g;
  CHECK(100, "gmtime_r(365 days)",
        gmtime_r(&t, &g) == &g && g.tm_year == 71 && g.tm_mon == 0 &&
            g.tm_mday == 1 && g.tm_hour == 0 && g.tm_wday == 5 &&
            g.tm_yday == 0);
  struct tm l;
  CHECK(101, "localtime_r is UTC",
        localtime_r(&t, &l) == &l && memcmp(&l, &g, sizeof g) == 0);
  struct tm m = {0};
  m.tm_year = 71;
  m.tm_mday = 1;
  CHECK(102, "mktime(1971-01-01) is UTC", mktime(&m) == 31536000);
  char buf[32];
  CHECK(103, "strftime %Z is UTC",
        strftime(buf, sizeof buf, "%Y-%m-%d %Z", &g) == 14 &&
            strcmp(buf, "1971-01-01 UTC") == 0);
  errno = 0;
  CHECK(104, "fopen refuses (no filesystem)",
        fopen("/etc/passwd", "r") == NULL && errno == ENOENT);
  CHECK(105, "fileno/ferror on stdout",
        fileno(stdout) == 1 && ferror(stdout) == 0);
  CHECK(106, "getpid is positive", getpid() > 0);
  CHECK(107, "the stack canary was randomised, low byte zero",
        __stack_chk_guard != 0x595e9fbd94fda700u &&
            (__stack_chk_guard & 0xff) == 0 && __stack_chk_guard != 0);
  return 0;
}
