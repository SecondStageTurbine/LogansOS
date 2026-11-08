/* SPDX-License-Identifier: MPL-2.0 */
/*
 * OT-61 C3b: the pthread half of cxx-runtime-smoke. Each check returns 0 or
 * the number of the first failed item, after printing it. The calls are the
 * ones SpiderMonkey's POSIX threading layer and libc++'s thread API make.
 */
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <time.h>

#define CHECK(n, what, cond)                                                   \
	do {                                                                   \
		if (!(cond)) {                                                 \
			printf("[ crt-smoke ] FAIL p%d %s\n", (n), (what));    \
			return (n);                                            \
		}                                                              \
	} while (0)

enum { WORKERS = 6, ROUNDS = 2000 };

static pthread_mutex_t counter_lock = PTHREAD_MUTEX_INITIALIZER;
static long counter;

static void *add_rounds(void *arg)
{
	for (int i = 0; i < ROUNDS; i++) {
		pthread_mutex_lock(&counter_lock);
		long v = counter;
		sched_yield(); /* widen the window: a broken lock loses updates */
		counter = v + 1;
		pthread_mutex_unlock(&counter_lock);
	}
	return (void *)((intptr_t)arg * 10);
}

/* create/join with return values, and a mutex that really excludes. */
int pt_check_threads(void)
{
	pthread_t t[WORKERS];
	for (intptr_t i = 0; i < WORKERS; i++)
		CHECK(1, "pthread_create", pthread_create(&t[i], 0, add_rounds, (void *)i) == 0);
	for (intptr_t i = 0; i < WORKERS; i++) {
		void *ret = 0;
		CHECK(2, "pthread_join", pthread_join(t[i], &ret) == 0);
		CHECK(2, "pthread_join returns the start routine's value", ret == (void *)(i * 10));
	}
	CHECK(3, "the mutex lost no update", counter == (long)WORKERS * ROUNDS);
	CHECK(4, "pthread_self/equal", pthread_equal(pthread_self(), pthread_self()) &&
					   !pthread_equal(pthread_self(), t[0]));
	return 0;
}

/* Mutex kinds. */
int pt_check_mutex_kinds(void)
{
	pthread_mutexattr_t a;
	pthread_mutex_t m;
	pthread_mutexattr_init(&a);
	pthread_mutexattr_settype(&a, PTHREAD_MUTEX_RECURSIVE);
	pthread_mutex_init(&m, &a);
	CHECK(10, "recursive: lock twice, unlock twice",
	      pthread_mutex_lock(&m) == 0 && pthread_mutex_lock(&m) == 0 &&
		      pthread_mutex_unlock(&m) == 0 && pthread_mutex_unlock(&m) == 0);
	CHECK(10, "recursive: a third unlock is EPERM", pthread_mutex_unlock(&m) == EPERM);
	pthread_mutex_destroy(&m);

	pthread_mutexattr_settype(&a, PTHREAD_MUTEX_ERRORCHECK);
	pthread_mutex_init(&m, &a);
	CHECK(11, "errorcheck: relock is EDEADLK",
	      pthread_mutex_lock(&m) == 0 && pthread_mutex_lock(&m) == EDEADLK);
	CHECK(11, "errorcheck: trylock while held is EBUSY", pthread_mutex_trylock(&m) == EBUSY);
	pthread_mutex_unlock(&m);
	pthread_mutex_destroy(&m);
	pthread_mutexattr_destroy(&a);

	pthread_mutex_t n = PTHREAD_MUTEX_INITIALIZER;
	CHECK(12, "normal: trylock while held is EBUSY",
	      pthread_mutex_lock(&n) == 0 && pthread_mutex_trylock(&n) == EBUSY);
	pthread_mutex_unlock(&n);
	return 0;
}

/* Condition variables: wake, a monotonic timeout, the refused default clock. */
static pthread_mutex_t cv_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t cv;
static int ready;

static void *set_ready(void *arg)
{
	(void)arg;
	pthread_mutex_lock(&cv_lock);
	ready = 1;
	pthread_cond_signal(&cv);
	pthread_mutex_unlock(&cv_lock);
	return 0;
}

