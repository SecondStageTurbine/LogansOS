<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur arc-2 Slice 3 — `neo-arthur` (the Arthur process; first conversation) → Janus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Janus (built Slices 1 & 2 + the 8.x inference arc — full context). New branch off `origin/main` HEAD: `codex/phase-8-arthur-slice-3-neo-arthur-janus`.
**Status:** DISPATCHED. **The build where Arthur first runs.** With 8.5 (real-model generation), Slice 1 (`neo-shell`), and Slice 2 (the action surface) all on `main`, the dependencies are met. This is the **conversation relay** — Arthur loads a model, receives NL, runs inference, replies. **NOT** action execution yet (that's Slice 4).

## 0. The decision this build makes
Stand up **`neo-arthur`**, the userland assistant process: load a model (`.neom`/GGUF via the importer), receive user text over the Slice-1 IPC channel, tokenize → run inference (the 8.5 generate path) → send the response back. This is the first time a human can **talk to Arthur** and get a model-generated reply, end-to-end, on-device. It is the conversation *relay* only — **no intent parsing, no action execution** (Slice 4), no capability escalation (Slice 5).

## 1. Authoritative design — RATIFIED (read first, build to it)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md   # §4 (Arthur app architecture), §6 Slice 3, §10 (ratified ODs)
git show origin/main:docs/VISION-arthur.md                                              # the destination
```
Reuse: **Slice 1** (`neo-shell` spawn + IPC relay + the scoped cap bundle), **Slice 2** (the action surface — *present but unused by Slice 3*; wired in Slice 4), **8.2** (GGUF import + BPE tokenizer), **8.5** (the generate path), **`libneo`**. No net-new kernel work, no net-new op math.

## 2. ISCs
- **ISC-1 — `neo-arthur` process.** New crate `userland/neo-arthur/` (links `libneo`): on spawn, receive its capability bundle (Handle(0)=conversation channel, Handle(1)=fs-read cap, Handle(2)=model cap) per the THINK §3D pattern; load the model (`SYS_MODEL_LOAD` / the importer path). Single process, single thread (per §4A).
- **ISC-2 — The conversation loop (relay only, §4B steps 1–3 + 5a).** `READ` user text from the conversation channel (`SYS_RECV`) → `TOKENIZE` (8.2 BPE) → `INFER` (the 8.5 generate path: token-in → logits → sample → next token, autoregressive) → `RESPOND` text back over the channel (`SYS_SEND`). **Stop at the text response** — do NOT detect/parse intents or execute actions (that's Slice 4's `5b`). Context as a userspace `Vec<u32>` sliding window (ratified OD-6), re-encoded per turn.
- **ISC-3 — `neo-shell` `arthur` command.** Wire the Slice-1 `arthur` builtin: it spawns `neo-arthur` with the scoped bundle and relays the conversation (shell stdin → Arthur, Arthur reply → shell stdout). The user types `arthur`, then converses.
- **ISC-4 — Standards + kill-switch.** Host-testable loop logic in a `*_logic` split (the tokenize→infer→respond wiring, with a stub/dummy model where the real model isn't available — the §6 Slice-3 "dummy mode" kill-switch: if inference is gated off, Arthur echoes input, proving the IPC relay + shell integration independent of the model). Behind the `ai-generate`/`gguf` gate; reuse Slice 1/2 + 8.2/8.5; `fmt`/`clippy -D warnings`; **no new `unsafe`/invariant; no kernel change** (uses existing syscalls). ANSPEC §11.x.

## 3. Gate
- **Host:** the conversation-loop logic unit-tested (tokenize→infer→respond wiring; the dummy-echo kill-switch path); builds on+off the feature; `fmt`/`clippy` clean.
- **QEMU scenario `arthur_converse`:** boot → `neo-shell` → type `arthur` → `neo-arthur` spawns, loads the (test) model → type a prompt → Arthur replies with model-generated text over the relay → exit. End-to-end NL conversation on real (QEMU) hardware at NEO_SMP=4. (If the real model is too heavy for the QEMU scenario, the scenario uses the tiny hand-laid `.neom` from the 8.1/8.5 KATs — the relay + generate path is what's under test, not model quality.)
- **Touches `kernel/` only via the scenario selector (like Slice 2) ⇒ run the pre-merge `workflow_dispatch` QEMU gate green before declaring close.** **Run the exact CI command** (`cargo clippy --workspace --all-targets -- -D warnings`, as CI does — not `-p <crate>`) before pushing; the new `neo-arthur` crate is a workspace member, so verify it doesn't leak a `std` dep into the `no_std` userland binaries (the benchmark-branch trap — `crypto_common`→`panic_impl`).

## 4. Scope guard
The `neo-arthur` process + the conversation relay (NL→infer→reply) + the `neo-shell arthur` command **only**. **Do NOT** build intent parsing or action execution (Slice 4 — the action surface stays present-but-unused here), capability escalation (Slice 5), multi-threading, or any kernel change. Reuse Slices 1/2 + 8.2/8.5. No new `unsafe`/invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; re-run host loop tests + the `arthur_converse` scenario; **pre-merge QEMU gate green**; **the exact `cargo clippy --workspace --all-targets` CI command green locally**; push (CI). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (Article 13 — **Arthur arc-2 Slice 3: Arthur converses — first human↔Arthur conversation on-device**). **This is the build where someone first talks to Arthur.** Next: Slice 4 (intent parsing + wiring the Slice-2 action surface — "model proposes, capability disposes" end-to-end).

**Disk reclaim (after PM confirms merge):** once on `origin/main` and PM-confirmed, delete this build's tree (`git worktree remove` / folder delete; never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-arthur-slice-3-neo-arthur-janus.md` (`git show origin/main:<path>`).
- **Branch:** `codex/phase-8-arthur-slice-3-neo-arthur-janus` off `origin/main` (has 8.5 + Slices 1/2).
- **One-line:** Build **`neo-arthur`** — the Arthur process, **Slice 3, where Arthur first runs** (read the RATIFIED design: `git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md` §4/§6/§10). A `userland/neo-arthur/` crate: receive its scoped cap bundle (conversation channel + fs-read + model cap), load the model, then the **conversation relay** — `SYS_RECV` user text → BPE tokenize (8.2) → infer (the 8.5 generate path) → `SYS_SEND` reply. **Relay only — NO intent parsing / NO action execution** (Slice 4; the Slice-2 action surface stays present-but-unused). Context = userspace `Vec<u32>` sliding window (OD-6). Wire the `neo-shell` `arthur` builtin (spawn + relay). Kill-switch: feature-off ⇒ dummy-echo mode (proves the IPC relay independent of inference). Gate: host loop tests + the `arthur_converse` QEMU scenario (type `arthur` → prompt → model-generated reply) at NEO_SMP=4 + **pre-merge QEMU gate green**. **Run the EXACT CI command `cargo clippy --workspace --all-targets -- -D warnings` (not `-p`) before pushing** — and verify the new crate doesn't leak a `std` dep into the no_std userland binaries (the benchmark trap). No new `unsafe`/invariant/kernel change. fmt/clippy clean.
