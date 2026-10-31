<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Arthur's philosophy of disagreement + reversibility mechanism (OT-19)

**Date:** 2026-06-18
**Author:** Prometheus
**Branch:** `prometheus/arthur-disagreement-reversibility-design`
**Inputs:** Dispatch `Plans/DISPATCH-2026-06-18-arthur-disagreement-reversibility-design-prometheus.md`; live code at `origin/main` HEAD `bf30a04`; OT-19/OT-22 from `docs/OPEN-THREADS.md`; Article 6.6 (no kernel judgment/policy), Article 29 (Arthur); OT-17 ratified THINK (`THINK-2026-06-18-arthur-memory-substrate-design.md` — D5 crypto-erase); OT-18 ratified THINK (`THINK-2026-06-18-arthur-grant-ergonomics-design.md` — D5 depends on this THINK delivering reversibility for reversible actions); Slice 4 action surface (`userland/libneo/src/action.rs` — `ActionKind` enum including `WriteFile`, `KillProcess`); destination red-team THINK (`THINK-2026-06-18-arthur-destination-red-team.md` — Q6 disagreement probe); NeoFS (Phase 3, Article 10 — durable filesystem with capability-gated handles)
**Scope:** Design THINK only. No production code, no `ci.yml`. One pillar (OT-22): disagreement + reversibility. Completes the trilogy: OT-17 (what Arthur remembers) · OT-18 (what Arthur may do) · OT-19 (how Arthur disagrees).

---

## 1. Problem statement — the obey-vs-override trap

A useful long-term assistant eventually has enough context to know when you're making a mistake. "Delete the project directory" — two months of work, three branches reference it. Both poles are wrong:

| Pole | What happens |
|------|-------------|
| **Always obey** | Arthur is a shell script. The context is wasted. |
| **Knows better / refuses** | Arthur owns the machine. The owner loses sovereignty. |

The resolution (red-team 2026-06-18): **disagreement expressed through reversibility, not refusal.** Arthur's lever is never "no," it's *"yes — and I made it undoable."* Graduated by **reversibility × stakes, not by Arthur's confidence** — so the design is robust to Arthur being wrong about whether you're wrong. The owner stays sovereign; Arthur never vetoes.

This THINK delivers two things:
1. **The disagreement model (D1/D3/D4):** How Arthur responds when it thinks you're mistaken — graduated, non-vetoing, robust to its own fallibility.
2. **The reversibility mechanism (D2):** The substrate that makes "yes, and I made it undoable" real — snapshot-before-write, tombstone-on-delete, undo-window. **This is what OT-18 D5 depends on.** OT-18's coarse durable grants are only safe because wrong actions are recoverable. This THINK delivers that recovery.

**Non-negotiables:**
- **Userland (Article 6.6):** The disagreement *behavior* lives in Arthur (userland). The kernel ships no judgment/policy. A NeoFS snapshot primitive would be a *mechanism* (general capability), not policy — flag as ADR-justified, not an `if Arthur` branch.
- **Robust to Arthur's fallibility:** Every design choice must be safe whether Arthur's disagreement is right or wrong. A model that's confidently wrong must not cause damage (reversibility) and must not veto the owner (no refusal).
- **Irreversible-external stays OT-18's domain:** NetSend/exfiltration is NOT made reversible. You can't "un-send" a leak. That's OT-18's restriction (host:port scoping), not this THINK's problem.

---

## 2. Constraints and the existing substrate

### 2.1 What exists today

- **NeoFS (Phase 3, bare-metal verified):** Durable filesystem. Files are NeoFS objects with capability-gated handles. `sys_write` modifies file contents; `sys_unlink` removes directory entries. No snapshot or copy-on-write primitive exists. **This is the storage substrate D2 must build on.**
- **Slice 4 action surface:** `ActionKind` includes destructive actions: `WriteFile`, `KillProcess`. These are the actions reversibility applies to.
- **OT-18 ratified (D5):** States that reversible actions' blast radius is bounded by recoverability — delivered by OT-19. Irreversible external actions (NetSend) are bounded by restriction — OT-18's own host:port scoping. **This THINK delivers the reversible half.**
- **OT-17 ratified (D5):** Per-category key burn for memory erasure. Memory writes are reversible (Arthur can overwrite a remembered fact). Memory erasure is irreversible (key burn destroys the category). The interaction between undo and crypto-erase needs explicit treatment (D5).

