# DISPATCH: QEMU suite red — identify the failing scenario(s), real-regression vs load-flake, fix → (PL assigns: Jarvis or Janus)

**Date:** 2026-06-15
**From:** PM (Claude)
**To:** a free main coder (Jarvis or Janus — PL assigns). New branch off `origin/main` HEAD (`8eeb3f8`, build-green): `codex/qemu-suite-red-diagnosis-<coder>`.
**Status:** DISPATCHED. **Host build is GREEN (the virtio-accel break is fixed). This is the QEMU integration suite going red on the first full run in weeks.**

## 0. The situation
After weeks of `[skip ci]`/runner-down, the full QEMU suite finally ran on `main` (`8eeb3f8`). Result: **host-checks GREEN**, **shard 4/4 PASSED**, **shards 1/2/3 FAILED** at the `Run QEMU integration tests` step (~5–7 min in). The visible serial logs show scenarios *passing* (e.g. `vtd_fence event=PASS`), so it is **specific scenario(s) failing/timing-out, not a wholesale break.** Cause unknown: could be a real regression from a Phase-8 merge that landed without full QEMU validation (8.1 decode loop, 8.4a/8.5a ops, BUILD-LO latency oracle), or **load-induced flake** — the CI host (the PL's gaming PC) is simultaneously running ~15 other Docker containers (qdrant×5, open-webui, postgres, …), which can starve a timing-sensitive scenario.

## 1. Step one — NAME the failing scenarios (don't guess)
The run uploaded the serial logs as an artifact: **`neo-test-serial-logs`, run `27574228789`, artifact ID `7652950276`** (54 files). Pull it, **or** reproduce locally:
```
git fetch origin && git checkout 8eeb3f8
cargo neo-test --shard 1/4    # then 2/4, 3/4 — the failing shards
```
The harness prints a per-scenario PASS/FAIL summary — **identify the exact scenario name(s)** that fail (and whether a scenario HANGS/times out vs asserts a wrong value). That name drives everything. Report it before fixing.

## 2. Step two — the discriminator: real regression vs load-flake (REQUIRED)
Run the identified failing scenario(s) **both ways**:
- **(a) Unloaded:** stop the other containers (or run on a quiet box) and re-run the scenario several times.
- **(b) Loaded:** with the other containers running, re-run several times.

Map the observation to the cause:
- **Passes unloaded, fails loaded / intermittently → load-induced flake.** Fix = make the scenario robust to a slow host: raise the QEMU/test timeout, replace wall-clock assumptions with deterministic waits/breadcrumbs, or mark it serial. Do **not** weaken what it asserts — make its *timing* robust, not its *check*.
- **Fails deterministically both ways → real regression.** Bisect across the Phase-8 merges (8.1 `generate`, 8.4a/8.5a ops, BUILD-LO oracle, the virtio OpKind additions) to the commit that introduced it; fix the root cause. The new `OpKind` variants (Silu/Swiglu/Rope/GqaAttention) are a prime suspect if a scenario exercises the accel/op path and hits a `todo!()`/`unsupported` arm at runtime.

## 3. Gate
- The failing scenario(s) named, cause classified (flake vs regression) with the both-ways evidence, and fixed so **all 4 shards pass on the loaded CI host** (that's the real environment — a fix that only passes unloaded is incomplete; harden it).
- `fmt`/`clippy` clean; host-checks stays green; no weakening of any scenario's assertion. If the fix is "raise a timeout / serialize," say so explicitly and justify the new bound.
- Note in the RETURN whether this argues for **not running CI while the box is loaded** (a runner-hygiene finding for the PL).

## 4. Scope guard
Diagnose + fix the failing QEMU scenario(s) only. Do **not** refactor the harness, touch unrelated scenarios, or merge other lanes. If it's a real regression in a Phase-8 op/scenario, fix the scenario/op — coordinate with PM before reverting any merged feature.

## 5. Merge (PM)
Rebase onto current `main`; full QEMU suite green on the loaded host; PM `pm-merge-check` → `--ff-only` → ledger + audit-log note (this is the debt the `[skip ci]` era hid — record it).

---

## Relay — what to tell the coder
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-15-qemu-suite-red-diagnosis.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-qemu-suite-red-diagnosis.md
  ```
- **Branch:** `codex/qemu-suite-red-diagnosis-<you>` off `origin/main` (`8eeb3f8`).
- **One-line:** Full QEMU suite ran for the first time in weeks → **shards 1/2/3 fail, shard 4 passes, host-checks green** (so build's fine; it's scenario-specific). **First: name the exact failing scenario(s)** — pull artifact `7652950276` from run `27574228789`, or `cargo neo-test --shard {1,2,3}/4` locally; the harness prints which scenario fails. **Then run it loaded vs unloaded** (the box also runs ~15 other containers): passes-unloaded ⇒ load-flake (harden timing, don't weaken the check); fails-both ⇒ real regression, bisect the Phase-8 merges (suspect the new `OpKind` arms). Fix so **all 4 shards pass on the loaded host**. Report the scenario name + flake-vs-regression verdict before fixing. No assertion-weakening.
