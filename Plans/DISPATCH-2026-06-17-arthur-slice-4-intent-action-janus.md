<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur arc-2 Slice 4 — intent parsing + capability-bounded action ("model proposes, capability disposes") → Janus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Janus (built Slices 1–3 + the inference arc — full context). New branch off `origin/main` HEAD: `codex/phase-8-arthur-slice-4-intent-action-janus`.
**Status:** DISPATCHED. **The build where Arthur acts.** Wire Slice 3's conversation relay to Slice 2's action surface so Arthur's model output drives **capability-checked** actions. This is Article 29.3's keystone made live — *the model proposes, the capability system disposes* — end-to-end. **All design decisions are ratified** (model + intent FINDINGS §6; Arthur THINK §10); build to them.

## 0. The decision this build makes
Extend the Slice-3 loop from "reply with text" (§4B step 5a) to "reply with text **OR** execute a bounded action" (step 5b): detect a tool-call in the model's output → parse the intent → run it through Slice 2's `execute_action()` (capability-checked) → report the result (or the denial). After this, a user can ask Arthur to *do* something and Arthur does it **iff** it holds the capability — visibly, revocably, auditably.

## 1. Authoritative design — RATIFIED (read first, build to it)
```
git fetch origin
git show origin/main:Plans/RESEARCH-2026-06-17-arthur-model-and-intent-FINDINGS.md   # §6 = ratified D1-D6 (model + intent format)
git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md   # §3 action surface, §4B/4C the loop+intent, §10
```
Ratified, binding: **model = Qwen2.5-1.5B-Instruct** (D1/D6; the 0.5B stays the bring-up/test model); **format = native Hermes tool-calling** (`<tool_call>…</tool_call>` + JSON — D6, OD-3); **renderer = userland** (D2); **dynamic cap-scoped tool defs** (D4); **lenient JSON + retry** (D3); **one action per turn** (D5). Reuse **Slice 2** (`libneo` `execute_action`/the 12-action catalog), **Slice 3** (the relay loop), **8.2** (tokenizer), **8.5** (generate).

