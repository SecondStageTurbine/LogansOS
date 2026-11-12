<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 409: Jarvis, a design read of the C3c-2 THINK (2026-11-12)

**MODEL** GPT-6-Luna, **EFFORT** high.
Why: this is a design read of userland threading (no kernel change and no new syscall), but it is SMP-touching, so it
gets a full-effort read.

- **Doc:** `docs/THINK-2026-11-12-ot61-c3c2-helper-threads.md` on `main` (merged from #112, `e17e1911`).
- **Context:** you reviewed C3c-1 (#107, `Plans/RETURN-2026-11-12-pr107-review-jarvis.md`).
- **PM decisions already made:** D1(a), SpiderMonkey's internal pool; D2(b), export `js::DisableExtraThreads` and add
  `mozjs_smoke_single`.
- **Code to read against:**
  - `third_party/ot61-mozjs/` (the vendored engine and patch 0003);
  - `userland/neo-crt/` (pthreads, `seqcond`, TLS keys);
  - the `mozjs` Rust bindings' `JSEngine::drop`.

Answer each item pass or fail, with the enforcing `file:line` (or upstream file and line). For any fail, give a
concrete scenario or input.

1. **CP table (§5):** for each of CP-14, 58, 84, 85, 106, 107 and 111, does the named guard exist in our code, and
   does the named discriminator actually catch the failure? Look for any CP class the table leaves out.
2. **Check 21 and its control:**
   - Is "helper condvar wakes ≥ 1" a sound proof that a task ran off the main thread? Look for wakes the pool makes
     for other reasons: spurious wakes, the notify-all at startup, lock hand-off.
   - §4.1's control says "`JSGC_PARALLEL_MARKING_ENABLED` and compaction off". The PM reads this as parallel marking
     *disabled*. Does that configuration, with a small heap, really give 0 wakes?
3. **CP-111:** `JSEngine::drop` calls `JS_ShutDown` (which joins the pool) while it holds `ENGINE_STATE`. Confirm that
   no path a helper thread runs can take `ENGINE_STATE` or any other lock the dropping thread holds.
4. **The C3c-1 gap:** C3c-1's smoke never runs `JS_ShutDown`. Do §3 item 3 and check 22 close that gap? Is anything
   else torn down at exit that C3c-1 never exercised?
5. **§4.3, the OOM and crash variants:** with helper threads alive, does neo-crt's `abort`/`exit` path, plus the
   kernel's CP-85 retire-all under `browser-runtime`, really leave `any_user_process()` false and every frame back?
   Name the code path.
6. **§8, the falsification tests:** is each one observable with the checks as written?

**RETURN:** `Plans/RETURN-2026-11-12-c3c2-think-read-jarvis.md` on `codex/c3c2-think-read-jarvis`. Push it.
