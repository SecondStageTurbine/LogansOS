<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-41 — console gate honesty + the read-byte busy-spin (cheap follow-ons) → Jarvis

**Date:** 2026-06-29
**From:** PM (Claude)
**To:** Jarvis (main-tier). New branch off `origin/main` HEAD: `codex/ot41-console-honesty-jarvis`.
**Status:** DISPATCHED — the OT-41 console AAG carry-forwards (interactive console merged `c50436b`). Small, self-contained, **non-conflicting** with Cerebro's WiFi surface and with Janus's agentic lane (this is the console gate's *honesty* + a userland read primitive, not the agentic grammar). **Kernel/test-harness-touching ⇒ 4-shard QEMU gate + AAG** (small surface, but the gate's pass-string is what changes).

## 0. The decision this build makes
Make the `arthur_console` gate **assert exactly what it proves, no more**, and remove a userland busy-spin. The AAG found the gate's pass-string **over-asserts**: it claims `arthur_reply_framebuffer=ok` but the framebuffer check is a bare bright-pixel count (≥64) and the `screendump` is captured one step *before* the keys are injected — so green means "non-blank fb + serial reply," **not** "the reply text is rendered on screen." An over-asserting gate is a green that lies; honesty here is the same property as the byte-oracle.

## 1. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md                     # OT-41 — the three nits + what the gate actually proves vs over-asserts
git show origin/main:kernel/src/test_harness.rs              # :1512 the over-asserting pass-string (arthur_reply_framebuffer=ok …)
git show origin/main:tools/xtask/src/main.rs                 # :5021 the `screendump` HMP command — captured before the keys (the ordering bug)
git show origin/main:userland/neo-arthur/src/main.rs         # :497/:520 console_read_byte() — busy-spins on EAGAIN, no yield
```

## 2. ISCs
- **ISC-1 — Relabel the pass-string to what's verified (honesty).** In `test_harness.rs:1512`, replace the over-asserting `arthur_reply_framebuffer=ok` with what the check actually proves — **either** `framebuffer_nonblank=ok serial_reply=ok` (honest relabel) **OR** strengthen the fb assertion to check the **reply region** (not a global bright-pixel count) so the original label becomes true. Recommend the relabel unless the region check is cheap; the definitive "reply legible on screen" is the PL's bare-metal console run (QEMU PPM glyph-assertion is brittle — the CB-1b "device-less QEMU proves logic, bare-metal proves the pixels" pattern). State which you chose and why.
- **ISC-2 — Fix the screendump-before-keys ordering.** In `xtask/src/main.rs` (~:5021), order the `screendump` capture **after** the keys are injected + the reply renders, so the capture doesn't lag a step behind the state it's meant to witness. If the relabel (ISC-1) drops the fb-content claim entirely, this is still worth fixing so any future region check captures the right frame.
- **ISC-3 — `console_read_byte`: blocking read / yield, not a busy-spin.** `userland/neo-arthur/src/main.rs:520` busy-spins on `EAGAIN` (no `yield` syscall in libneo). Add a blocking read (or a `yield`/sleep) so the console doesn't peg a CPU waiting for a keystroke. Keep it minimal — a libneo `yield` wrapper if one's missing, or the existing block primitive.

## 3. Standing-rule sections
- **Kill-switch / invariant budget:** N/A new — this is gate-honesty + a read primitive, no policy/substrate change, **no new invariant**, no behavior change to the *shipped* console path beyond the read-yield. The `arthur_console` kill-switch (default-off ≡ prior serial relay) is untouched.
- **I-14:** inherited from the console build (HID poll-not-ISR, fenced xHCI, fb=MMIO); ISC-3 only changes how the userland read waits — no ISR/DMA/governance lock.
- **Scope guard:** the three nits ONLY. **Do NOT:** add a brittle PPM glyph-assertion (that's the bare-metal visual's job), change the agentic grammar (Janus's A2 lane), touch the forward math or connectivity, or expand the console feature set.

## 4. Gate + close
- **Kernel/test-harness-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` workflow_dispatch + AAG** (the gate's pass-string + a userland path change — let QEMU re-prove the console scenario with the corrected assertion). Push-CI green. **Green without the CI URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase, re-run, push ff-clean; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the chosen ISC-1 approach (relabel vs region-check) + the new pass-string, and confirmation the console scenario still passes with the honest assertion. **Disk reclaim** after merge. **On merge, OT-41 (2)+(3) close**; the "reply legible on screen" proof remains the PL's bare-metal console run.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-29-ot41-console-honesty-jarvis.md` (+ OT-41 in OPEN-THREADS, the three code sites). **Branch:** `codex/ot41-console-honesty-jarvis` off `origin/main` HEAD.
- **One-line:** **Make the console gate assert exactly what it proves, and stop the read busy-spin.** (1) Relabel `arthur_reply_framebuffer=ok` (`test_harness.rs:1512`) to `framebuffer_nonblank=ok serial_reply=ok` — OR strengthen the fb check to the reply region — since green currently means non-blank-fb + serial-reply, not glyphs-on-screen. (2) Move the `screendump` capture (`xtask main.rs ~:5021`) to **after** the keys inject. (3) Give `console_read_byte` (`neo-arthur/main.rs:520`) a blocking read / yield instead of an `EAGAIN` busy-spin. **Scope: the three nits only** — no brittle glyph-assertion (bare-metal's job), no agentic-grammar/forward/connectivity touch. **Gate:** 4-shard QEMU + AAG (the pass-string changes — re-prove the console scenario). Close-gate; RETURN shard URLs + SHA + the new pass-string. On-screen legibility stays the PL's bare-metal console run.
