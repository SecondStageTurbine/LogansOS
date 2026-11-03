// SPDX-License-Identifier: MPL-2.0
//
// OT-61 C3a: the C++ half of cxx-runtime-smoke (libc++ 18.1.8 over neo-crt;
// no exceptions, no RTTI). Each cxx_check_* returns 0 or the number of the
// first failed item, after printing it. Expected values are computed by hand
// (sums, counts) or are the exact strings the C++ standard fixes (to_chars
// shortest round-trip output, to_string).

#include <algorithm>
#include <atomic>
#include <charconv>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <functional>
#include <map>
#include <memory>
#include <new>
#include <numeric>
#include <optional>
#include <string>
#include <string_view>
#include <unordered_map>
#include <variant>
#include <vector>

#define CHECK(n, what, cond)                                                   \
  do {                                                                         \
    if (!(cond)) {                                                             \
      std::printf("[ crt-smoke ] FAIL x%d %s\n", (n), (what));                \
      return (n);                                                              \
    }                                                                          \
  } while (0)

namespace {

// A namespace-scope object with a constructor: it runs only if neo_crt::init
// walked .init_array before main code. The value comes through a volatile
// read, so the compiler can't constant-initialise it (a constant initialiser
// would pass this check without any constructor running).
volatile int g_seed = 0x5eed;
struct Global {
  int magic;
  Global() : magic(g_seed) {}
};
Global g_global;

struct Shape {
  virtual ~Shape() = default;
  virtual int area() const = 0;
};
struct Rect final : Shape {
  int w, h;
  Rect(int w, int h) : w(w), h(h) {}
  int area() const override { return w * h; }
};
struct Square final : Shape {
  int s;
  explicit Square(int s) : s(s) {}
  int area() const override { return s * s; }
};

} // namespace

extern "C" int cxx_check_globals() {
  CHECK(1, "a global constructor ran (.init_array)", g_global.magic == 0x5eed);
  return 0;
}

extern "C" int cxx_check_containers() {
  std::vector<int> v;
  for (int i = 0; i < 10000; i++)
    v.push_back((i * 7919) % 10007);
  std::sort(v.begin(), v.end());
  CHECK(10, "vector + sort", std::is_sorted(v.begin(), v.end()) && v.size() == 10000);
  // i*7919 mod 10007 is a permutation of 0..10006 restricted to i < 10000,
  // so the sum is unchanged by sorting: sum over i of (7919 i mod 10007).
  long long want = 0;
  for (int i = 0; i < 10000; i++)
    want += (i * 7919) % 10007;
  CHECK(11, "accumulate", std::accumulate(v.begin(), v.end(), 0LL) == want);

  std::unordered_map<std::string, int> m;
  for (int i = 0; i < 1000; i++)
    m["key" + std::to_string(i)] = i * i;
  CHECK(12, "unordered_map", m.size() == 1000 && m["key999"] == 998001 && !m.count("nope"));

  std::map<int, std::string> ordered{{3, "c"}, {1, "a"}, {2, "b"}};
  std::string joined;
  for (auto &[k, s] : ordered)
    joined += s;
  CHECK(13, "map order", joined == "abc");
  return 0;
}

extern "C" int cxx_check_strings() {
  std::string s = "hello";
  s += ", world";
  CHECK(20, "string append/find/substr",
        s == "hello, world" && s.find("world") == 7 && s.substr(7, 3) == "wor");
  CHECK(21, "to_string", std::to_string(12345) == "12345" && std::to_string(-7) == "-7");
  // std::to_chars without a format gives the shortest string that reads back
  // to the same double ([charconv.to.chars]): 0.1 -> "0.1", 1e22 -> "1e+22".
  char buf[64];
  auto r = std::to_chars(buf, buf + sizeof buf, 0.1);
  CHECK(22, "to_chars(0.1)", std::string_view(buf, r.ptr - buf) == "0.1");
  r = std::to_chars(buf, buf + sizeof buf, 1e22);
  CHECK(23, "to_chars(1e22)", std::string_view(buf, r.ptr - buf) == "1e+22");
  r = std::to_chars(buf, buf + sizeof buf, 255, 16);
  CHECK(24, "to_chars(255, 16)", std::string_view(buf, r.ptr - buf) == "ff");
  int parsed = 0;
  const char *num = "-4096";
  auto fr = std::from_chars(num, num + 5, parsed);
  CHECK(25, "from_chars", fr.ec == std::errc() && parsed == -4096);
  return 0;
}

extern "C" int cxx_check_objects() {
  static int deleted = 0;
  {
    auto del = [](int *p) {
      deleted++;
      delete p;
    };
    std::unique_ptr<int, decltype(del)> u(new int(5), del);
    CHECK(30, "unique_ptr", *u == 5);
  }
  CHECK(30, "unique_ptr deleter ran", deleted == 1);
  auto sp = std::make_shared<std::vector<int>>(3, 9);
  auto sp2 = sp;
  CHECK(31, "shared_ptr", sp.use_count() == 2 && (*sp2)[2] == 9);
  sp2.reset();
  CHECK(31, "shared_ptr release", sp.use_count() == 1);

  std::function<int(int)> twice = [](int x) { return x * 2; };
  CHECK(32, "function", twice(21) == 42);
  std::optional<int> none, some = 3;
  CHECK(33, "optional", !none && some.value_or(0) == 3);
  std::variant<int, std::string> var = std::string("v");
  CHECK(34, "variant", std::holds_alternative<std::string>(var) &&
                           std::visit([](auto &x) { return sizeof(x) > 0; }, var));

  std::vector<std::unique_ptr<Shape>> shapes;
  shapes.push_back(std::make_unique<Rect>(3, 4));
  shapes.push_back(std::make_unique<Square>(5));
  int total = 0;
  for (auto &p : shapes)
    total += p->area();
  CHECK(35, "virtual dispatch", total == 37);

  // Allocation failure without exceptions: the nothrow form says so.
  volatile size_t huge = SIZE_MAX / 4;
  int *volatile none_p = new (std::nothrow) int[huge];
  CHECK(36, "new(nothrow) of an impossible size -> nullptr", none_p == nullptr);
  return 0;
}

extern "C" int cxx_check_clock() {
  auto a = std::chrono::steady_clock::now();
  for (int i = 0; i < 100000; i++) {
    __asm__ volatile("" ::: "memory");
  }
  auto b = std::chrono::steady_clock::now();
  CHECK(40, "steady_clock does not go back", b >= a);
  return 0;
}

// Called from several threads at once (from Rust). The function-local static
// is built exactly once: its constructor takes long enough that every caller
// arrives while it runs, and all of them must see the one finished object.
namespace {
std::atomic<int> g_built{0};
struct Slow {
  int value;
  Slow() {
    g_built.fetch_add(1);
    for (int i = 0; i < 20000000; i++) {
      __asm__ volatile("" ::: "memory");
    }
    value = 77;
  }
};
} // namespace

extern "C" int cxx_static_guard_probe(int) {
  static Slow once;
  return once.value == 77 && g_built.load() == 1;
}

extern "C" int cxx_static_guard_builds() { return g_built.load(); }

// The hardening-trap twin: libc++'s fast hardening mode checks
// vector::operator[] bounds and traps (ud2) on a violation.
extern "C" int cxx_out_of_bounds(int index) {
  std::vector<int> v(4, 1);
  return v[index];
}
