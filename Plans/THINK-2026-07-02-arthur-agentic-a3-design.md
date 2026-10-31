<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Arthur agentic A3 design - destructive and irreversible-external tier

**Date:** 2026-07-02
**Author:** Janus
**Branch:** `codex/agentic-a3-design-think-janus`
**Inputs:** Dispatch `Plans/DISPATCH-2026-07-02-phase-8-arthur-agentic-a3-design-janus.md`; OT-26 ratified agentic design (`Plans/THINK-2026-06-21-arthur-agentic-action-design.md`); A2 dispatch (`Plans/DISPATCH-2026-06-29-phase-8-arthur-agentic-a2-janus.md`); OT-18/OT-19 rows in `docs/OPEN-THREADS.md`; Article 29.3 in `docs/bible.md`; current `userland/libneo/src/action.rs` action/grant surface.
**Scope:** Design THINK only. No production code, no grammar change, no cap-check change, no QEMU/AAG gate.

---

## 1. Problem statement

A1 made read/navigation actions safe by construction. A2 added a narrow mutating tier: a model-selected `WriteFile` is staged, must be confirmed, then executes through the unchanged `execute_action()` cap check with a bounded pre-write snapshot so undo can restore the old bytes. A2 also explicitly restricted delete, move, kill, spawn, and external/network actions.

A3 is the next safety boundary: actions whose effects may be destructive, process-affecting, resource-expensive, or externally observable, and where A2's snapshot-before-write trick does not always generalize. You cannot un-send a network payload. You cannot reliably restore a killed process's side effects. You cannot treat a spawned process as harmless if it inherits file, process, model, or network handles. A3 therefore needs a stronger model than "confirm, then undo if wrong."

The A3 rule is:

> No A3 action executes silently unless it has a concrete recovery path. If recovery is impossible, the user must explicitly type-confirm the specific irreversible act. An ungranted destructive/send action remains impossible, not merely unconfirmed.

Article 29.3 holds harder here, not softer: NL proposes; `execute_action()` and the kernel capabilities dispose. Confirmation and preview never mint authority.

---

## 2. Current substrate facts

- `ActionKind` currently contains `ReadFile`, `WriteFile`, `ListDir`, `SpawnProcess`, `KillProcess`, `InspectProcess`, `WaitProcess`, `NetDial`, `NetSend`, `NetRecv`, `Infer`, `LoadModel`, and `CloseFile`.
- There is no first-class `DeleteFile` or `MoveFile` action in the current `Action` enum. A3 must treat "delete" and "move" as user-visible destructive file intents that require a follow-on action surface before they can be built.
- Current grants already model the E5 surface in userspace shape: `SpawnProcess { name }`, `InheritHandle`, `Process { handle, rights }`, `NetDial { host, port }`, `TcpSocket { handle, rights }`, `ModelLoad { path }`, `ModelUse`, and `TensorUse`.
- The current executor checks every action against a `Grant`/`RequiredCapability` before syscalls. A3 must route through that unchanged disposal path.
- OT-18 says Process/Network/Model grants are banked as E5 until real need. A3 is that real need.
- OT-19 says reversibility is structural, not model-judged; irreversible external actions are not made reversible and must be bounded by restriction.

---

## 3. A3-D1 - action set and irreversibility taxonomy

### Recommendation

A3 should classify actions by effect class, not by action name alone. The grammar may expose user-friendly verbs, but the gate is selected from the resolved action's effect class after parameters, grants, and recovery feasibility are known.

