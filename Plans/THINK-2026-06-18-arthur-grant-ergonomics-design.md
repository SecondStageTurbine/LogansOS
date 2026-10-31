<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Arthur grant ergonomics design (OT-18)

**Date:** 2026-06-18
**Author:** Prometheus
**Branch:** `prometheus/arthur-grant-ergonomics-design`
**Inputs:** Dispatch `Plans/DISPATCH-2026-06-18-arthur-grant-ergonomics-design-prometheus.md`; live code at `origin/main` HEAD `bf30a04`; OT-18/OT-19/OT-20/OT-22 from `docs/OPEN-THREADS.md`; Article 29.3 (model proposes, capability disposes), Article 29.4 (userland), Article 15.7 (lean patch); Slice 4 action surface (`userland/libneo/src/action.rs` — `ActionKind` enum, `Grant`/`RequiredCapability` enums, `execute_action()` cap-check); OT-17 ratified THINK (`THINK-2026-06-18-arthur-memory-substrate-design.md` — D3 per-category caps is the grant model applied to memory); capability primitives (`process::derive_handle`, `process::revoke_handle`, `ipc::grant` at `kernel/src/sched/process.rs:866-930`); neo-shell Arthur spawn (`userland/neo-shell/src/main.rs:73-126` — handle inheritance via `sys_spawn`); destination red-team THINK (`THINK-2026-06-18-arthur-destination-red-team.md` — "safe to be wrong" concession)
**Scope:** Design THINK only. No production code, no `ci.yml`. One pillar (OT-22): grant ergonomics across Arthur's action surface.

---

## 1. Problem statement — the companion vs compliance officer tradeoff

Article 29.3 is the safety keystone: **"model proposes, capability disposes."** Arthur can reach only what it has been granted, enforced by the kernel's capability system. Slice 4 proved this live: `execute_action()` checks the `Grant` set before any syscall runs, returning `ActionError::Denied` on missing authority.

The keystone answers **CAN** (does Arthur hold the authority?). It says nothing about **how the human grants it.** Two poles, both wrong:

| Pole | UX | Blast radius |
|------|----|-------------|
| Fine per-action grants — prompt before every action | Arthur is a compliance officer. "Can I read this file?" × 400 per session. Unusable. | Small. Each action is individually authorized. |
| Coarse durable grants — grant everything once, Arthur acts silently | Good UX. Arthur acts at machine speed. | Large. If Arthur is compromised or confidently wrong, it can do anything it was granted. The "safe to be wrong" property degrades as grants widen. |

This design finds the humane middle: **front-loaded, scoped, durable grants** where the cap-check is silent and deterministic within scope, only a *missing* capability surfaces to the user, and the blast radius is bounded by complementary mechanisms (not by narrowing the grants themselves).

**Non-negotiables:**
- **The keystone is never softened.** The kernel cap-check (29.3) remains the boundary by construction. Ergonomics live in *userland* — how Arthur requests and the user grants. The enforcement stays in the kernel, unchanged.
- **Userland ergonomics.** Article 29.4: Arthur is userland. The grant surface is a userspace concern.
- **Build on existing primitives.** No new authority primitive. `derive_handle`/`grant`/`revoke_handle` compose into the ergonomics.

---

## 2. Constraints and the existing substrate

### 2.1 What exists today (the authority substrate)

- **Capability system (Phase 2, silicon-proven):** `Handle` with `CapRights`. `process::derive_handle(pid, h, child_rights)` attenuates rights (child ⊆ parent, requires DERIVE meta-right). `process::revoke_handle(pid, h)` revokes through the derivation graph (requires REVOKE meta-right). `ipc::grant` delegates across process boundaries. **These are built and proven.**
- **Slice 4 action surface (`action.rs`):** Typed `Action` enum (13 kinds: `ReadFile`, `WriteFile`, `ListDir`, `SpawnProcess`, `KillProcess`, `InspectProcess`, `WaitProcess`, `NetDial`, `NetSend`, `NetRecv`, `Infer`, `LoadModel`, `CloseFile`). `Grant` enum mirrors the authority required for each action. `execute_action(grants, action)` checks `grant.allows(&required)` before any syscall — returns `ActionError::Denied { action, required }` on miss. **This is the userspace policy layer that decides WHICH kernel cap to present; it is not the enforcement boundary.** The enforcement boundary is the kernel cap-check: Arthur holds only per-scope narrow kernel capabilities (derived via `derive_handle`, granted via `ipc::grant`). If a scope is not granted, Arthur simply does not hold the kernel cap for that resource — the syscall fails with EPERM regardless of what `grants.json` says. A compromised Arthur that ignores `grants.json` cannot exceed its held kernel caps.
- **Arthur spawn (`neo-shell/main.rs:73-126`):** Arthur gets initial capabilities via handle inheritance through `sys_spawn("arthur", &inherited)`. Today: IPC channel + model handle. Placeholder slot reserved for future fs-read cap. **Arthur starts with nearly nothing — the least-privilege floor is already real.**
- **OT-17 ratified (D3):** Per-category capabilities, read-only by default. Arthur holds parent cap, derives USE-only for grantees. This THINK's D2 scope taxonomy includes memory categories as one scope class.

