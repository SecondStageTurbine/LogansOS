<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #46): review of PR #108, the Arthur display fixes → Jarvis

**MODEL:** GPT-6-Luna. **EFFORT:** high. This is contained code: Arthur's userland reply path and footer, plus the
xtask checks and the WM's boot-test witness. It changes no kernel, syscall or ABI.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `821dd634` (pinned; PR #108, branch
`claude/arthur-strip-stop-token`, base `main` `436674dd`). Claude-authored, so the review must come from a different
model family.

**What it fixes (Zenbook image A acceptance, 2026-10-23):**
1. The reply showed `<|im_end|>`.
2. The footer read `neo> arthur · TEXT surface · cap-gated · IBM Plex`. It now reads
   `<model> · runs on this machine · Esc to go back`, and the name comes from the model actually loaded.
3. Answers stopped at 32 tokens. On the desktop the cap is now 256 tokens and 4096 bytes, and a capped reply ends
   with ` [answer cut short]`.

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/arthur-display-review-jarvis
          git fetch origin && git checkout -B codex/arthur-display-review-jarvis 821dd634

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-arthur-display-review-jarvis.md   # this file
git diff 436674dd 821dd634 --stat
git diff 436674dd 821dd634 -- userland tools
```

RAG (semantic code search) is available. Use it for "where is X done?" questions; grep is better for exact symbols.
Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in the RETURN and use
grep/Read; the PL manages the RAG. Example query: `$body=@{query='sys_generate max_new KV cache capacity'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input.

**Stop token (`userland/neo-arthur/src/vocab_sidecar.rs`, `main.rs` decode path)**
1. `control_ids` holds every vocab entry spelled `<|name|>` with a non-empty name, plus the sidecar's EOS. It is
   sorted and de-duplicated before `binary_search`.
2. `reply_tokens` returns the tokens before the first control ID. When there is none it returns all of them. A
   reply made of only a control token decodes to an empty string without an error.
3. The decode in `ConversationBackend` uses `reply_tokens`, so no `<|...|>` text reaches the rendered reply or the
   serial `reply="` line.
4. `reply_was_cut` is true only when the token count reached the cap and no control token is present. A reply that
   ends on `<|im_end|>` exactly at the cap is not marked.
5. **Cap parity.** At `cfb995ed` the decode checked the cut against `STAGE_B_MAX_NEW_TOKENS` while the
   qwen35 smoke asked for 32, so a cut answer could go unmarked. `821dd634` adds `REPLY_MAX_NEW_TOKENS`, used for
   both the `sys_generate` request and `reply_was_cut`. Check that every feature combination that builds defines it
   exactly once with the right value (qwen35: 32; desktop: 256; other real-vocab: 32), and that the marker can't
   be wrongly added to, or wrongly left off, a reply in any of them.

**Reply length (`main.rs`, `conversation_logic.rs`)**
6. `buf[..max_new]` is in bounds for every feature combination that builds (`max_new <= STAGE_B_MAX_NEW_TOKENS`).
7. The kernel's `sys_generate` accepts `max_new = 256`. With `DEFAULT_CONTEXT_WINDOW_TOKENS` = 512, a full-length
   prompt plus 256 new tokens fits the KV cache, or is clamped or refused cleanly with no overrun. Cite the kernel
   check.
8. `MAX_REPLY_BYTES = 4096` applies only under `desktop-session`. No fixed 256-byte reader (console relay, shell
   channel) can receive a desktop reply. Cite where each reply consumer is gated.
9. 256 tokens plus `REPLY_CUT_MARKER` that exceed 4096 bytes are truncated or refused cleanly (no panic, no
   overflow). Cite the bound check, for example `push_memory_reply`.

**Footer (`showcase_logic.rs`, `showcase_text.rs`, `showcase_surface.rs`)**
10. `footer_model` returns `NoModel` when no model is ready or loaded. A name only ever comes from `model_path`,
    which is set only on a successful load. There is no hard-coded default name.
11. `model_display_name("/neofs/model-qwen2.5-0.5b.neom")` is `Qwen2.5 0.5B`. No input path can panic it, including
    non-ASCII, empty, `/`, `model-.neom` and `-`.
12. `footer_text` always terminates and never panics on a multi-byte name. The text it returns fits the measured
    width at the narrowest surface (320 px), or is `Esc to go back`.
13. `set_footer` redraws and damages only the band below the footer rule. The transcript's pixels and damage are
    untouched.
14. `commit_footer` commits only the footer's damage, with a serial greater than every earlier serial. It runs after
    the session handshake, and its error path (`return 63`) doesn't strand the session. Transcript damage still
    pending is committed by the next ordinary commit and not lost. Footer damage is never committed twice in a way
    the WM rejects.

**Checks and witnesses (`neo-init/src/desktop_session.rs`, `tools/xtask/src/main.rs`)**
15. `is_footer_band` classifies only commits wholly below the footer rule. The first keystroke's band commit is still
    what `log_damage` (`first=`) records. `ARTHUR_FOOTER_RULE_FROM_BOTTOM` (36) matches `showcase_text`'s footer
    geometry.
16. The xtask checks fail when they should:
    - a missing footer commit;
    - `band=no`;
    - `ARTHUR footer model=none` missing when no model is installed;
    - the expected `model="Qwen2.5 0.5B"` line missing in the real-model scenario;
    - any `reply="` line containing `<|`.
17. The host tests in `tools/arthur-logic-tests/src/showcase_s2_tests.rs` and `vocab_sidecar.rs` cover:
    - the control-token cut;
    - the cut marker;
    - display names;
    - footer fitting.
    Spot-check each test for a constant that would hide a wrong answer.

**Scope**
18. `git diff 436674dd 821dd634 --stat` touches no `kernel/` file and no syscall or ABI struct.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-arthur-display-review-jarvis.md` on your branch and **push it**, with a verdict:
  PASS, or FAIL with the item numbers. Paste the pushed SHA to the PL.
- No build is needed, though `cargo test -p arthur-logic-tests` is welcome evidence. **Never write the Zenbook's
  disk.**
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
