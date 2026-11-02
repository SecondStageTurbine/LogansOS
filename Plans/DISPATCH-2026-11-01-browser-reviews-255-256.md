<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 255-256: cross-family reviews of the browser's base (#2 neo-crt, #4 Stylo)

**MODEL** GPT-6-Luna, **EFFORT** high: contained userland and vendored code; the kernel changes are test wiring only.

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. Most of each diff is vendored upstream code under
`third_party/`: spot-check it, and read every first-party file in full. These unblock the browser's rendering stack
(#22, #25-#28), so be thorough but don't stall on upstream style.

## Section 255: Jarvis, #2 (OT-61 C3a: neo-crt, musl + libc++ over our std)

`git fetch origin +pull/2/head:pr2 && git diff origin/main...pr2` (head `32fcddf9` when queued; name the head).

1. What was vendored from musl and libc++, at which upstream versions/commits, and are their licence files kept
   (MIT; Apache-2.0 WITH LLVM-exception)? Any GPL/LGPL code?
2. Every syscall the C runtime makes: list them; does it add a syscall or change an ABI?
3. `userland/neo-crt/c/neo_bottom.c` and the TLS/stdio/malloc glue: bounds checked, no undefined behaviour, errno
   correct, and memory from our allocator freed through the same allocator.
4. Any `build.rs`: no network, no writes outside `OUT_DIR`, no running downloaded binaries; clang is pinned.
5. The default and product images are unchanged (the browser bundle is opt-in).
6. The prior-art survey (licences, last-commit dates) is present and holds.

RETURN: `Plans/RETURN-2026-11-01-pr2-neo-crt-review-jarvis.md`, branch `codex/pr2-review-jarvis`.

## Section 256: Chad, #4 (OT-61 C2.4: the Stylo closure, html5ever + Stylo)

`git fetch origin +pull/4/head:pr4 && git diff origin/main...pr4` (head `f14faff0` when queued; name the head).

1. Every vendored crate records a compatible licence (MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unicode-3.0,
   Unlicense/CC0), its upstream URL and version. Any GPL/LGPL/AGPL? (`xtask vendor-verify` from #1 should pass; run it.)
2. Build scripts: Stylo's property generator runs host `python3` (allowed); confirm it fetches nothing over the
   network and writes only to `OUT_DIR`.
3. `stylo_smoke`: does its oracle check computed values derived by hand (not by the code under test), and does it
   cover cascade order, inheritance and a `!important` case?
4. Memory: the 8 MiB style thread and the page size limits: what bounds a hostile or huge page?
5. The default and product images are unchanged.

RETURN: `Plans/RETURN-2026-11-01-pr4-stylo-review-chad.md`, branch `codex/pr4-review-chad`.