## 2. ISCs (all userland — NO kernel change; OD-2 is moot)
- **ISC-1 — The chat-template renderer (userland, D2).** Build the model prompt in the Hermes format: system preamble + **the dynamic tool definitions** (D4 — emit a tool def ONLY for actions Arthur holds the capability for; the cap set shapes the tool menu) + the conversation window. The renderer is a string formatter (~the FINDINGS' ~200 LOC). Host-testable against a golden prompt.
- **ISC-2 — The intent parser (D3, lenient + retry).** After generate, scan output for the Hermes `<tool_call>` delimiters; parse the JSON to `{action, args}`. **Lenient:** validate required fields; on malformed/missing, **re-prompt the model with the error, up to 3× then report failure to the user** (D3). **⚠ no_std-safe JSON:** `neo-arthur` is a `no_std` userland binary — do **NOT** pull `serde_json`/a std-requiring JSON crate (the #243 `crypto_common`→`panic_impl` class). Hand-roll a minimal Hermes-tool-call extractor (or a `no_std`/`alloc`-only parser); **run `cargo clippy --workspace --all-targets -- -D warnings`** to prove the no_std binaries still build.
- **ISC-3 — Wire intent → `execute_action()` (the disposition, the keystone).** A parsed intent calls Slice 2's capability-checked `execute_action()`; a **granted** action runs and its result is formatted back ("I read the file: …"); an **ungranted** one returns `Denied` and Arthur reports it ("Denied: no WRITE capability for /…"). **The model never calls a syscall — it proposes; `execute_action` (capability-gated) disposes.** **One action per turn** (D5): execute one, report, then the model decides the next turn (no batch).
- **ISC-4 — Standards + kill-switch.** **Kill-switch:** the act-path behind a feature/flag — disabled ⇒ Slice-3 relay-only (Arthur talks, doesn't act), **bit-for-bit Slice 3** (the Rule-1 regression). Host-testable parser/renderer/wiring in a `*_logic` split, **with canned/deterministic model outputs** (a fixed Hermes tool-call string → parse → execute → assert granted-runs / ungranted-Denied — do not depend on real-model nondeterminism for the KAT). `fmt`/`clippy -D warnings`; **no new `unsafe`/invariant; no kernel change** (no new syscalls — OD-2 moot). ANSPEC §11.x.

## 3. Gate
- **Host:** the renderer golden test; the intent parser (valid tool-call → `{action,args}`; **malformed → retry path**; pure-text → no action); the intent→`execute_action` wiring with **every grant/deny path** (canned outputs); kill-switch ≡ Slice 3; builds on+off the feature.
- **QEMU scenario `arthur_act`:** boot → `arthur` → a prompt that yields a tool-call for a **granted** action (e.g. read a file Arthur was granted) → Arthur executes + reports the contents; a prompt that yields a tool-call for an **ungranted** action (e.g. write without the cap) → **Denied**, reported. The full propose→dispose→report loop at NEO_SMP=4. *(Use a deterministic/canned model tool-call output or the tiny test model with a pinned prompt — the loop is what's under test, not model quality.)*
- **Touches `kernel/` only via the scenario selector ⇒ run the pre-merge `workflow_dispatch` QEMU gate green; run the exact `cargo clippy --workspace --all-targets -- -D warnings` (the #243 / no_std-JSON guard) before pushing.**

## 4. Scope guard
Intent parsing + action execution + the chat-template renderer + dynamic cap-scoped tool defs **only**. **Do NOT** build on-demand capability escalation (Slice 5 — the grant-request-via-IPC), add new actions (use Slice 2's 12), change the model integration (use the 8.5 generate path), pull a std JSON crate, or make any kernel change. Reuse Slices 2/3 + 8.2/8.5. No new `unsafe`/invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; host renderer/parser/wiring tests (incl. every grant/deny + the retry path) green; the `arthur_act` scenario green; **pre-merge QEMU gate green**; **`cargo clippy --workspace --all-targets` green (no_std-JSON verified)**; push (CI). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (Article 13 — **Arthur arc-2 Slice 4: "model proposes, capability disposes" live — Arthur acts, bounded**). This is the build where Arthur's keystone (Article 29.3) becomes real. Next: Slice 5 (on-demand cap escalation) + the physical-input path (OD-1 framebuffer keyboard) for "talk to Arthur at the machine."

**Disk reclaim (after PM confirms merge):** once on `origin/main` and PM-confirmed, delete this build's tree (`git worktree remove` / folder delete; never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-arthur-slice-4-intent-action-janus.md` (`git show origin/main:<path>`).
- **Branch:** `codex/phase-8-arthur-slice-4-intent-action-janus` off `origin/main` (has Slices 1–3 + 8.5).
- **One-line:** Build **Slice 4 — Arthur acts** ("model proposes, capability disposes" end-to-end). Read the RATIFIED design (FINDINGS §6 = D1–D6; Arthur THINK §3/§4/§10). Extend the Slice-3 loop: after generate, **detect the Hermes `<tool_call>`** in the output → **parse the JSON** to `{action,args}` (D3 **lenient + retry** 3× then report) → call **Slice 2's `execute_action()`** (capability-checked) → report the result, or **`Denied`** if Arthur lacks the cap. Build the **userland Hermes chat-template renderer** (D2) with **dynamic cap-scoped tool defs** (D4 — only offer tools Arthur holds the cap for). **One action per turn** (D5). **All userland, NO kernel change** (OD-2 moot). **⚠ no_std-JSON:** `neo-arthur` is a no_std binary — hand-roll a minimal Hermes/JSON extractor, do NOT pull `serde_json`/std (the #243 trap); **run `cargo clippy --workspace --all-targets -- -D warnings` to verify the no_std binaries still build.** Kill-switch: act-path off ⇒ Slice-3 relay-only bit-for-bit. Gate: host tests (renderer golden + parser incl. retry + every grant/deny path, **canned model outputs** — not real-model nondeterminism) + the `arthur_act` QEMU scenario (granted action executes / ungranted Denied) at NEO_SMP=4 + **pre-merge QEMU gate green**. No new `unsafe`. **Scope: intent+action only — NO Slice-5 escalation, NO new actions, NO kernel change.**
