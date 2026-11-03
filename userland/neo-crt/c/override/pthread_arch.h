/* SPDX-License-Identifier: MPL-2.0 */
/*
 * OT-61 C3a tripwire. musl's real src/internal/pthread_impl.h includes this
 * header to read the thread pointer from %fs. neo-crt replaces
 * pthread_impl.h (override/pthread_impl.h); if any musl file reaches the real
 * one anyway, it finds this file first on the include path and stops here.
 */
#error "neo-crt: musl's real pthread_impl.h was included; see c/override/pthread_impl.h"