| Class | Meaning | Actions / intents | A3 posture |
|-------|---------|-------------------|------------|
| R: Recoverable local | A concrete undo artifact or trash record exists before mutation. | A2-compatible `WriteFile`; future delete-as-trash; same-filesystem move with journaled source/destination. | Execute under the A2 recovery model. No irreversible typed confirm required. Surface "undo available" after action. |
| T: Recoverable-by-conversion | The user's destructive intent can be converted into a recoverable operation. | "delete file" implemented as move to `.arthur/undo/`; "replace file" implemented as copy-before-write plus restore metadata. | Convert to R. If conversion fails, do not fall through silently; escalate to L or refuse. |
| L: Irreversible local | Local effect cannot be exactly reversed, but does not itself send data outside the machine. | `KillProcess`; permanent delete/unlink; truncating overwrite without complete preimage; spawn of a local process with no external handles; model load/infer when only local compute/resource effects are involved. | Dry-run preview plus typed confirmation if the action is allowed by caps. Some subcases may be outright refused. |
| X: Irreversible external | The action emits data or externally observable traffic, or starts a process with external-capable authority. | `NetDial`, `NetSend`, `NetRecv` sessions; spawn inheriting a network socket or external-send capability; process that is allowed to dial/send; future external model/provider actions. | Explicit Network/Process/Model grant required. Dry-run preview plus typed confirmation. External-send policy remains PL-ratification-sensitive. |
| U: Unsafe or unbounded | Blast radius cannot be represented in a narrow cap, preview, or typed phrase. | Wildcard network grants; recursive wipe; spawn with broad inherited handles; send payload whose origin cannot be explained; permanent delete of `.arthur/` state; kill of Arthur's supervising shell/session unless PL later ratifies. | Outright refuse. No typed confirmation override in A3. |

Concrete A3 action set for the follow-on build:

1. **File destructive intents:** `TrashFile` / delete-as-trash, future `MoveFile`, future `DeletePermanent`.
2. **Process actions:** `SpawnProcess`, `KillProcess`; read-only `InspectProcess` and `WaitProcess` remain lower risk but are used to produce previews.
3. **Network actions:** `NetDial`, `NetSend`, `NetRecv`.
4. **Model actions:** `LoadModel`, `Infer` with local model/tensor grants and resource preview.
5. **Control actions:** `dry_run`, `confirm_typed`, `cancel`, `refuse`, and `undo`/`restore` where recovery exists.

### Rationale

The same action name can belong to different risk classes depending on parameters. A same-filesystem "move to trash" is recoverable; a permanent unlink is not. `SpawnProcess` without inherited handles is local and bounded by the binary grant; `SpawnProcess` with a writable file cap or network socket can become destructive or external. `NetSend` is always irreversible-external because the payload may leave the machine.

### Rejected alternatives

- **Action-name-only gates:** rejected because they over-confirm safe converted actions and under-specify dangerous parameter combinations.
- **Treat delete/move as already present in `ActionKind`:** rejected because the current action surface does not contain those variants. The build must either add explicit actions or keep them restricted.
- **Make all process/model actions A2:** rejected because `KillProcess`, `SpawnProcess`, and model loading have side effects that snapshot-file undo does not cover.

---

## 4. A3-D2 - gate model when undo is impossible

### Recommendation

A3 uses a four-step gate ladder. The action must stop at the first applicable rung.

| Rung | Gate | Applies when | Result |
|------|------|--------------|--------|
| 1 | **Outright refuse** | No cap exists; payload/source/destination is unpreviewable; requested scope is wildcard/broad; operation targets protected Arthur state; user asks for permanent high-blast-radius destruction. | No pending action is created. Arthur explains the missing cap or forbidden class. |
| 2 | **Convert to recoverable** | A destructive file intent can be implemented as trash/journal/snapshot. | Stage or execute as a recoverable action, with undo metadata. |
| 3 | **Dry-run preview** | Recovery is absent or incomplete, but the action is representable and cap-authorized. | Show target, authority, effect class, recovery status, and a confirmation phrase. No syscall yet. |
| 4 | **Typed confirmation** | The user accepts the exact irreversible preview. | Execute the exact previewed action only if caps still match and the preview hash is still current. |

Typed confirmation is not a generic `yes`. It must bind the class and target:

- `trash /path/to/file`
- `permanently delete /path/to/file`
- `kill pid 42`
- `spawn neo-fetch with network api.example.com:443`
- `send 128 bytes to api.example.com:443`
- `load model /models/qwen.gguf`

The pending A3 action stores a preview digest. Confirmation is valid only for that pending digest, expires quickly, and is invalidated by any new user request that changes target/action/grants. A wrong phrase, stale phrase, or grant revocation is a no-op.

Dry-run previews should be specific but not noisy:

- **File destructive:** path, operation, byte count if known, whether trash/snapshot exists, undo window, and whether metadata is preserved.
- **Kill:** pid, process name, process state, owner/principal if available, and dependent children if inspect can see them.
- **Spawn:** binary, args, inherited handles, resource grants, network/model/file capabilities passed to the child, and whether any inherited handle makes the spawn external-capable.
- **Network:** host, port, socket handle, direction, byte count, payload class, payload digest, and source of payload. Do not dump large or secret payloads into the console by default.
- **Model:** model path, model handle/path grant, tensor handles, expected output count, and approximate memory/resource cost when known.

### Rationale

This keeps OT-18's humane grant model while stopping the dangerous conflation of "has a standing cap" with "may silently destroy or send." A standing cap makes an action possible; recovery or typed confirmation decides whether the agentic NL layer may cause it.

### Rejected alternatives

- **Generic yes/no confirmation:** rejected because it does not bind the user to the exact irreversible target.
- **Per-action prompt for every A3-shaped operation:** rejected because recoverable conversions should use the A2 safety net instead of turning Arthur into a prompt machine.
- **Model-judged risk:** rejected for the same OT-18/OT-19 reason as before: trusting the model's stakes estimate is circular.
- **Execute after grant approval in the same turn:** rejected for A3. Grant approval and destructive/send confirmation must be separate steps so "give Arthur the cap" cannot be confused with "fire this exact irreversible action now."

---

## 5. A3-D3 - cap acquisition through OT-18 E5

### Recommendation

A3 opens OT-18 E5 for Process, Network, and Model grants. E5 should extend the existing grant lifecycle rather than creating an A3-specific authority path.

**Process E5:**

- Durable grant shape: allow spawning a named binary or binary path, optionally with a constrained argument pattern once the shell has a real parser for it.
- Session grant shape: allow control of a concrete `ProcessHandle` with specific rights (`INSPECT`, `WAIT`, `KILL`), usually for a process Arthur spawned or the user selected.
- Inherited handles are explicit grants. `SpawnProcess` must require both `SpawnProcess { name }` and `InheritHandle(handle)` for every handle passed to the child.
- A spawn that passes a writable file cap, process-control cap, network socket, or model/tensor authority inherits the highest effect class of those handles.

**Network E5:**

- Durable grant shape: `Network(host, port)` for dial authority only. No wildcard host, no wildcard port, and no "all network" grant in A3.
- Session grant shape: `TcpSocket(handle, READ|WRITE)` minted by a successful, cap-authorized dial and scoped to that socket.
- `NetDial` with a standing host:port grant may open the socket only after its A3 preview/confirmation rule has been satisfied for external observability. `NetSend` always goes through external-send policy (see PL open point).
- `NetRecv` is not exfiltration but still belongs to an externally observable session; it should require the socket read cap and use the same session preview/logging surface.

**Model E5:**

- Durable grant shape: `ModelLoad(path)` for a specific model path or model directory, not arbitrary filesystem load.
- Session grant shape: `ModelUse(handle)` and `TensorUse(handle)` for already loaded model/tensor handles.
- Model actions are local unless an external backend/provider is introduced later. A3 should still preview resource cost and handles because model load/infer can consume scarce local resources.

**On-miss re-request:**

- A missing E5 cap surfaces as an OT-18 scope-level request, not as an A3 confirmation.
- If the user grants the scope, Arthur records/acquires the cap, then returns to a new dry-run/confirm step. It must not execute the original A3 action as the side effect of granting.
- Denials are remembered per session to avoid nagging.

### Rationale

This preserves Article 29.3. The grant store and confirmation state are UX/audit/control surfaces; the kernel cap is still the enforcement boundary. A compromised Arthur that skips the preview or edits `grants.json` still lacks the kernel object or rights needed for the syscall.

### Rejected alternatives

- **A3-local allowlist outside OT-18:** rejected because it creates a second authority system.
- **Broad Process/Network/Model grants:** rejected because the blast radius cannot be previewed or revoked meaningfully enough for A3.
- **Let typed confirmation bypass caps:** rejected. Confirmation is user intent, not authority. The cap-check must still be able to deny after confirmation.

---

## 6. A3-D4 - reversibility ceiling and no silent destruction/send

### Recommendation

A3 should state the reversibility ceiling explicitly:

- File writes are recoverable only when the preimage is captured and restoration semantics are exact enough for the current filesystem.
- Delete is recoverable only when implemented as trash/move with enough metadata to restore.
- Move is recoverable only when source, destination, overwrite state, and metadata are journaled.
- Kill is not recoverable.
- Spawn is not recoverable once the child has run user code; it may be terminated, but its side effects are not undone.
- Network send is not recoverable.
- Network dial/recv are externally observable session effects and must not be treated as pure local reads.
- Model load/infer are local resource actions today; if a future model backend sends data off-machine, it moves to irreversible-external.

Hard rule:

> A3 never performs silent destruction or send. It either proves a recovery path first or requires typed confirmation of the irreversible act after a dry-run preview.

This does not require a new complexity-ledger invariant for the design. It rides existing substrate:

- Article 29.3 for authority.
- OT-18 D5 for blast-radius split.
- OT-19 D6 for "can't reverse -> confirm-before-destructive."
- I-15 and the cap graph for file-cap enforcement.
- The existing `execute_action()` requirement that actions are checked before syscall.

The follow-on build should encode this as an application-level A3 safety contract and test matrix. It should add a ledger invariant only if the build introduces a new cross-cutting kernel guarantee or a new handle/cap graph rule. A userland confirmation/refuse state machine alone does not earn a new invariant.

### Rationale

"Undo" is a real mechanism, not a label. Once the mechanism stops being exact, A3 must stop pretending A2 safety still applies. The fallback is explicit typed consent or refusal.

### Rejected alternatives

- **Best-effort undo for irreversible actions:** rejected because it produces false confidence.
- **Silent irreversible-local actions when user policy says low-stakes:** rejected for agentic A3. OT-19 allowed that in a broader behavioral model, but this A3 NL layer is specifically the tier opened because A2 cannot safely generalize. For the build, no silent irreversible action is simpler and more auditable.
- **New kernel policy hook:** rejected. The policy belongs in Arthur/userland; the kernel enforces caps and mechanisms.

---

## 7. A3-D5 - kill-switch and ISC decomposition

### Recommendation

A3 should remain under the existing `agentic-actions` feature family with a subordinate A3 tier gate. When A3 is disabled, the behavior must be bit-for-bit equivalent to the current A2 posture: A1/A2 actions still work as they do today, while delete/move/kill/spawn/external actions remain restricted with the A2 restricted response.

Proposed build decomposition:

| ISC | Scope | Deliverable |
|-----|-------|-------------|
| ISC-A3-1 | Grammar/action taxonomy | A3 ASTs for dry-run, typed confirm, cancel/refuse, process/network/model/file-destructive intents. No new action bypass. Byte-oracle fixtures for every new AST. |
| ISC-A3-2 | Preview and typed-confirm state machine | Pending action digest, typed phrase generation, expiry, stale-confirm rejection, wrong-phrase no-op, grant-revocation recheck before execution. |
| ISC-A3-3 | OT-18 E5 grants | Process/Network/Model grant-store parsing/serialization, on-miss re-request, session-vs-durable grant handling, no grant+execute shortcut. |
| ISC-A3-4 | Recoverable file destructive tier | Trash/journal behavior for delete/move-equivalent intents where possible; downgrade to A2 recovery if exact restoration is available. |
| ISC-A3-5 | Refuse tier and external-send policy | Wildcard/broad scopes refused; unpreviewable payloads refused; external sends gated by PL-ratified policy. |
| ISC-A3-6 | Failure-mode scenarios | Host KATs plus eventual QEMU scenario set: malformed impossible, ungranted denied, no-confirm no-op, wrong confirm no-op, stale preview no-op, trash undo, kill typed-confirm, spawn inherited-handle denial, no silent network send, kill-switch A2-equivalence. |

### Rationale

The split keeps A3 from becoming one large "dangerous things" patch. The grammar can be proven first, confirmation/refusal can be proven independently, then E5 grant acquisition can be wired without changing `execute_action()`.

### Rejected alternatives

- **One monolithic A3 build:** rejected because Process/Network/Model grants plus confirmation/refuse plus file trash semantics are separable risk surfaces.
- **A new global feature separate from `agentic-actions`:** rejected because A3 is a tier inside the agentic layer, not a separate product mode.
- **Gate A3 by model size or confidence:** rejected because safety must not depend on model self-assessment.

