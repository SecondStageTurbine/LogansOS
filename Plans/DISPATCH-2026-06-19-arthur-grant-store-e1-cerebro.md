<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur grant ergonomics — Slice E1 (grant store + D7 default) → Cerebro

**Date:** 2026-06-19
**From:** PM (Claude)
**To:** Cerebro. New branch off `origin/main` HEAD (`59b94a7` or later): `codex/arthur-grant-store-e1-cerebro`.
**Status:** DISPATCHED. **The first ratified OT-18 slice (D1/D2/D4/D7).** Arthur persists + reloads its *coarse durable grant set* across sessions, and ships a least-privilege default. Userland (Article 29.4). **Independent of OT-19** — E1 is the grant *infrastructure*; only the *width* of default grants depends on OT-19 (D5 sequencing guard), so this slice ships narrow.

## 0. The decision this build makes
Turn "Arthur starts blank every session" into "**Arthur reloads the scopes you durably granted it, and boots with a minimal sensible default.**" A `.arthur/grants.json` store records durable grants (audit/persistence); at Arthur spawn the shell re-derives the corresponding **narrow kernel caps** and delegates them via `ipc::grant`; D7 default = FileRead on `.arthur/` only. **The kernel cap is the enforcement boundary by construction (Article 29.3) — `grants.json` is a UX/persistence/audit record, NEVER the authority.** A compromised Arthur that ignores `grants.json` cannot exceed its held kernel caps.

