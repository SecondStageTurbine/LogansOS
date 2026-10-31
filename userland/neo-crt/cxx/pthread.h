/* SPDX-License-Identifier: MPL-2.0 */
/*
 * OT-61 C3b: musl's <pthread.h> plus the one call libc++ needs that musl
 * 1.2.5 doesn't declare. Only C++ consumers see this directory (it is first
 * on their include path, ahead of musl's headers).
 */
#ifndef NEO_CRT_PTHREAD_H
#define NEO_CRT_PTHREAD_H

#include_next <pthread.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Wait on `cond` until `abstime` on `clock` (glibc 2.30, POSIX 2024).
 * neo-crt accepts CLOCK_MONOTONIC only (src/pthread.rs). */
int pthread_cond_clockwait(pthread_cond_t *__restrict, pthread_mutex_t *__restrict, clockid_t,
                           const struct timespec *__restrict);

#ifdef __cplusplus
}
#endif

#endif