---

## 8. External-send policy - PL ratification point

This THINK recommends the conservative interim policy:

> A Network(host:port) grant permits Arthur to stage and preview external network actions for that destination. It does not, by itself, authorize silent `NetSend`. Every `NetSend` requires a per-send typed confirmation binding host, port, byte count, and payload digest/class.

PL should ratify one of these before the A3 build:

| Option | Policy | Tradeoff |
|--------|--------|----------|
| A | Per-send typed confirmation for every `NetSend`. | Safest and simplest to audit; high friction for chatty protocols. |
| B | Standing send grant by host:port plus payload class, size bound, and source class. | Better UX for APIs; requires a real policy vocabulary and stronger preview/logging. |
| C | Refuse agentic `NetSend`; allow only deterministic `neo>`/shell network commands. | Lowest agentic risk; fails the "Arthur can operate the OS naturally" ceiling for network tasks. |

Recommendation for first build: **Option A**. Bank Option B until PL defines the payload-class vocabulary. Reject Option C as the default because A3's purpose is to design the safe path for external actions, not to permanently remove them.

---

## 9. Failure-mode enumeration

| Failure mode | Required A3 response |
|--------------|----------------------|
| Malformed A3 AST | Impossible under constrained grammar; parser rejects any hand-injected malformed block. |
| Valid AST for ungranted action | `execute_action()` denies or the E5 request path asks for a scope; no action executes. |
| User grants missing E5 scope | Cap is acquired/recorded, then action returns to preview. It does not execute as part of granting. |
| Granted but no typed confirmation | No-op; pending preview may remain until expiry. |
| Wrong typed phrase | No-op; show expected action class/target without executing. |
| Confirmation after target/grant changed | No-op; preview digest mismatch forces a fresh dry-run. |
| Trash conversion succeeds | Action executes as recoverable; undo can restore. |
| Trash/snapshot conversion fails | Escalate to typed irreversible confirm or refuse. |
| Permanent delete requested on protected state | Refuse. |
| Kill target disappeared | No-op or denied by stale handle; never kill a newly resolved "same name" process. |
| Spawn asks to inherit ungranted handle | Denied before spawn. |
| Spawn has external-capable inherited handle | Classified as X and requires the external confirmation path. |
| `NetSend` without socket write cap | Denied. |
| `NetSend` with cap but no typed send confirmation | No-op. |
| Payload cannot be previewed or classified | Refuse. |
| Model load lacks `ModelLoad(path)` | Denied/on-miss E5 request. |
| Model infer lacks `ModelUse` or `TensorUse` | Denied. |
| A3 kill-switch off | A2 behavior exactly: A3 classes remain restricted. |

---

## 10. Summary decisions

| Decision | Recommendation |
|----------|----------------|
| A3-D1 | Classify by irreversibility/effect class: recoverable local, recoverable-by-conversion, irreversible-local, irreversible-external, unsafe/unbounded. Current delete/move are intents, not existing `ActionKind`s. |
| A3-D2 | Use refuse -> convert-to-recoverable -> dry-run preview -> typed confirmation. No generic yes/no. No grant+execute shortcut. |
| A3-D3 | Open OT-18 E5 for Process/Network/Model through the existing grant lifecycle. Caps are acquired by scoped grants and kernel objects; confirmation never mints authority. |
| A3-D4 | State the reversibility ceiling. No silent destruction/send: recovery path or typed irreversible confirmation. No new ledger invariant unless the build changes kernel cap semantics. |
| A3-D5 | Keep A3 under `agentic-actions` with an A3 tier gate. Off equals current A2 restricted behavior. Build as grammar, confirmation, E5 grants, recoverable file tier, refuse/external-send policy, failure-mode scenarios. |

**Open PL point:** external-send policy. This THINK recommends per-send typed confirmation for first A3, and asks PL to ratify whether a future standing send grant can ever allow silent `NetSend` within a host:port plus payload-class scope.

**Status:** Design THINK complete for PM review. No code. No QEMU/AAG gate. The follow-on A3 build dispatch can lift the D1-D5 decisions and ISC split above.
