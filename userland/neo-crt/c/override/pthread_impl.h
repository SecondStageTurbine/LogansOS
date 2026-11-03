/* SPDX-License-Identifier: MPL-2.0 */
/*
 * OT-61 C3a: replaces musl's src/internal/pthread_impl.h for the musl files
 * neo-crt compiles. Those files reach thread state only for the current
 * locale (locale_impl.h: CURRENT_LOCALE, MB_CUR_MAX). neo-crt has one locale,
 * the global C locale, for every thread (uselocale is not compiled), so
 * __pthread_self() returns one shared, read-only record. Its tid is 0, which
 * no FILE lock ever holds. errno lives in Rust
 * (per-thread, src/lib.rs), not here.
 *
 * Anything else from the real header (tid, TSD, cancellation, robust lists,
 * futex helpers) is absent on purpose: a musl file that needs it fails to
 * compile here instead of silently reaching Linux.
 */
#ifndef _PTHREAD_IMPL_H
#define _PTHREAD_IMPL_H

#include <pthread.h>
#include <errno.h>
#include <limits.h>
#include "libc.h"
#include "atomic.h"

#define pthread __pthread

struct pthread {
	int tid;
	locale_t locale;
};

hidden struct pthread *__neo_crt_self(void);
#define __pthread_self() __neo_crt_self()

/* Only reachable through a FILE with lock >= 0, and neo-crt makes none
 * (c/neo_bottom.c): it aborts. */
hidden void __wake(volatile void *, int, int);

#endif