### 2.2 What this does NOT design (OT-22 boundaries)

| Deferred | Why | Cross-link |
|----------|-----|-----------|
| Memory substrate + memory-access caps | OT-17, ratified. Its per-category caps are an **input** to D2's scope taxonomy. | **OT-17** — cross-link only. |
| Philosophy of disagreement (Arthur vs owner) | "Yes — and I made it undoable" is a behavioral model, not an authority model. OT-18 = friction on *authorization*; OT-19 = friction on *advice*. | **OT-19** — D5's reversibility lever connects to OT-19; the disagreement model itself is deferred. |
| Multi-Arthur provenance / N-principal control | One Arthur today. N=1 floor first. | **OT-20** — D6's single-Arthur audit is the seam. OT-20 extends it with action-provenance chains across principals. |

---

## 3. Design space: D1 — The grant model

### 3.1 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Per-action prompts** | Every `execute_action()` that would succeed is preceded by a user prompt: "Arthur wants to read /home/project/Cargo.toml — allow?" | Maximum safety, minimum UX. Arthur stops before every action. |
| **B: Coarse durable scopes** | The user grants broad scopes once ("Arthur can read my project directory"). Within scope, actions are silent. Only missing caps surface. | Good UX. Arthur acts at machine speed within scope. Blast radius = the scope width. |
| **C: Hybrid — coarse default + high-stakes escalation** | Coarse scopes for routine actions (read files, list dirs, infer). High-stakes actions (write, spawn, kill, net-dial) require per-action confirmation regardless of grant. | Middle ground. But: "high-stakes" is a policy judgment the kernel doesn't make (Article 6.6). And it still prompts on every destructive action — Arthur asks before every `rm`. |

### 3.2 Tradeoffs

**Option A (per-action):** The red-team already identified this as making Arthur a compliance officer. It's the wrong answer for a companion. **Rejected.**

**Option C (hybrid with high-stakes escalation):** The "high-stakes" classification is the problem. Who decides what's high-stakes? If it's Arthur, we're back to trusting the model's judgment about its own authority — which is exactly what the capability system exists to avoid. If it's a hardcoded list in userspace, it's brittle and becomes a policy surface that ages poorly. And prompting on every file write makes Arthur unusable for editing tasks ("write config", "save notes"). **Rejected.** The right way to bound destructive actions is not *more prompts* — it's *reversibility* (D5, cross-link OT-19).

**Option B (coarse durable scopes):** This is the direction the red-team identified. Grant a scope once, Arthur acts silently within it. The cap-check is deterministic and silent — only a *missing* capability surfaces. The blast radius is bounded not by narrowing grants but by complementary mechanisms: reversibility (OT-19), revocation (D6), and least-privilege defaults (D7). **Recommended.**

### 3.3 Recommendation: B — Coarse front-loaded scopes, silent-within-scope

**The model:**
- The user grants Arthur a **scope** (e.g., "read access to /home/project/"). Mechanically: the shell derives a narrow kernel cap for that scope (`derive_handle`) and delegates it to Arthur via `ipc::grant`. The grant is also recorded in `.arthur/grants.json` as an audit/persistence record. **Arthur holds only per-scope narrow kernel caps — never a broad cap gated by the file.**
- Within scope, Arthur acts silently. It presents the relevant kernel cap to the syscall. No prompt, no friction.
- Only when Arthur lacks the kernel capability for an action does it surface to the user: "I can't do that — I don't have permission to write to /etc/. Want me to ask for it?"
- The cap-check in the kernel is **never** softened. Ergonomics are purely about *how the grant set is managed*, not about bypassing the check.

---

## 4. Design space: D2 — Minimal scope taxonomy

### 4.1 The constraint: map to Slice 4's actual `ActionKind`

The scope taxonomy must cover every `ActionKind` in `action.rs` without inventing scopes Arthur can't use. The `Grant` enum already defines the authority granularity. The scope taxonomy groups related grants into human-meaningful scopes.