### 2.2 What this does NOT design (OT-22 boundaries)

| Deferred | Why | Cross-link |
|----------|-----|-----------|
| Grant ergonomics | OT-18, ratified. D5 depends on this THINK. | **OT-18** — cross-link only. This THINK delivers what OT-18 D5 assumes. |
| Memory substrate | OT-17, ratified. D5 crypto-erase interacts with undo. | **OT-17** — cross-link for the memory-undo interaction. |
| Multi-Arthur provenance | One Arthur today. | **OT-20** — defer. |

---

## 3. Design space: D1 — The disagreement model

### 3.1 The graduated response

Arthur's response to a potentially mistaken command is graduated by two structural dimensions:

| Dimension | What it measures | Who decides |
|-----------|-----------------|-------------|
| **Reversibility** | Can the action be undone? (snapshot exists, tombstone exists, or can't) | Structural property of the action — not Arthur's opinion. The mechanism either produces an undo artifact or it doesn't. |
| **Stakes** | How much would the user lose if this goes wrong? | **User policy, not Arthur's judgment.** The user marks which paths/patterns are high-stakes. Arthur never decides stakes. |

The response table:

| Reversibility | Stakes | Arthur's response |
|---------------|--------|-------------------|
| Reversible (undo artifact exists) | Any | **Obey, silently.** The action is undoable. No prompt, no friction. If the user later regrets it, they undo. |
| Irreversible local | Low (user policy says low) | **Obey, silently.** The stakes are low enough that confirm adds friction without proportional benefit. |
| Irreversible local | High (user policy says high) | **Surface consequence once, require confirm.** "This will delete /home/project/ permanently — no undo. Proceed?" One prompt. Not a veto — just a checkpoint. |
| Irreversible external (NetSend) | Any | **Not this THINK's domain.** OT-18's restriction (host:port scoping) bounds this. Per-host confirm is the user's policy choice (OT-18 D5). |

### 3.2 Why this is robust to Arthur being wrong

**The test:** Arthur thinks you're making a mistake, but it's wrong. You actually do want to delete the directory. What happens?

- If the action is **reversible:** Arthur obeys silently and preserves the undo artifact. You proceed. If Arthur was right (you didn't mean it), you undo. If Arthur was wrong (you did mean it), you just keep the new state and the undo artifact expires after the window. **No damage either way.**
- If the action is **irreversible + high-stakes:** Arthur surfaces the consequence and asks for confirm. You confirm. The action proceeds. Arthur doesn't veto — it checkpoints. **You stay sovereign.**
- If Arthur was wrong about the stakes (it thought something was catastrophic when it wasn't): the reversibility dimension handles it. If an undo artifact exists, Arthur obeys silently regardless of what it "thinks" about the stakes. The stakes dimension only matters for irreversible actions, where Arthur can't make it undoable.

**Key insight:** Arthur's confidence about whether you're wrong is irrelevant. The graduated response depends on *structural properties* (reversibility) and *user policy* (stakes thresholds), neither of which requires trusting the model's judgment.

### 3.3 Recommendation: Obey, make undoable, checkpoint only when irreversible + high-stakes

Arthur never refuses. Arthur never vetoes. The graduated response is:

1. **Can I make it undoable?** If yes → do it silently. No prompt.
2. **Can't make it undoable?** Is the stakes threshold (user policy) crossed? If no → do it silently. If yes → surface consequence once, require confirm.
3. **Irreversible external (NetSend):** Not this THINK's domain — OT-18 restriction.

---

## 4. Design space: D2 — The reversibility mechanism (load-bearing)

**This is what OT-18 D5 depends on.** The mechanism must make destructive local actions undoable.

### 4.1 Where does it live?

| Option | Description | Properties |
|--------|-------------|------------|
| **A: NeoFS snapshot primitive (kernel)** | A kernel-level `sys_snapshot(path) → snapshot_handle` that creates a point-in-time copy of a file via copy-on-write or full copy. General capability, not Arthur-specific. | Clean separation. Other processes can use snapshots too (backup tools, version control). Requires new kernel syscall and COW/full-copy logic in NeoFS. |
| **B: Arthur-userland copy-before-write** | Arthur copies the file to a backup location before writing. Pure userspace — reads the file, writes a copy to `.arthur/undo/`, then performs the destructive action. | No kernel change. Minimal-sufficient. Arthur-specific (only Arthur does reversibility). Backup lives on NeoFS (durable). |
| **C: NeoFS tombstone primitive (kernel)** | `sys_unlink` is replaced by `sys_tombstone` — moves the file to a `.tombstones/` directory instead of freeing blocks. General capability. | Clean for delete. Requires kernel change. Doesn't help with write (overwriting a file isn't unlink). |

### 4.2 Tradeoffs

**Option A (NeoFS snapshot primitive):** This is the cleanest long-term answer — a general COW/snapshot capability that any process can use. But it requires a new kernel syscall, NeoFS COW logic, and an ADR-level justification. At N=1 with one Arthur, this is machinery that doesn't earn its place yet (Article 15.7). **Banked as the post-N=1 answer.** The seam exists: when backup tools or version control arrive, the kernel snapshot primitive earns its place.

**Option C (NeoFS tombstone primitive):** Same kernel-change cost as A, narrower scope (only delete, not write). Doesn't help with `WriteFile` reversibility (overwriting a file needs a pre-write copy, not a post-delete tombstone). **Insufficient alone.**

**Option B (Arthur-userland copy-before-write):** No kernel change. Arthur reads the target file before writing, stores a copy in `.arthur/undo/<path-hash>/`, then performs the write. For delete, Arthur moves the file to `.arthur/undo/` instead of unlinking (tombstone, in userspace). This is minimal-sufficient: it makes destructive actions undoable with zero kernel surface. The backup lives on NeoFS (durable across reboot). **Recommended for N=1.**

### 4.3 Recommendation: B — Arthur-userland copy-before-write + tombstone

**The mechanism:**

**WriteFile reversibility (snapshot-before-write):**
- Before `Action::WriteFile { path, data }`, Arthur reads the current contents of `path` and writes them to `.arthur/undo/<category>/<path-hash>/prewrite.bin`.
- Arthur then performs the write.
- If the user later undoes, Arthur reads from the backup and writes it back to `path`.

**Delete tombstone (move, don't unlink):**
- Arthur does not have a `DeleteFile` action in Slice 4's `ActionKind`. But when Arthur effectively deletes content (e.g., overwrites with empty data, or the model proposes a destructive file operation), Arthur moves the original to `.arthur/undo/<category>/<path-hash>/tombstone.bin` instead of destroying it.
- **Honest note:** Slice 4's `ActionKind` has no explicit delete action. The reversibility mechanism covers `WriteFile` (the primary destructive action). If a `DeleteFile` action is added later, the same tombstone pattern applies: move to `.arthur/undo/`, don't unlink.

**KillProcess reversibility:**
- Killing a process is inherently irreversible (the process state is gone). No snapshot of a running process exists. **KillProcess is an irreversible local action.** It falls into the "irreversible + user policy decides stakes" row of the D1 table. If the user marks process-killing as high-stakes, Arthur confirms before killing. If not, it proceeds silently.

**Undo window and retention:**
- Undo artifacts are retained for **one Arthur session** (until Arthur process exits). Session-scoped: simple, bounded storage, matches the "undo what you just did" mental model.
- Storage: `.arthur/undo/` on NeoFS. Arthur prunes old undo artifacts at startup (from previous sessions). Max total undo storage: configurable, default 10 MB. If full, Arthur drops the oldest undo artifacts (LRU).

**Why userland, not kernel:** At N=1 with one Arthur, a kernel snapshot primitive is machinery that doesn't earn its place. Arthur-userland copy-before-write makes destructive actions undoable with zero kernel surface. The seam to a kernel primitive is clean: when backup tools or other consumers arrive, the kernel COW/snapshot capability earns its place and replaces the userspace copy.

---

## 5. Design space: D3 — Gauging stakes without trusting Arthur's judgment

### 5.1 The trap (OT-18-L3 revisited)

OT-18 rejected "high-stakes confirm" because *who decides high-stakes* is a policy judgment. If Arthur decides, we trust the model's judgment about its own authority — circular. This THINK must solve the same problem for the stakes dimension of the disagreement model.

### 5.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Arthur decides stakes** | Arthur assesses "how important is this file?" based on its memory and context. | Circular. Arthur is the thing we're protecting against — it may be confidently wrong. If Arthur thinks a file is low-stakes and doesn't make it undoable, but it was actually high-stakes, the user loses data. **Rejected.** |
| **B: Hardcoded stakes table** | "Files under /etc/ are high-stakes. Files under /tmp/ are low-stakes." | Brittle. The user's project directory might be more important than /etc/. A hardcoded list ages poorly and becomes a policy surface. **Rejected.** |
| **C: User policy — the user marks what's high-stakes** | The user declares which paths or patterns require confirm for irreversible actions. Arthur applies the user's policy mechanically. | The user decides stakes. Arthur enforces the policy. No model judgment involved. Clean separation: reversibility is structural (D2), stakes are user policy. |

### 5.3 Recommendation: C — User policy, Arthur enforcement

**The mechanism:**
- The user declares high-stakes paths/patterns in `.arthur/stakes.json` (or via conversation: "Arthur, always confirm before touching /home/project/").
- Format: list of `{ pattern, requires_confirm_for: ["irreversible"] }`.
- Arthur applies this mechanically: when an irreversible action targets a high-stakes path, Arthur surfaces the consequence and requires confirm. For reversible actions, Arthur never prompts — the undo artifact is the safety net.
- **Default:** No paths are marked high-stakes. All irreversible actions proceed silently. The user adds stakes as they learn what matters.

**Why this avoids the trap:** Arthur never judges stakes. It applies the user's policy mechanically — like a capability check, but for user-defined risk thresholds instead of kernel-enforced authority. The reversibility dimension (D2) is structural and doesn't depend on stakes at all: if an undo artifact exists, the action proceeds silently regardless of stakes.

---

## 6. Design space: D4 — How disagreement is surfaced

### 6.1 The constraint: information once, then defer

Arthur surfaces disagreement **once**, then defers to the owner. No nagging. No veto. No repeated "are you sure?" loops.

### 6.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Silent disagreement** | Arthur never surfaces disagreement. It just makes things undoable and lets the user find out later. | Maximum UX, minimum awareness. The user never knows Arthur thought they were wrong. Wastes the context that made Arthur disagree in the first place. |
| **B: Informational surface, once** | Arthur says: "I think this is a mistake because [reason from memory/context]. Proceeding — it's undoable." One line. Then proceeds. | Informs the user of Arthur's reasoning. Gives the user a chance to reconsider. Doesn't block. The "because" grounds the disagreement in Arthur's actual memory (OT-17), not abstract confidence. |
| **C: Block until acknowledged** | Arthur surfaces disagreement and waits for the user to acknowledge before proceeding. | Adds friction. If the user is busy or away, Arthur blocks. Moves toward veto territory. **Rejected.** |

### 6.3 Recommendation: B — Informational surface, once, then proceed

**The surfacing:**
- Arthur says one line: "I think this is a mistake because [grounded reason from memory]. Proceeding — it's undoable."
- The reason is grounded in Arthur's actual memory (OT-17): "because you told me this project has two months of work" not "because I have low confidence."
- Arthur proceeds immediately. No wait for acknowledgment.
- If the action is irreversible + high-stakes (user policy), Arthur requires confirm instead of proceeding silently. This is the only case where Arthur blocks — and it's because the user set the policy, not because Arthur decided stakes.

**Composition with OT-18 prompts:** Arthur never double-prompts. If OT-18's on-miss re-request (D4) surfaces ("I don't have permission to write here"), that's an authority prompt. If D4's disagreement surface fires ("I think this is a mistake"), that's an informational note. They don't stack: a missing cap means Arthur can't act at all, so there's nothing to disagree about. A disagreement only fires when Arthur *can* act (has the cap) but thinks it's a bad idea.

---

## 7. Design space: D5 — Boundaries

### 7.1 OT-18: irreversible-external is NOT made reversible

NetSend (exfiltration) is an irreversible external action. You cannot "un-send" data to a remote host. Reversibility does not apply. **This is OT-18's domain:** host:port scoping restricts *where* Arthur can send; per-host confirm is the user's policy choice. This THINK explicitly excludes NetSend from the reversibility mechanism.

### 7.2 OT-17: undo vs memory crypto-erase

OT-17's D5 uses per-category key burn for memory erasure. The interaction with undo:

- **Memory write (Arthur adds a fact):** Reversible via the undo mechanism (D2). Arthur backs up the memory file before writing. If the user undoes, the old memory content is restored.
- **Memory erasure (user says "forget about X"):** Irreversible by design. OT-17's key burn destroys the category's encryption key — the memories are cryptographically inaccessible. The undo mechanism does NOT back up encryption keys (that would defeat the forget guarantee). Memory erasure is an irreversible action that the user explicitly requested — it doesn't need Arthur's disagreement model because the user initiated it.
- **Boundary:** The undo mechanism backs up `.mem` file contents but never backs up `.key` files. Burning a key is a user-directed erasure, not an Arthur-initiated destructive action. The forget guarantee (OT-17 D5) is preserved: no undo artifact can recover burned-key memories.

### 7.3 OT-20: defer

Multi-Arthur provenance (who did what, through which delegation chain) is OT-20's domain. The undo mechanism is single-Arthur: Arthur backs up what it changes. When a second Arthur arrives, each Arthur manages its own undo artifacts. Cross-Arthur undo (Arthur-A undoes Arthur-B's action) is an OT-20 concern.

---

## 8. Design space: D6 — Fail-safe

### 8.1 The requirement

If reversibility can't be provided (snapshot fails, no space for backup), Arthur must degrade to **confirm-before-destructive** — never silently proceed without an undo. The kill-switch: reversibility-off ⇒ Arthur is conservative (confirm before destructive), never less safe than today.

### 8.2 Failure modes and responses

| Failure | Detection | Response |
|---------|-----------|----------|
| **Undo storage full** | `.arthur/undo/` exceeds max (default 10 MB) | Arthur prunes oldest undo artifacts (LRU). If still full after pruning, degrade to confirm-before-destructive for this action. |
| **Backup write fails** | `sys_write` to `.arthur/undo/` returns error (ENOSPC, EIO) | Cannot create undo artifact. Degrade to confirm: "I can't make this undoable (no space). Proceed without backup?" User confirms or cancels. |
| **Read-before-write fails** | `sys_read` of target file fails (file disappeared between check and copy) | No pre-state to back up. The action is now irreversible. Degrade to confirm: "The file changed since I checked — I can't back it up. Proceed?" |
| **Undo directory missing/corrupt** | `.arthur/undo/` doesn't exist or is unreadable | Arthur creates it on first use. If creation fails, degrade to confirm-before-destructive. |

### 8.3 The kill-switch shape

**Feature flag: `arthur-reversibility`** (default: off).

When **off** (default):
- No undo mechanism. No `.arthur/undo/` directory.
- Arthur is conservative: for destructive actions, Arthur surfaces the consequence and requires confirm if the user has marked the path as high-stakes in `.arthur/stakes.json`. Otherwise, Arthur proceeds silently (same as today — no reversibility, no extra friction).
- **Never less safe than today:** without reversibility, Arthur doesn't gain new destructive capabilities. It has the same caps as before. The kill-switch removes the undo mechanism; it doesn't add authority.

When **on**:
- Arthur creates `.arthur/undo/` on first destructive action.
- Copy-before-write for `WriteFile`. Tombstone-move for delete-equivalent actions.
- Undo artifacts retained for one session. Pruned at startup.
- If undo can't be created, degrade to confirm (never silently proceed without backup).

**The invariant:** Reversibility on is strictly more capable than reversibility off (undo exists vs doesn't). Reversibility off is never less safe than today (Arthur's caps are unchanged; the stakes policy is user-controlled).

---

## 9. Prior art — raw-sourced, design-level only

### 9.1 Reversibility in interactive systems

**Git:** Every commit is reversible (`git revert`, `git reset`). The undo artifact is the previous commit. Git doesn't ask "are you sure?" before `git push --force` — it just does it, and the old ref is recoverable via reflog for 30 days. **Design insight:** reversibility eliminates the need for confirm in most cases. Confirm is only needed when the undo artifact can't be preserved (e.g., `git push --force` to a shared branch where others have based work).

**Unix `mv` to trash:** Many desktop environments replace `rm` with move-to-trash. The trash is a tombstone directory. Emptying the trash is the irreversible step. **Design insight:** tombstone-on-delete is the simplest reversibility mechanism — move, don't unlink.

**SQLite WAL (Write-Ahead Log):** Changes are written to a WAL file before being applied to the main database. Rollback is replaying the WAL in reverse. **Design insight:** snapshot-before-write is a general pattern — preserve the pre-state before mutating.

### 9.2 Disagreement in human-AI interaction

**Copilot suggestions (GitHub):** Copilot suggests code completions. It doesn't refuse or confirm — it proposes, and the user accepts or ignores. **Design insight:** proposal-without-veto is the right model for assistant behavior. Arthur's disagreement is an informational note, not a block.

**Apple's "Focus" mode conflict resolution:** When a scheduled Focus mode conflicts with a calendar event, iOS surfaces the conflict once ("Focus mode will silence calls during your meeting") and lets the user decide. **Design insight:** surface once, then defer. Don't nag.

---

## 10. D1–D6 ratification block

| Decision | Recommendation | Rationale in one line |
|----------|----------------|----------------------|
| **D1: Disagreement model** | **Obey, make undoable, checkpoint only when irreversible + high-stakes.** Arthur never refuses, never vetoes. Reversible actions proceed silently (undo is the safety net). Irreversible + high-stakes (user policy) surface consequence once, require confirm. Irreversible-external (NetSend) is OT-18's restriction domain — not this THINK's problem. | Robust to Arthur being wrong: if Arthur is wrong about the mistake, the user just keeps the new state and the undo expires. If Arthur is right, the user undoes. No veto either way. |
| **D2: Reversibility mechanism** | **Arthur-userland copy-before-write + tombstone.** Before `WriteFile`, Arthur reads current contents → `.arthur/undo/<category>/<path-hash>/prewrite.bin`, then writes. Delete-equivalent: move to `.arthur/undo/` (tombstone), don't unlink. Undo window: one session. Max storage: 10 MB, LRU prune. KillProcess is irreversible — falls to stakes policy. **No kernel change.** NeoFS snapshot primitive banked as post-N=1. | Minimal-sufficient (Article 15.7). Zero kernel surface. Makes destructive local actions undoable. Delivers what OT-18 D5 depends on for reversible actions. |
| **D3: Stakes gauge** | **User policy, Arthur enforcement.** User declares high-stakes paths in `.arthur/stakes.json` (or via conversation). Arthur applies mechanically — never judges stakes itself. Default: no paths are high-stakes. Reversibility is structural (D2), stakes are user policy. | Avoids the OT-18-L3 trap. Arthur never trusts its own judgment about stakes. The reversibility dimension doesn't depend on stakes — if undo exists, proceed silently. |
| **D4: Surfacing UX** | **Informational, once, then proceed.** "I think this is a mistake because [grounded reason from OT-17 memory]. Proceeding — it's undoable." One line. No wait for acknowledgment. Composes with OT-18 prompts: no double-prompt (missing cap → can't act; disagreement → can act but thinks it's wrong). | Informs without blocking. Grounded in Arthur's actual memory, not abstract confidence. The user sees Arthur's reasoning and can reconsider — but Arthur doesn't hold up the action. |
| **D5: Boundaries** | **Irreversible-external (NetSend) excluded from reversibility** — OT-18 restriction (host:port scoping). **OT-17 memory erasure preserved:** undo backs up `.mem` contents but never `.key` files; key burn is user-directed, not Arthur-initiated. OT-20 multi-Arthur defer. | Honest exclusion: can't undo a leak. Memory forget guarantee preserved: no undo artifact recovers burned-key memories. |
| **D6: Fail-safe** | **Can't reverse ⇒ confirm-before-destructive.** Undo storage full → prune LRU, then confirm. Backup write fails → confirm. Read-before-write fails (file vanished) → confirm. Kill-switch: `arthur-reversibility` off ⇒ no undo, stakes policy still applies, never less safe than today. | Safe degradation: Arthur never silently proceeds without an undo artifact. Kill-switch removes undo capability; doesn't add authority. |

### 10.1 Build sequencing (post-ratification, one pillar per OT-22)

**Requires:**
- NeoFS (Phase 3, complete)
- Slice 4 action surface (`action.rs`, landed)
- Arthur Slices 1-4 (landed)
- OT-17 ratified THINK (D5 crypto-erase — boundary for D5 memory interaction)
- OT-18 ratified THINK (D5 depends on this THINK delivering reversibility)

**Build slices:**
1. **Slice R1:** Undo directory + copy-before-write for `WriteFile`. Arthur reads target, writes backup to `.arthur/undo/`, performs write. Undo window: one session. Kill-switch: `arthur-reversibility` off ⇒ no backup, proceed silently (same as today).
2. **Slice R2:** Tombstone for delete-equivalent actions + undo restoration ("undo last write" reads from backup). LRU prune at startup. Max storage config. Kill-switch: tombstone-off ⇒ unlink directly (no backup for delete).
3. **Slice R3:** Stakes policy (`.arthur/stakes.json`) + confirm-before-destructive for irreversible + high-stakes. Informational disagreement surface (D4). Kill-switch: stakes-off ⇒ no confirm, all actions proceed silently (reversibility still active from R1/R2).
4. **Slice R4:** Fail-safe degradation. Undo creation failure → confirm-before-destructive. Storage full → prune then confirm. Kill-switch: fail-safe-off ⇒ if undo fails, action fails (conservative, no silent proceed).

**Each slice is independently kill-switchable.**

### 10.2 Honest uncertainties

| Uncertainty | Status | Resolution path |
|-------------|--------|-----------------|
| Undo storage cost — how much space do backups consume? | Unknown | N=1 measurement. Default 10 MB, LRU prune. Adjust based on real usage. |
| Does the user actually use undo? | Unknown | Measure. If undo is never used, the mechanism is still valuable as OT-18's blast-radius bound — it exists so Arthur can grant wider scopes safely. |
| NeoFS snapshot primitive timing — when does it earn its place? | Banked | When backup tools or version control arrive (post-N=1). The userspace copy works for Arthur now; the kernel primitive serves multiple consumers later. |
| Memory undo granularity — per-file backup is coarse for memory | Tentative | OT-17's memory files are small (KB-scale). Backing up the whole category file before a single-record write is acceptable overhead. Per-record undo is a refinement that doesn't earn its place at N=1. |

---

## 11. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Arthur backs up a file, then the backup is corrupted | Medium — undo doesn't work | GCM-authenticated backup (reuse OT-17's AES-GCM). If backup auth fails on restore, Arthur reports "undo artifact corrupt" and the user must manually recover. The original destructive action already happened — the backup was best-effort insurance. |
| Undo storage fills silently, Arthur can't back up new writes | Medium — no undo for new actions | LRU prune at each write. If still full after prune, degrade to confirm (D6). Arthur never silently proceeds without a backup. |
| User relies on Arthur's disagreement surface and second-guesses themselves | Low — psychological, not technical | The informational surface is one line, grounded in memory, then defers. It's advice, not authority. Over time, the user calibrates to Arthur's reliability. |
| Arthur makes everything undoable, slowing down destructive actions | Low — copy-before-write adds one read + one write | File reads/writes are fast for KB-MB files (typical Arthur targets). The overhead is one extra round-trip per destructive action. Acceptable for the safety it buys. |
| Compromised Arthur deletes undo artifacts to cover its tracks | Medium — reversibility defeated | Undo artifacts are in `.arthur/undo/`, protected by Arthur's directory cap. A compromised Arthur process has the same cap — it could delete backups. But a compromised Arthur can also just write directly, bypassing the undo mechanism entirely. The undo mechanism protects against Arthur being *wrong*, not Arthur being *malicious*. Malicious Arthur is bounded by caps (OT-18 D5 revocation) and reversibility is a best-effort safety net, not a security boundary. |

---

**Status:** Design THINK complete. D1–D6 stated for PM review → PL ratification. No code. No CI gate. OT-19 remains OPEN until ratified. **Completes the trilogy: OT-17 (memory) · OT-18 (authority) · OT-19 (disagreement).**
