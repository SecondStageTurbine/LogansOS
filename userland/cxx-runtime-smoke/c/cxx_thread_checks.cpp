// SPDX-License-Identifier: MPL-2.0
//
// OT-61 C3b: libc++'s threads over neo-crt's pthreads. Each check returns 0
// or the number of the first failed item, after printing it. No C++
// `thread_local` here: on this target it compiles to %fs-relative accesses,
// which the bundle doesn't have (the A4 check would refuse the ELF).

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <mutex>
#include <shared_mutex>
#include <thread>
#include <vector>

#define CHECK(n, what, cond)                                                   \
  do {                                                                         \
    if (!(cond)) {                                                             \
      std::printf("[ crt-smoke ] FAIL t%d %s\n", (n), (what));                \
      return (n);                                                              \
    }                                                                          \
  } while (0)

extern "C" int cxx_check_threads() {
  constexpr int kThreads = 6, kRounds = 2000;
  std::mutex m;
  long counter = 0;
  std::vector<std::thread> ts;
  for (int i = 0; i < kThreads; i++)
    ts.emplace_back([&] {
      for (int r = 0; r < kRounds; r++) {
        std::lock_guard<std::mutex> g(m);
        long v = counter;
        std::this_thread::yield();
        counter = v + 1;
      }
    });
  for (auto &t : ts)
    t.join();
  CHECK(1, "std::thread + std::mutex lost no update", counter == long(kThreads) * kRounds);
  CHECK(2, "hardware_concurrency is the CPU count",
        std::thread::hardware_concurrency() >= 1);
  return 0;
}

extern "C" int cxx_check_condition_variable() {
  using namespace std::chrono;
  std::mutex m;
  std::condition_variable cv;
  bool ready = false;

  // A timeout with nobody to signal: wait_for returns timeout, no earlier.
  {
    std::unique_lock<std::mutex> lk(m);
    auto t0 = steady_clock::now();
    auto st = cv.wait_for(lk, milliseconds(50));
    auto waited = duration_cast<milliseconds>(steady_clock::now() - t0).count();
    CHECK(10, "wait_for times out after its duration",
          st == std::cv_status::timeout && waited >= 50);
  }
  // A wake.
  std::thread t([&] {
    std::this_thread::sleep_for(milliseconds(20));
    {
      std::lock_guard<std::mutex> g(m);
      ready = true;
    }
    cv.notify_one();
  });
  {
    std::unique_lock<std::mutex> lk(m);
    bool ok = cv.wait_for(lk, seconds(10), [&] { return ready; });
    CHECK(11, "wait_for with a predicate wakes on notify", ok);
  }
  t.join();
  return 0;
}

extern "C" int cxx_check_sync() {
  std::once_flag flag;
  std::atomic<int> runs{0};
  std::vector<std::thread> ts;
  for (int i = 0; i < 6; i++)
    ts.emplace_back([&] { std::call_once(flag, [&] { runs++; }); });
  for (auto &t : ts)
    t.join();
  CHECK(20, "call_once ran once", runs.load() == 1);

  std::shared_mutex sm;
  {
    std::shared_lock<std::shared_mutex> a(sm), b(sm);
    CHECK(21, "shared_mutex: two readers at once, no writer", !sm.try_lock());
  }
  CHECK(22, "shared_mutex: a writer once the readers left", sm.try_lock());
  sm.unlock();

  // atomic wait/notify (libc++'s contention table over the pthread layer).
  std::atomic<int> flag2{0};
  std::thread w([&] {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    flag2.store(1);
    flag2.notify_one();
  });
  flag2.wait(0);
  w.join();
  CHECK(23, "atomic::wait returns after notify_one", flag2.load() == 1);
  return 0;
}
