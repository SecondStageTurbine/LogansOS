// SPDX-License-Identifier: MPL-2.0
//
// OT-61 C3c-1: libc++'s localization and iostreams over neo-crt (the "C"
// locale only). SpiderMonkey and the V8-derived irregexp need <ostream>,
// <sstream> and <iomanip>. Built with _GNU_SOURCE (see neo-crt's
// cxx/__config_site). Every expected string is worked by hand from the
// standard's formatting rules for the "C" locale.

#include <cstdio>
#include <ctime>
#include <iomanip>
#include <iostream>
#include <locale>
#include <regex>
#include <sstream>
#include <string>

#define CHECK(n, what, cond)                                                   \
  do {                                                                         \
    if (!(cond)) {                                                             \
      std::printf("[ crt-smoke ] FAIL x%d %s\n", (n), (what));                \
      return (n);                                                              \
    }                                                                          \
  } while (0)

extern "C" int cxx_check_streams() {
  // Integers, a double (default precision 6, shortest form "1.5"), hex, and
  // a width-5 field padded with '*' on the left.
  std::ostringstream o;
  o << 42 << ' ' << 1.5 << ' ' << std::hex << 255 << ' ' << std::dec
    << std::setw(5) << std::setfill('*') << 7;
  CHECK(90, "ostringstream: int, double, hex, setw/setfill",
        o.str() == "42 1.5 ff ****7");

  std::istringstream in("12 34 x");
  int a = 0, b = 0, c = 0;
  in >> a >> b;
  CHECK(91, "istringstream reads two ints", a == 12 && b == 34);
  in >> c;
  CHECK(92, "a failed extraction sets failbit", in.fail() && c == 0);

  CHECK(93, "the global locale is \"C\"", std::locale().name() == "C");

  // 2026-11-08 (tm_year counts from 1900, tm_mon from 0); no time zones,
  // so %Z is "UTC" (neo-crt's __tm_to_tzname).
  std::tm t{};
  t.tm_year = 126;
  t.tm_mon = 10;
  t.tm_mday = 8;
  t.tm_hour = 9;
  t.tm_min = 5;
  std::ostringstream ts;
  ts << std::put_time(&t, "%Y-%m-%d %H:%M %Z");
  CHECK(94, "put_time (strftime) in the C locale",
        ts.str() == "2026-11-08 09:05 UTC");

  CHECK(95, "regex_match",
        std::regex_match("abc123", std::regex("[a-z]+[0-9]+")) &&
            !std::regex_match("123abc", std::regex("[a-z]+[0-9]+")));

  // The bundle has no standard input: std::cin reads end-of-file.
  CHECK(96, "std::cin reads end-of-file",
        std::cin.get() == std::char_traits<char>::eof() && std::cin.eof());
  return 0;
}