int pt_check_condvar(void)
{
	pthread_condattr_t ca;
	pthread_condattr_init(&ca);
	CHECK(20, "condattr_setclock(MONOTONIC)", pthread_condattr_setclock(&ca, CLOCK_MONOTONIC) == 0);
	pthread_cond_init(&cv, &ca);
	pthread_condattr_destroy(&ca);

	pthread_t t;
	pthread_mutex_lock(&cv_lock);
	pthread_create(&t, 0, set_ready, 0);
	while (!ready)
		pthread_cond_wait(&cv, &cv_lock);
	pthread_mutex_unlock(&cv_lock);
	pthread_join(t, 0);
	CHECK(21, "cond_wait woke on the signal", ready == 1);

	struct timespec now, at;
	clock_gettime(CLOCK_MONOTONIC, &now);
	at = now;
	at.tv_nsec += 50 * 1000 * 1000;
	if (at.tv_nsec >= 1000000000) {
		at.tv_sec++;
		at.tv_nsec -= 1000000000;
	}
	pthread_mutex_lock(&cv_lock);
	int r = pthread_cond_timedwait(&cv, &cv_lock, &at);
	pthread_mutex_unlock(&cv_lock);
	struct timespec after;
	clock_gettime(CLOCK_MONOTONIC, &after);
	long waited_ms = (after.tv_sec - now.tv_sec) * 1000 + (after.tv_nsec - now.tv_nsec) / 1000000;
	CHECK(22, "timedwait (monotonic) times out after its deadline",
	      r == ETIMEDOUT && waited_ms >= 50);

	pthread_cond_t plain = PTHREAD_COND_INITIALIZER;
	pthread_mutex_lock(&cv_lock);
	r = pthread_cond_timedwait(&plain, &cv_lock, &at);
	pthread_mutex_unlock(&cv_lock);
	CHECK(23, "timedwait on the default (realtime) clock is EINVAL", r == EINVAL);
	pthread_cond_destroy(&cv);
	return 0;
}

/* Keys: per-thread values, and destructors at thread exit. */
static pthread_key_t key;
static int destroyed[WORKERS];

static void key_dtor(void *v)
{
	destroyed[(intptr_t)v - 1]++;
}

static void *use_key(void *arg)
{
	if (pthread_getspecific(key) != 0)
		return (void *)1;
	pthread_setspecific(key, (void *)((intptr_t)arg + 1));
	sched_yield();
	return pthread_getspecific(key) == (void *)((intptr_t)arg + 1) ? 0 : (void *)1;
}

int pt_check_keys(void)
{
	CHECK(30, "key_create", pthread_key_create(&key, key_dtor) == 0);
	pthread_t t[WORKERS];
	for (intptr_t i = 0; i < WORKERS; i++)
		pthread_create(&t[i], 0, use_key, (void *)i);
	int bad = 0;
	for (int i = 0; i < WORKERS; i++) {
		void *r;
		pthread_join(t[i], &r);
		bad |= r != 0;
	}
	CHECK(31, "each thread sees only its own value", !bad);
	for (int i = 0; i < WORKERS; i++)
		CHECK(32, "each thread's destructor ran once at exit", destroyed[i] == 1);
	CHECK(33, "the main thread's value is untouched", pthread_getspecific(key) == 0);
	CHECK(34, "key_delete", pthread_key_delete(key) == 0 && pthread_setspecific(key, (void *)1) == EINVAL);
	return 0;
}

/* once */
static pthread_once_t once = PTHREAD_ONCE_INIT;
static int once_runs;
static void once_init(void)
{
	once_runs++;
	for (volatile int i = 0; i < 2000000; i++) {
	}
}
static void *call_once(void *arg)
{
	(void)arg;
	pthread_once(&once, once_init);
	return (void *)(intptr_t)once_runs;
}

int pt_check_once(void)
{
	pthread_t t[WORKERS];
	for (int i = 0; i < WORKERS; i++)
		pthread_create(&t[i], 0, call_once, 0);
	int all_saw_it = 1;
	for (int i = 0; i < WORKERS; i++) {
		void *r;
		pthread_join(t[i], &r);
		all_saw_it &= r == (void *)1;
	}
	CHECK(40, "pthread_once ran the routine once, and every caller saw it done",
	      once_runs == 1 && all_saw_it);
	return 0;
}

/* rwlock: readers share, a writer excludes. */
static pthread_rwlock_t rw = PTHREAD_RWLOCK_INITIALIZER;
int pt_check_rwlock(void)
{
	CHECK(50, "two read locks at once",
	      pthread_rwlock_rdlock(&rw) == 0 && pthread_rwlock_tryrdlock(&rw) == 0);
	CHECK(51, "no write lock while read", pthread_rwlock_trywrlock(&rw) == EBUSY);
	pthread_rwlock_unlock(&rw);
	pthread_rwlock_unlock(&rw);
	CHECK(52, "write lock, then no read lock",
	      pthread_rwlock_wrlock(&rw) == 0 && pthread_rwlock_tryrdlock(&rw) == EBUSY);
	pthread_rwlock_unlock(&rw);
	return 0;
}