**Slice 4's `ActionKind` (13 kinds):**
```
ReadFile, WriteFile, ListDir           → File I/O
SpawnProcess, KillProcess, InspectProcess, WaitProcess  → Process management
NetDial, NetSend, NetRecv              → Network
Infer, LoadModel                       → Model compute
CloseFile                              → Lifecycle (no authority needed)
```

### 4.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: One scope per `ActionKind`** | 13 scopes, one per action kind. "Grant Arthur ReadFile" / "Grant Arthur WriteFile". | Matches the code exactly. But: granting "ReadFile" without scoping *which files* is either everything (too broad) or requires a path filter per grant (too fine). |
| **B: Resource-class scopes with path/host scoping** | Group by resource class (files, processes, network, model), scoped to specific paths or hosts. "Read /home/project/" / "Dial api.example.com:443". | Human-meaningful. Matches the `Grant` enum's existing path/host fields. Minimal number of scopes for real use. |
| **C: Free-text policy rules** | The user writes arbitrary rules ("Arthur can read markdown files in project dirs"). Arthur parses and matches. | Maximum flexibility. Maximum complexity. A policy language is a whole subsystem. Violates Article 15.7. **Rejected.** |

### 4.3 Tradeoffs

**Option A (one scope per action kind):** Loses the path/host scoping that the `Grant` enum already carries. "ReadFile" without a path is either all files or requires a separate path-list per grant — which is Option B in disguise. **Rejected.**

**Option B (resource-class scopes with scoping):** The `Grant` enum already has path fields (`Grant::File { path, rights }`) and host:port fields (`Grant::NetDial { host, port }`). The scope taxonomy groups these into human-meaningful buckets: "file read on /home/project/", "network dial to api.example.com:443". This is what the code already supports. **Recommended.**

### 4.4 Recommendation: B — Resource-class scopes, minimal-sufficient taxonomy

