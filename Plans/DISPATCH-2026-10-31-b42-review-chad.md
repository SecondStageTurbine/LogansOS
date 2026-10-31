<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Section 219: Chad, review of #249 and #261 (batch 42, the browser lane, run by the PC relay runner)

**MODEL** GPT-6-Luna, **EFFORT** high: contained userland and vendored code, with no kernel change beyond test wiring.

Fetch with `git fetch origin +pull/249/head:pr249 +pull/261/head:pr261` and read `git diff origin/main...pr249`
and `git diff origin/main...pr261`. Most of each diff is vendored code under `third_party/`; spot-check it. Answer as
a plain checklist: PASS or FAIL, the enforcing `file:line`, and a concrete input for any FAIL.

1. **#249 (OT-61 C2.4, the Stylo closure, head `ff3e112f`):** does every vendored crate record a compatible license
   (MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unlicense/CC0), its upstream URL and its version? Is there any GPL,
   LGPL or AGPL code?
2. **#249:** does any `build.rs` do something unexpected on the build host (network, writes outside `OUT_DIR`,
   running downloaded binaries)?
3. **#261 (OT-61 C3a, neo-crt, head `db5caa24`):** what was vendored from musl and libc++, and are their license
   files kept (MIT; Apache-2.0 WITH LLVM-exception)?
4. **#261:** list each syscall the C runtime uses. Does it add a syscall or change any ABI?
5. **#261:** in `userland/neo-crt/c/neo_bottom.c` and the pthread/TLS overrides: are bounds checked, are there any
   undefined-behaviour cases, and are futex wait and wake correct (no lost wakeup)?
6. Both: are the default and product builds unchanged (the ladder is opt-in)?
7. Both: is the prior-art section present (the survey with licenses and last-commit dates, trap mining, CP rows for
   the thread code), and do its claims hold?

**Delivery:** commit `Plans/RETURN-2026-10-31-b42-browser-review-chad.md` on a new branch
`codex/b42-browser-review-chad` off `origin/main`. Push it and reply with the commit SHA. Never push to main.
