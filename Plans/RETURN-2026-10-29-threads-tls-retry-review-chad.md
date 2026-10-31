<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay #166, threads_tls step-6 retry (#207)

Reviewed exact test-only diff `d422dfa0`; citations at that squash. Requested GPT-6.1-Sol/high, same session throughout b18; runtime setting not independently verifiable. Worktree `E:\AI Automation\workers\chad`, RAG healthy. **PASS.**

- **1 PASS - bounded and specific.** `userland/neo-init/src/main.rs:2803` sets 16 attempts. The loop at :2821 reloads the selector each time; only Ok([_,_,msr]) with msr==base retries (:2828). Zero is success and immediately breaks; another value or syscall error returns 6 immediately. Sixteen B observations leave zeroed=false and return 6 (:2832). No unconditional polling was added.
- **2 PASS - dead selector still yields 8006.** The initial set_tls(base) establishes B. If load_user_fs_selector becomes a no-op, every probe sees B, all 16 attempts exhaust, selector_check returns 6. The battery's :3050 maps that to 8000+6. A timer restore can cause retries on a correct kernel, but cannot turn a never-zeroing load into success.
- **3 PASS - discriminators retained.** The setter is still called with the already-recorded base and checked by fs_is (:2836); the second selector load, explicit sleep and same-CPU restored-base checks remain (:2843-2850). Thus setter caching and switch-in caching still fail their distinct later checks. The only broadened observation is the known switch-in value in step 6, with a hard retry cap. Production TLS paths are untouched.

Validation: exact diff and enclosing selector_check/battery source, including error-code mapping. No fresh hardware/QEMU run is claimed; this review does not replace the batch's recorded gate.