**The scopes (derived from Slice 4's `Grant` enum):**

| Scope | Covers (`ActionKind`) | Scoping dimension | Example grant |
|-------|----------------------|-------------------|---------------|
| **FileRead** | `ReadFile`, `ListDir` | Path prefix | "read /home/project/" |
| **FileWrite** | `WriteFile` | Path prefix | "write /home/project/" |
| **Process** | `SpawnProcess`, `KillProcess`, `InspectProcess`, `WaitProcess` | Binary name (spawn) or process handle | "spawn neo-grep" |
| **Network** | `NetDial`, `NetSend`, `NetRecv` | Host:port | "dial api.example.com:443" |
| **Model** | `Infer`, `LoadModel` | Model path | "load /models/qwen2.5-0.5b.neom" |
| **Memory** | (OT-17 D3) per-category memory access | Category name | "read health memories" |

**Six scopes.** They cover every `ActionKind`. The scoping dimension (path, host:port, binary name, category) is what the `Grant` enum already carries. No new authority granularity — just grouping existing grants into human-meaningful buckets.

**Why not merge FileRead and FileWrite:** Read and write have different blast radii. "Arthur can read my project" is a natural grant. "Arthur can write to my project" is a bigger ask that the user may want to control separately. The capability system already separates `FileRights::READ` from `FileRights::WRITE` — the scope taxonomy should reflect that.

**Why Memory is a separate scope:** OT-17's D3 already defines per-category memory caps. Memory access is privacy-sensitive (Article 23.2) and has its own grant/derive/revoke mechanics. It fits the same pattern (scope + scoping dimension) but deserves its own bucket because the user thinks about "what Arthur remembers" differently from "what files Arthur can read."

---

## 5. Design space: D3 — The friction model

### 5.1 The question

When does the human see a prompt? The answer determines whether Arthur feels like a companion or a compliance officer.

### 5.2 Options

| Option | Description | When prompts appear |
|--------|-------------|-------------------|
| **A: Grant-time only** | The user grants scopes explicitly (e.g., via a shell command or Arthur conversation). Within scope, zero prompts. Outside scope, Arthur says "I can't do that" and stops. | Only when the user initiates a grant. Never during Arthur's actions. |
| **B: Grant-time + on-miss re-request** | Same as A, but when Arthur hits `ActionError::Denied`, it offers to request the scope: "I don't have permission to write to /etc/. Want me to ask?" The user can grant then. | Grant-time + opportunistically when Arthur needs a capability it doesn't have. |
| **C: Periodic review prompts** | Arthur periodically asks "here's what I can do — still okay?" | Grant-time + on-miss + periodic reviews. Adds friction without proportional safety benefit. |

### 5.3 Tradeoffs

**Option C (periodic reviews):** Friction without proportional safety. The user already has the audit surface (D6) to review grants on demand. Periodic prompts are nagging, not protecting. **Rejected.**

**Option A (grant-time only):** Cleanest UX. But it puts the burden on the user to anticipate what Arthur will need. "Arthur can't read my calendar file" — the user has to remember to grant it before Arthur needs it. This is workable for a power user but frictional for everyday use.

**Option B (grant-time + on-miss re-request):** The humane middle. Arthur acts silently within scope. When it encounters something it can't do, it asks *once* — and the user can grant "for this session" or "always." This is how real delegation works: you delegate a task, and when the delegate hits a wall, they ask for what they need. **Recommended.**

### 5.4 Recommendation: B — Grant-time + on-miss re-request

**The friction model:**
1. **Grant-time:** The user grants scopes explicitly (via Arthur conversation or shell command). This is the front-loaded grant.
2. **Silent-within-scope:** Arthur acts at machine speed. The cap-check is deterministic and silent. No prompts, no friction.
3. **On-miss re-request:** When `execute_action()` returns `ActionError::Denied`, Arthur surfaces it to the user: "I can't [action] — I don't have permission to [resource]. Want me to ask for [scope]?" The user can grant for this session or always.
4. **Remember the denial:** If the user denies the re-request, Arthur remembers and doesn't ask again for that scope during this session. No nagging.

---

## 6. Design space: D4 — Graceful re-request on Denied

### 6.1 The problem

When Arthur hits `ActionError::Denied`, it needs to request the missing capability from the user without becoming a nag. The re-request must be:
- **Informative:** Tell the user what Arthur wants and why.
- **Bounded:** Don't ask more than once per scope per session.
- **Scoped:** Offer the minimal grant that would satisfy this action — not a blanket "give me everything."

### 6.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Single-action re-request** | "I can't read /home/user/calendar/june.cal. Grant me read access to this file?" Grants exactly one file path. | Minimum blast radius. But: if Arthur needs to read 50 files in the same directory, it asks 50 times. |
| **B: Scope-level re-request** | "I can't read /home/user/calendar/june.cal. Grant me read access to /home/user/calendar/?" Infers the parent scope from the denied path. Grants a directory, not a file. | Natural scoping. One grant covers future actions in the same directory. The user grants what they meant to grant. |
| **C: Batch re-request** | Arthur accumulates Denied actions and presents them as a batch: "I need these 3 permissions to complete your request." | Efficient for multi-action tasks. But: the batch may contain unrelated scopes that the user would grant separately. |

### 6.3 Tradeoffs

**Option A (single-action):** Too fine-grained. Arthur asking for every file path is a compliance officer. **Rejected.**

**Option C (batch):** Premature. At N=1 with a real user, Arthur's tasks are usually single-action or a small sequence. Batching adds complexity (when does the batch flush? what if the user grants some but not all?) that doesn't earn its place. **Rejected for now.**

**Option B (scope-level re-request):** The denied action tells Arthur exactly what scope is missing. For file actions, infer the parent directory as the scope. For network, the host:port. For process, the binary name. One grant covers future actions in the same scope. **Recommended.**

### 6.4 Recommendation: B — Scope-level re-request with remember-the-denial

**The mechanism:**
- Arthur receives `ActionError::Denied { action, required }`. It extracts the scope from `required` (e.g., `RequiredCapability::File { path, rights }` → scope = parent directory + rights).
- Arthur presents: "I can't [action description]. I'd need [scope description] to do that. Grant it?"
- User options: **"Yes, for this session"** (grant expires at Arthur process exit) or **"Yes, always"** (durable grant, persists across restarts).
- If the user says **no**, Arthur records the denial in a per-session `denied_scopes` set and does not re-request the same scope during this session.

**The authority boundary (crisp statement):** Granting a scope is a two-step operation: **(1)** the shell or user process derives a narrow kernel capability via `derive_handle` (scoped to the granted resource) and delegates it to Arthur via `ipc::grant`; **(2)** the grant is recorded in `.arthur/grants.json` as a UX/persistence/audit record. **The kernel cap is the enforcement boundary by construction (Article 29.3).** Arthur holds only per-scope narrow caps — never a broad cap gated by the tamperable `grants.json` file. If Arthur is compromised and ignores `grants.json`, it cannot exceed its held kernel capabilities. `grants.json` is the audit trail and persistence layer, not the authority boundary. Revocation (D6) works by calling `revoke_handle` on the parent kernel cap — cascading through the derivation graph — then updating `grants.json` to reflect the change. Session-scoped grants are kernel caps that expire at Arthur process exit (the kernel drops them); they are not recorded in `grants.json`.

---

## 7. Design space: D5 — Bounding the blast radius (load-bearing)

### 7.1 The problem

Coarse durable grants mean Arthur can do a lot without asking. If Arthur is compromised, confused, or confidently wrong, the blast radius is the width of the grants. This is the unresolved tradeoff from the red-team: **"the safe-to-be-wrong property degrades as grants widen."**

The question: how do we preserve "safe to be wrong" under wide grants?

### 7.2 Candidate levers

| Lever | What it does | Blast-radius impact |
|-------|-------------|-------------------|
| **L1: Reversibility (OT-19)** | Destructive actions produce an undoable artifact (snapshot, backup, tombstone). Arthur's lever is never "no" — it's "yes, and I made it undoable." | High. A wrong action can be reversed. The blast radius is bounded by reversibility, not by refusing to act. |
| **L2: Session-scoped grants** | Some grants expire at Arthur process exit. Wide grants for a session don't persist into the next session. | Medium. Limits persistence of wide grants. But: the user may want durable access (e.g., "always read my project"). |
| **L3: High-stakes confirm** | Certain actions (kill process, write to system dirs) require explicit confirmation regardless of grant. | Low-medium. Adds friction for specific high-risk actions. But: what's "high-stakes" is a policy judgment (Article 6.6). |
| **L4: Revocation (D6)** | The user can revoke any grant at any time. `revoke_handle` cascades through the derivation graph. | High. Immediate authority removal. But: reactive, not preventive. |

### 7.3 Tradeoffs and the honest answer

**L3 (high-stakes confirm):** This is the lever that looks right but isn't. Defining "high-stakes" is a policy judgment. If Arthur decides, we trust the model's judgment about its own authority — circular. If a hardcoded list decides, it becomes brittle ("Arthur needs to write to /etc/ for this one config change and now it prompts"). **Not a primary lever.** It can exist as a user-configurable preference (the user marks certain paths as high-stakes), but it's not the architecture's blast-radius guard.

**L2 (session-scoped grants):** Useful as a grant *duration* option (offered in D4's re-request: "for this session" vs "always"). But it's not a standalone blast-radius bound — the user will choose "always" for scopes they trust, and then the grant is durable.

**L1 (reversibility) + L4 (revocation) for reversible actions; L3 (restriction) for irreversible/external:** These levers compose into a real blast-radius bound, but they apply to different action classes:

- **Reversible actions (files, processes):** Reversibility (OT-19) makes wrong actions recoverable. Arthur writes a file? It also keeps a backup. Arthur deletes something? It goes to a tombstone, not gone. Revocation (D6) makes authority removable. The blast radius is bounded by *what can be undone*.
- **Irreversible external actions (NetSend — exfiltration):** Reversibility cannot undo a leak. You can't "un-send" data to a remote host. Revocation stops future sends but doesn't recover what was already sent. The bound here is **restriction**: D2's host:port scoping ensures Arthur can only dial specific hosts — not the internet at large. A broad "Network" grant that covers all destinations is not safe, even with reversibility. Per-action confirm (L3) is legitimate as a user-configurable preference for network sends, because the restriction (host:port scope) makes the confirm surface small and bounded — Arthur asks before sending to a new host, not before every packet.

**The honest answer:** The blast radius is bounded differently by action class:
- **Reversible actions (files, local):** Recoverability, not restriction. Wrong actions are undoable (OT-19). Authority is removable (D6).
- **Irreversible external actions (network):** Restriction, not recoverability. D2's host:port scoping limits *where* Arthur can send. Confirm-on-new-host is a legitimate user preference here because the scope is narrow enough that confirm doesn't become nagging. Reversibility does not apply to exfiltration — don't claim it does.

### 7.4 Recommendation: Recoverability for reversible, restriction for irreversible/external

**The model:**
- Arthur holds coarse durable grants (D1). Within scope, it acts silently (D3).
- **Reversible actions (files, local process ops):** Reversibility (OT-19) makes wrong actions recoverable — snapshot-before-write, tombstone-on-delete, etc. Revocation (D6) makes authority removable. The blast radius is bounded by *recoverability*. **Cross-link: OT-19 must deliver reversibility before OT-18's coarse file grants are safe.** If OT-19 is delayed, narrower file scopes interim.
- **Irreversible external actions (NetSend):** Reversibility cannot undo a leak. The bound is **restriction** — D2's host:port scoping limits *where* Arthur can send. No broad "Network" grant. Per-host confirm is legitimate as a user preference here because the narrow scope keeps confirm from becoming nagging.
- **Revocation (D6):** Applies to all action classes. The user inspects and revokes any grant at any time. `revoke_handle` cascades through the derivation graph — immediate, complete, no stale access.

**The sequencing guard:** OT-18's coarse grants for reversible actions depend on OT-19's reversibility. Network grants are scoped by host:port (D2) regardless of OT-19 — restriction is the bound there, not recoverability. Until OT-19 ships, narrower file/process defaults limit the blast radius for reversible actions.

---

## 8. Design space: D6 — Audit + revoke

### 8.1 The problem

The user needs to see what Arthur can currently do and be able to revoke any scope. This is the human control surface for N=1 Arthur.

### 8.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Shell command (`arthur grants`)** | A neo-shell builtin that lists Arthur's current grants and accepts `revoke <scope>` commands. | Simple. Uses existing shell infrastructure. But: requires leaving the Arthur conversation to manage grants. |
| **B: Arthur conversation ("show me what I've granted you")** | Arthur reports its grant set in natural language. "I can read /home/project/, write /home/notes/, and dial api.example.com:443." Revocation via conversation: "revoke my write access to /home/notes/". | Natural. Stays in the conversation flow. Arthur parses the revoke request and calls `revoke_handle`. |
| **C: Both** | Shell command for power users, conversation for everyday use. | Complete coverage. More surface to maintain. But: both are thin wrappers over the same grant store. |

### 8.3 Recommendation: C — Both (conversation primary, shell secondary)

**The conversation surface (primary):**
- "What can you do?" → Arthur lists its grant set in natural language, grouped by scope.
- "Revoke my [scope]" → Arthur parses the scope, finds the corresponding grant, and calls `revoke_handle` on the parent capability. The revocation cascades through the derivation graph — immediate, complete.
- "What did you do today?" → Arthur reports recent actions (the action log, stored in userspace). This is the single-Arthur audit trail. OT-20 extends this to multi-principal provenance.

**The shell surface (secondary):**
- `arthur grants` → lists grants in structured format (scope, resource, rights, durable/ session).
- `arthur revoke <scope>` → revokes the specified scope.
- `arthur log` → shows recent action log.

**How revocation works mechanically:** Arthur holds the parent capability for each granted scope. When the user revokes a scope, Arthur calls `revoke_handle` on the parent — and all derived handles (including any delegated to other processes via `ipc::grant`) are revoked through the derivation graph. This is the built primitive doing exactly what it was designed for.

**The OT-20 seam:** This audit surface is single-Arthur. When a second Arthur principal arrives (OT-20), the audit extends to show delegation chains: "Arthur-Ops can read /home/project/ because you granted it to Arthur-Coder, who delegated it." The grant store format includes a `granted_by` field for this extension.

---

## 9. Design space: D7 — Least-privilege default

### 9.1 The question

What does Arthur ship with? No grants (earn each scope) or a minimal sensible default?

### 9.2 Options

| Option | Description | Properties |
|--------|-------------|------------|
| **A: Zero grants** | Arthur starts with nothing. Every scope must be explicitly granted. The model handle is inherited at spawn (compute capability), but no file, network, or process access. | Maximum security. Maximum initial friction. Arthur says "I can't do that" for everything until the user grants scopes. |
| **B: Minimal sensible default** | Arthur ships with read access to its own directory (`.arthur/`) and the model path. No write, no network, no process management. | Reduces initial friction for Arthur's own operations (reading its grant store, memory files). Still least-privilege for everything else. |
| **C: Broad default** | Arthur ships with read access to the user's home directory. | Convenience over security. Violates the least-privilege principle. The user hasn't consented to Arthur reading their entire home. **Rejected.** |

### 9.3 Recommendation: B — Minimal sensible default

**The default grants at Arthur first-boot:**
- **FileRead on `.arthur/`:** Arthur needs to read its own grant store (`.arthur/grants.json`) and memory files (`.arthur/memory/`, OT-17). Without this, Arthur can't function — it can't load its grants or memories.
- **ModelUse for the loaded model:** Inherited at spawn from neo-shell. Not a grant — it's the model handle passed through `sys_spawn`.

**Everything else requires explicit user grant.** File write, network, process management, memory access to other categories — all earned through D1/D4 (front-loaded grant or on-miss re-request).

**Why not zero grants:** Arthur needs to read its own state files to function. Granting `.arthur/` read is not a privilege — it's Arthur reading its own home directory. This is the minimum for Arthur to boot and operate. The user can see this grant in the audit surface (D6) and revoke it if desired (which would disable Arthur's memory and grant persistence — a deliberate choice).

---

## 10. Prior art — raw-sourced, design-level only

### 10.1 Capability delegation patterns

**Capsicum (FreeBSD):** The `capsicum-mode` transition drops ambient authority; the process operates only with file descriptors it was granted. Grant is at process spawn (FD inheritance). No runtime re-request — the parent must anticipate all needed FDs. **Design insight:** capsicum's grant model is parent-driven (the spawning process decides), not self-driven (the child can't ask for more). Arthur needs self-driven re-request (D4) because the user, not a parent process, is the authority.