## 1. References
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md   # the RATIFIED design — D1-D7, §11.1 build slices (E1 is yours), §12 risks
git show origin/main:userland/libneo/src/action.rs                              # Slice-4 action surface: ActionKind/Grant/RequiredCapability, execute_action() cap-check, ActionError::Denied
git show origin/main:userland/neo-shell/src/main.rs                             # Arthur spawn (handle inheritance via sys_spawn) — where the re-derived caps are delegated
# capability primitives (Phase 2, proven): process::derive_handle / revoke_handle / ipc::grant @ kernel/src/sched/process.rs
git show origin/main:Plans/THINK-2026-06-18-arthur-memory-substrate-design.md   # OT-17 — .arthur/ dir layout + the .mem schema's human-readable/byte-oracle discipline E1's format mirrors
```
**This dispatch is the build of the THINK's §11.1 Slice E1 only.** D2's six scopes (FileRead/FileWrite/Process/Network/Model/Memory) are the taxonomy; E1 stores+reloads them and wires the FileRead default. E2 (on-miss re-request), E3 (audit/revoke), E4 (memory-scope) are LATER slices — not this build.

## 2. ISCs
- **ISC-1 — The grant-store format + an independent byte/text oracle.** Define `.arthur/grants.json`: a versioned, human-readable JSON record of durable grants — each entry `{ version, scope, resource (path/host:port/binary/category), rights, granted_by }` (the `granted_by` field is the OT-20 seam — present, single-Arthur today). **Format rule (CLAUDE.md "Format/ABI specs require an independent byte oracle"):** ship a **hand-authored canonical `grants.json` fixture** with known contents, asserted field-by-field against the *loader* — NOT only a writer→reader round-trip (a writer+reader built together agree even when both mis-implement the spec). The hand-laid example is the oracle.
- **ISC-2 — Load + re-derive at spawn (the authority-boundary-correct path).** At Arthur spawn, neo-shell reads `grants.json`, and **for each durable grant re-derives a narrow kernel cap** (`derive_handle` scoped to the resource) and delegates it to Arthur (`ipc::grant`) — then builds the in-process `Grant` set for `execute_action()`. **The `Grant` set is the userspace policy mirror; the kernel caps Arthur holds are the boundary.** Do NOT make `execute_action()` trust `grants.json` over the held cap — the syscall must still fail EPERM if the kernel cap is absent regardless of the file (assert this in a test: a `Grant` present in the set but with NO backing kernel cap ⇒ the action still fails at the kernel).
- **ISC-3 — D7 least-privilege default.** First-boot / no-store / store-disabled ⇒ Arthur gets **only** FileRead on `.arthur/` (so it can read its own grants + OT-17 memories) + the spawn-inherited model handle. No write, no network, no process mgmt, no other-category memory. **Interim-narrow (D5 guard):** because OT-19 reversibility has not shipped, the *default* file scope stays `.arthur/`-only — do NOT widen file defaults in E1.
- **ISC-4 — Standards + kill-switch.** **Kill-switch `arthur-grants` (or extend the Arthur feature) default-OFF ⇒ no store read/write, Arthur starts with only spawn-inherited caps ≡ today bit-for-bit.** Parsing + grant-set construction in a host-testable `*_logic` split (no_std-clean if it lands in libneo). No new `unsafe`. **No new invariant** (it composes existing cap primitives — state this explicitly in the RETURN; if you find you need one, STOP and flag it). SPDX on new files. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker. ANSPEC/ADR note on the grant-store format + the authority-boundary statement.

## 3. Gate
- **Host:** the hand-laid `grants.json` oracle (field-by-field) + malformed/missing-store → D7 default (no crash, no leak — degrade like OT-17's fail-safe) + the **boundary test** (Grant-in-set-without-kernel-cap ⇒ action denied at kernel) + builds on+off the feature.
- **QEMU scenario `arthur_grants_persist`** (if Arthur spawn is exercised in QEMU): grant a FileRead scope → write store → fresh Arthur spawn re-derives the cap + reads the in-scope file; off ⇒ no persistence (D7 default only); corrupt store ⇒ D7 default, no crash. At **NEO_SMP=4**. *If this is purely userland and host-coverable, state why QEMU isn't needed (the 9.x host-only precedent) — but if Arthur spawn/`ipc::grant` is kernel-touching, the pre-merge QEMU gate is mandatory.*
- Kernel-touching ⇒ pre-merge `workflow_dispatch` QEMU gate green (all 4 shards) + clippy via Docker.

## 4. Scope guard
Grant-store format + load/re-derive-at-spawn + D7 default **only**. **Do NOT** build E2 (on-miss re-request), E3 (audit/revoke surface), E4 (memory-scope integration), any new authority primitive, any policy language (D2-C rejected), or widen the file default past `.arthur/` (D5 — OT-19 not shipped). Reuse `derive_handle`/`ipc::grant`/`execute_action`. No new invariant/`unsafe`.

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main`, the exact CI host-check set, SPDX, self-stat). Two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL + the QEMU `workflow_dispatch` URL if kernel-touching), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (OT-18 E1 landed; E2 next). 

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-19-arthur-grant-store-e1-cerebro.md` (and the ratified design `…THINK-2026-06-18-arthur-grant-ergonomics-design.md`). **Branch:** `codex/arthur-grant-store-e1-cerebro` off `origin/main`.
- **One-line:** Build **OT-18 Slice E1 — the Arthur grant store**: a versioned human-readable `.arthur/grants.json` recording durable coarse grants (D2's six scopes; `granted_by` = OT-20 seam), loaded at Arthur spawn where neo-shell **re-derives a narrow kernel cap per durable grant** (`derive_handle` + `ipc::grant`) and builds the `execute_action()` `Grant` set. **The kernel cap is the boundary (29.3); `grants.json` is UX/persistence/audit, NEVER authority** — prove it with a test: a `Grant` in the set with no backing kernel cap still fails EPERM at the kernel. **D7 default = FileRead on `.arthur/` only** (read its own grants + OT-17 memories) + spawn-inherited model handle; **stay narrow — OT-19 hasn't shipped, so don't widen file defaults (D5 guard).** **Format byte/text oracle: a hand-authored canonical `grants.json` asserted field-by-field against the loader (NOT round-trip).** **Kill-switch `arthur-grants` default-off ⇒ only spawn-inherited caps ≡ today.** No new invariant/`unsafe`. **Scope: store + load/re-derive + D7 default ONLY — NOT E2 re-request / E3 audit-revoke / E4 memory-scope.** **Close-gate: run `scripts/worker-close-gate.sh`; RETURN pastes the green CI URLs + confirms no-new-invariant.**