**seccomp-bpf (Linux):** Filters syscalls at kernel entry. Static policy, no runtime escalation. **Design insight:** seccomp's one-way ratchet (can't add permissions after filtering) matches our revoke-through-derivation-graph model — authority can be removed but not self-granted.

### 10.2 Permission UX patterns

**Android runtime permissions:** Coarse scopes ("location", "camera", "storage") granted at first use. The app requests the scope when it needs it (on-miss re-request). User can grant/deny in settings (audit + revoke). **Design insight:** Android's "grant at first use" is exactly our D4 on-miss re-request. The scope taxonomy (location, camera, storage) maps to our resource-class scopes. Android's settings panel for review maps to our D6 audit surface.

**macOS TCC (Transparency, Consent, and Control):** Per-scope prompts at first access ("App wants to access your Documents folder"). Grant persists. Revocable in System Settings. **Design insight:** TCC's per-scope persistence matches our durable grants. The settings-based revoke matches D6.

---

## 11. D1–D7 ratification block

| Decision | Recommendation | Rationale in one line |
|----------|----------------|----------------------|
| **D1: Grant model** | **Coarse front-loaded scopes, silent-within-scope.** Grant a scope once → Arthur acts silently within it. Only missing caps surface. Cap-check never softened. | Companion UX (no per-action prompts). Cap-check stays the kernel boundary. Blast radius bounded by D5 levers, not by scope narrowing. |
| **D2: Scope taxonomy** | **Six resource-class scopes:** FileRead (path), FileWrite (path), Process (binary name), Network (host:port), Model (model path), Memory (category). Derived from Slice 4's `Grant` enum — no new authority granularity. | Minimal-sufficient. Covers every `ActionKind`. Scoping dimensions (path, host:port, binary, category) are what the code already carries. |
| **D3: Friction model** | **Grant-time + on-miss re-request.** Silent within scope. On `Denied`, Arthur offers to request the scope. User grants "for this session" or "always". Remember-the-denial prevents nagging. | Humane middle. Arthur asks once per missing scope per session. No periodic reviews. |
| **D4: Re-request mechanism** | **Scope-level re-request.** Infer the parent scope from the denied action (directory for files, host:port for network). Offer "session" or "always". Denied scopes remembered per-session. **Authority boundary:** granting = shell derives narrow kernel cap (`derive_handle`) + `ipc::grant` to Arthur; `grants.json` is UX/persistence/audit record, NOT the enforcement boundary. Arthur holds only per-scope narrow kernel caps — a compromised Arthur ignoring `grants.json` cannot exceed its held caps. | One grant covers future actions in the same scope. The kernel cap is the boundary by construction (29.3). The tamperable file is never the authority boundary. |
| **D5: Blast radius** | **Recoverability for reversible actions (OT-19 + D6); restriction for irreversible/external (D2 host:port scoping).** File/process blast radius bounded by reversibility (OT-19's undoable artifacts) + revocation (D6). Network blast radius bounded by restriction (per-host:port scope — no broad Network grant; confirm-on-new-host as user preference). **Sequencing guard:** coarse file grants depend on OT-19; network scoped regardless. | Do not claim recoverability bounds exfiltration. Reversibility cannot undo a leak. The bound for irreversible external actions is restriction (scope width), not recoverability. Honest dependency on OT-19 for reversible actions — stated, not hidden. |
| **D6: Audit + revoke** | **Conversation primary ("what can you do?", "revoke X"), shell secondary (`arthur grants`, `arthur revoke`).** Revocation: `revoke_handle` on parent kernel cap — cascades through derivation graph (enforcement), then update `grants.json` (audit record). Action log for recent activity. OT-20 seam: `granted_by` field for multi-principal provenance. | Natural control surface. The enforcement is the kernel cap revocation; `grants.json` is the audit trail. Single-Arthur now; OT-20 extends. |
| **D7: Default posture** | **Minimal sensible default:** FileRead on `.arthur/` only (Arthur needs to read its own state). Model handle inherited at spawn. Everything else earned via D1/D4. | Least-privilege start. Arthur can boot and load its grants/memories. No ambient access to user data. |

### 11.1 Build sequencing (post-ratification, one pillar per OT-22)

**Requires:**
- Capability primitives: `derive_handle`, `revoke_handle`, `ipc::grant` (Phase 2, silicon-proven)
- Slice 4 action surface (`action.rs`, landed)
- Arthur Slices 1-4 (landed)
- OT-17 ratified THINK (D3 per-category caps — input to D2 scope taxonomy)

**Cross-pillar dependency:**
- **E5 (coarse grants) requires OT-19 (reversibility):** Coarse durable grants are only safe with reversible destructive actions. Until OT-19 ships, use narrower default scopes as an interim blast-radius bound. This is a stated dependency, not a blocker for the grant ergonomics build itself — the grant infrastructure (D1-D4, D6-D7) is independent. The *width* of the default grants depends on OT-19.

**Build slices:**
1. **Slice E1:** Grant store (`.arthur/grants.json`) + load at startup → build `Grant` slice for `execute_action()`. D7 defaults (FileRead on `.arthur/`). Kill-switch: grant-store-off ≡ no grants (Arthur starts with only spawn-inherited caps).
2. **Slice E2:** On-miss re-request (D3/D4). Arthur surfaces `ActionError::Denied` as a scope-level request. "Session" and "always" options. Remember-the-denial. Kill-switch: re-request-off ≡ Arthur says "I can't do that" and stops (no escalation).
3. **Slice E3:** Audit + revoke surface (D6). Conversation ("what can you do?", "revoke X") + shell (`arthur grants`, `arthur revoke`). Action log. Kill-switch: audit-off ≡ no surface (grants still enforced, just not inspectable).
4. **Slice E4:** Memory scope integration (OT-17 D3). Wire OT-17's per-category memory caps into the grant store and audit surface. Kill-switch: memory scopes unavailable → Arthur manages memory caps directly (OT-17's own mechanics).

**Each slice is independently kill-switchable.**

### 11.2 Honest uncertainties

| Uncertainty | Status | Resolution path |
|-------------|--------|-----------------|
| How many scopes does a real user grant? | Unknown | N=1 measurement. Start with six scopes (D2), measure usage, add or merge as needed. |
| Scope-level re-request inference — does parent directory always match user intent? | Tentative | If Arthur is denied `/home/project/src/lib.rs` and asks for `/home/project/`, the user likely meant that. Edge cases (denied `/etc/shadow` → asks for `/etc/`) are handled by the user's "no" on the re-request. |
| OT-19 reversibility timeline — when does it ship? | Unknown | OT-18's grant infrastructure (E1-E3) is independent of OT-19. The *width* of default grants depends on OT-19. Narrower defaults interim. |
| Grant store format — JSON or custom binary? | JSON for now | Human-readable, inspectable with `cat`. Matches the OT-17 memory schema's human-readability principle. Binary format is a performance optimization that doesn't earn its place at N=1. |

---

## 12. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Arthur grants itself a scope via re-request loop | Medium — authority creep | Remember-the-denial prevents loops. Each scope is requested at most once per session. The user controls the grant. |
| Grant store corrupt (`.arthur/grants.json` malformed) | Low — persistence record lost, not runtime authority | Arthur's *current* kernel caps are unaffected (they're kernel objects, not file-derived). The audit surface can't display grants from the file. On next Arthur spawn, the shell can't reconstruct durable grants from the corrupt file — Arthur falls to D7 defaults until the user re-grants. The kernel caps held during the current session remain valid until Arthur exits. |
| User revokes a scope Arthur is actively using | Medium — in-flight action denied | `revoke_handle` cascades immediately. The next cap-check fails with `Denied`. Arthur surfaces the denial to the user (D3). In-flight syscall may complete (the cap was held at call time) — this is correct: revocation affects future actions, not in-flight ones. |
| Scope taxonomy doesn't match user mental model | Medium — confusing grants | The six scopes map to the `Grant` enum, which maps to real actions. If the taxonomy is wrong, N=1 use will surface it. The version field in the grant store permits format evolution. |
| Arthur confidently wrong with wide grants | High — destructive action on wrong target | **This is D5's problem.** Bounded by reversibility (OT-19) + revocation (D6). Until OT-19 ships, narrower default scopes limit the blast radius. The dependency is stated, not hidden. |

---

**Status:** Design THINK complete. D1–D7 stated for PM review → PL ratification. No code. No CI gate. OT-18 remains OPEN until ratified.
