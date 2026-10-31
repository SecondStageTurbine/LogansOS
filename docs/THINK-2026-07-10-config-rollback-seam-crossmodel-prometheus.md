<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: The config↔rollback seam — cross-model independent check (Prometheus/Qwen)

**Date:** 2026-07-10
**Author:** Prometheus (Qwen family, research / design lane)
**Source:** Plans/DISPATCH-2026-07-10-config-rollback-seam-crossmodel-check-prometheus.md
**Branch:** prometheus/config-rollback-seam-crossmodel-check
**Inputs:** FINDINGS P1 (one config store, two views — macos-defaults.com, 3-0); FINDINGS P7 (never-mutate/A-B/kept-previous — rpm-ostree + transactional-update verified); rpm-ostree deployment docs (ostreedev.github.io/ostree/deployment/); snapper rollback source (github.com/openSUSE/snapper/client/snapper/cmd-rollback.cc); NixOS manual v26.05 (§sec-upgrading, rollback); protobuf schema evolution docs (developers.google.com/protocol-buffers/docs/proto3#updating); Kubernetes CRD versioning (kubernetes.io); Windows 10 1809 KFR incident (blogs.windows.com, Oct 2018).
**INDEPENDENCE GUARD:** I did NOT read `docs/THINK-2026-07-10-ot50-unified-config-substrate.md`, `docs/THINK-2026-07-10-ot51-update-rollback-model.md`, or the OT-50/OT-51 rows in OPEN-THREADS.md. I reasoned from the self-contained problem statement in the dispatch, FINDINGS P1/P7, and external prior art read firsthand.

---

## 0. The thesis in one breath

**Config lives BESIDE the deployment, not inside it. On system rollback, settings SURVIVE by default — because a rollback is "this version broke," not "undo everything I did." Version-skew is solved by schema-versioned keys with unknown-field preservation: older bases ignore keys they don't recognize, newer bases fill defaults for absent keys. The two axes — system version and config state — are independent, coupled only by a monotonic schema contract. The most dangerous blind spot in the "one store + reversible deltas" framing is treating config as a versioned artifact: it isn't. Config is user intent, not system state. Versioning it invites the exact drift-and-destruction failure mode that immutable OSes were built to avoid.**

---

## 1. ISC-1 — Where does config live relative to a deployment?

Three options: inside the rolled artifact, beside it (separately versioned), or a split.

### Inside the deployment (travels with the system)

**NixOS model:** `/etc` is generated atomically from `configuration.nix`. Rollback reverts both config and software together. No skew possible — config and software are one unit.

**Why it's tempting:** It eliminates version skew entirely. You can never have "v2 config for v1 software."

**Why it fails for our use case:** Our config store is **owner-authored policy**, not system-generated state. The owner says "turn dark mode on" or "set stakes to cautious" — these are preferences that express *who they are*, not *which version of the OS they're running*. If rolling back a kernel bug also reverts their dark-mode preference, the rollback has collateral damage on user intent. NixOS works this way because `/etc` is mostly service configuration (systemd units, daemon flags) — things that ARE coupled to the software version. Our config store is per-owner policy. Different thing.

### Inside the deployment + mutable overlay (the rpm-ostree model)

**rpm-ostree model:** The immutable commit contains `/usr/etc` (defaults). At deploy time, a 3-way merge produces a per-deployment `/etc`. The `/etc` is writable and per-deployment. `/var` is shared across all deployments.

**The critical behavior on rollback:** When you boot a previous deployment, you get the `/etc` that was merged when *that* deployment was created. Any `/etc` changes made after deploying the newer version are **lost** on rollback — they were merged into the new deployment's `/etc`, not the old one.

**Why this is worse than it sounds:** The 3-way merge at deploy time means `/etc` is a *derivative* of the deployment, not an independent artifact. If you change a setting, upgrade the OS, then rollback — your setting change might be in the new deployment's `/etc` but absent from the old one. The rollback silently reverts your preference. This is the "config drift" problem rpm-ostree users hit repeatedly.

### Beside the deployment (separately persisted)

**The claim:** Config is stored outside the deployment artifact, in a persistent location that survives across deployments. On rollback, the old system boots with the *current* config.

**Prior art for this split:**
- **rpm-ostree's `/var`:** Shared across all deployments, never touched by rollback. User data lives here.
- **openSUSE snapper (TRANSACTIONAL mode):** The entire rootfs including `/etc` is snapshotted. Rollback reverts everything — config AND system. But this is a *whole-disk* snapshot, not a split model.
- **Kubernetes CRDs:** Config (CustomResources) lives in etcd, separate from the control-plane version. Upgrading the API server doesn't revert your Deployments or ConfigMaps.

**Why this is the right answer for us:** Our config is owner policy — independent of which kernel or userland binary is running. The settings "dark mode on" and "stakes: cautious" are true regardless of whether the system is at deployment v1 or v2. Separating config from the deployment artifact makes rollback mean exactly what the owner intends: "this version broke, go back" — without silently reverting their preferences.

**The mechanism:** The config store lives under a persistent path that is NOT part of the deployment artifact. In our NeoFS model: `/neofs/.arthur/config/` is on the persistent NeoFS volume, which survives across deployment flips. The boot pointer changes which kernel/userland root is active; the config directory stays the same.

**Recommendation: Beside the deployment.** Config persists across rollbacks. It is NOT versioned with the system image.

---

## 2. ISC-2 — Default behavior on system rollback (the load-bearing UX call)

When the owner rolls the OS back to a previous deployment, **their settings survive.**

### Justification from owner intent

The owner's mental model for "rollback" is: "this version of the system is broken, use the old one." They are rolling back the *system*, not their *preferences*. If they want to revert a setting, they have a separate mechanism for that (the config store's own history — see ISC-4). Conflating "undo this version" with "undo everything I changed" makes rollback a sledgehammer when it should be a scalpel.

### Justification from prior art

| System | Config on rollback | Assessment |
|---|---|---|
| **rpm-ostree** | Per-deployment `/etc` — changes after upgrade are LOST on rollback | **Anti-pattern for our use case.** The 3-way merge couples config to deployment lifecycle. User settings silently revert. This is the #1 complaint in rpm-ostree forums: "I changed a setting, upgraded, rolled back, and my setting disappeared." |
| **snapper/MicroOS** | Full snapshot revert — `/etc` goes back to snapshot time | Also loses post-snapshot changes, but at least the loss is visible (it's a full filesystem revert, not a silent merge). Still an anti-pattern for owner-authored policy. |
| **NixOS** | Generated `/etc` reverts with the generation | Correct for service config (which NixOS `/etc` is), wrong for user preference. |
| **Kubernetes** | ConfigMaps/CustomResources survive control-plane upgrade/rollback | **The model to follow.** Your Deployments don't vanish when you roll back the API server version. |

### The Windows 1809 cautionary case

The KFR bug destroyed user files during an in-place upgrade because the OS update process touched user-visible paths. The immutable-deployment model structurally prevents this: the deployment artifact is read-only, and the upgrade process builds a *new* artifact beside the current one without touching it. Our config-beside-deployment model extends this protection to user settings: the rollback process changes the boot pointer, it doesn't mutate the config store.

**Recommendation: Settings survive rollback.** The owner's preferences are independent of which system version is active. If they want to revert a setting, they use the config history mechanism (ISC-4), not system rollback.

---

## 3. ISC-3 — The version-skew hazard

A setting was written against a newer config schema. The owner rolls back to an older base that doesn't define that key. What happens?

### The failure modes

| Behavior | Risk |
|---|---|
| **Reject the entire config** | **Fail-boot.** The system won't start because it can't parse its own config. Unacceptable. |
| **Crash on unknown key** | Same as reject — fail-boot. |
| **Silently drop unknown keys** | Data loss. If the owner upgrades again, those settings are gone. |
| **Ignore unknown keys, preserve them** | **Safe.** The older system runs with defaults for the unknown keys. When upgraded again, the keys are still there. |

### The protobuf model — unknown field preservation

**Source:** Protocol Buffers schema evolution docs (developers.google.com/protocol-buffers/docs/proto3#updating).

Protobuf's wire format identifies fields by number, not name. When an old parser encounters a field number it doesn't recognize:
- It does NOT reject the message
- It preserves the unknown field in memory
- When re-serialized, the unknown field is written back unchanged
- A newer parser can then read it correctly

This gives **forward compatibility** (old binary reads new data) and **backward compatibility** (new binary reads old data — missing fields return defaults).

### Applied to our config store

Each key in the config store carries a **schema version**. The config reader for deployment vN:
1. Reads all keys
2. For keys with schema version ≤ N: applies them normally
3. For keys with schema version > N: **ignores them, preserves them on write-back**
4. For absent keys that the current schema expects: uses the schema default

This is exactly protobuf's behavior, applied to a key-value store. The config file format must support unknown-field preservation — JSON doesn't (it's parsed into a typed structure that drops unknowns). A line-based or protobuf-encoded format would work. Or JSON with a convention: the config reader preserves the raw file and only mutates the keys it recognizes (write-back uses the original file as the base, not a re-serialized struct).

**Concrete mechanism:** The config store is file-backed (P1: files under `/neofs/.arthur/config/`). Each domain has its own file. The reader for deployment vN:
- Parses the file
- Extracts keys it recognizes (schema version ≤ N)
- Applies them
- On write, patches only the keys it manages — the rest of the file is preserved verbatim

This is a **surgical write-back**: the config reader is a lens over the file, not a full serializer. Unknown keys survive untouched.

**Recommendation: Ignore unknown keys, preserve them on write-back.** Fail-safe, never fail-boot. Schema-versioned keys with surgical write-back (protobuf-style unknown field preservation).

---

## 4. ISC-4 — Are the two axes independent?

**Yes.** "Roll back the system" and "roll back a setting" are independent operations, coupled only by the schema contract.

### The mechanism for independence

| Axis | Operation | Mechanism | What it reverts |
|---|---|---|---|
| **System version** | Rollback deployment | Flip boot pointer to previous deployment | Kernel, userland binaries, base config defaults |
| **Config state** | Revert a setting | Config store's own history (append-only log or file snapshots) | The specific key/value, not the whole system |

The config store maintains its own revision history. Each write to a config key appends to an append-only log (or creates a file snapshot). The owner can revert individual settings ("undo dark mode") without touching the system version. Conversely, rolling back the system version doesn't touch the config store.

### How they interact (the coupling)

The only coupling is the **schema contract**:
- Deployment vN declares which config keys it recognizes (schema version N)
- Config keys with schema version > N are ignored by deployment vN (ISC-3)
- When the owner upgrades back to a newer deployment, those keys become active again

This is a **monotonic** relationship: newer deployments recognize a superset of keys. Older deployments ignore keys they don't know. The config store doesn't need to know about deployments — it just persists keys with version tags.

### Prior art for this split

- **Kubernetes:** The API server version (system) is independent from CustomResource state (config). Upgrading the cluster doesn't revert your Deployments. CRD versioning handles schema evolution via conversion webhooks.
- **Protobuf:** Schema evolution is decoupled from message persistence. Old parsers read new data by ignoring unknown fields.

**Recommendation: Independent axes.** System rollback flips the boot pointer. Config revert uses the config store's own history. Coupled only by a monotonic schema contract (older systems ignore newer keys, preserve them on write-back).

---

## 5. ISC-5 — What would a Claude-style analysis likely MISS? (The cross-model value)

This is the highest-value section. I'm reasoning from a different model family's vantage — what might systematic priors of the "one store + reversible deltas + kill-switch discipline" framing blind a Claude agent to?

### Miss 1: Config is identity, not state

The "reversible deltas" framing treats config changes as *system mutations* that can be undone. But owner-authored settings are expressions of **identity** — "I prefer dark mode," "I trust this app with FileRead." These aren't mutations to be reverted; they're declarations of who the owner is. A Claude agent, thinking in terms of system state machines and reversible transitions, would naturally want to version config with the system. That's the wrong mental model. Config isn't a delta on the system — it's an overlay that the system respects.

**The discriminator:** Ask "would the owner be angry if this reverted on rollback?" For kernel binaries: no, that's the point. For "dark mode on": yes, absolutely. The anger test separates system state from owner intent.

### Miss 2: The surgical write-back problem is harder than it sounds

The protobuf model works because protobuf has a binary wire format that natively preserves unknown fields. JSON does not. If our config store uses JSON (which `grants.json` and `stakes.json` already do), the "preserve unknown keys on write-back" mechanism requires care: you can't parse-then-reserialize without losing unknown keys. You need a surgical patcher that only mutates recognized keys in the raw file.

A Claude agent would likely say "use protobuf" or "use a format that supports this" — which is correct but defers the problem. The real question is: **can we make this work with JSON files, since that's what we already ship?** The answer is yes, but it requires a config reader that operates on the raw file (regex-based key extraction and replacement, or a JSON patch library), not a full parse-serialize cycle. This is an implementation detail the design THINK should flag, not hand-wave away.

### Miss 3: The "config survives rollback" default has a real counter-case

There IS a case where config SHOULD revert with the system: **service configuration that is tightly coupled to a binary version.** If deployment v2 ships a new Arthur with a new config key `arthur.model_path`, and you roll back to v1 — v1's Arthur doesn't know about `model_path`. Our schema-versioning handles this (v1 ignores it). But what if v2 *changes the meaning* of an existing key? E.g., `arthur.timeout` meant "seconds" in v1 but "milliseconds" in v2.

Schema versioning catches *new* keys but not *changed semantics* on existing keys. This is the protobuf "wire-compatible but lossy" problem (int32↔int64 truncation). The defense: **existing keys never change meaning.** If a key's semantics change, it gets a new name and a new schema version. The old key is deprecated, not redefined. This is a discipline rule on config schema evolution, not a mechanism.

### Miss 4: The config store's own history is a feature, not an afterthought

The "reversible deltas" framing focuses on system-level reversibility (A-B deployment flip). But the config store needs its OWN reversibility — the ability to revert individual settings without rolling back the system. This isn't just "nice to have"; it's the mechanism that makes the two axes truly independent (ISC-4). Without config-level history, reverting a setting requires either manual editing or a full system rollback — both are wrong tools for the job.

**The minimal mechanism:** append-only log per config domain. Each write appends `{timestamp, key, old_value, new_value}`. Revert = replay backwards to the desired state. This is OT-19's copy-before-write pattern applied to config files. Small, mechanical, no new infrastructure.

### Miss 5: The boot-time config validation surface is a real UX problem

When the system boots a rolled-back deployment, it reads the config store and encounters keys it doesn't recognize. It ignores them (ISC-3). But should it *tell the owner*? "I'm running on deployment v1; these 3 settings are from v2 and won't take effect until you upgrade."

This is a butler-register surface (OT-46): Arthur reports at boot, "sir, I'm on the previous deployment. Three settings you configured are active only on the newer version — they'll resume when you upgrade." Not an error. A heads-up. This turns a silent degradation into transparent information.

A Claude agent focused on mechanism would implement the ignore-and-preserve and call it done. The UX surface — telling the owner what's dormant — is what makes the design feel honest rather than broken.

### Agreement: where the obvious answer survives scrutiny

After working through the adversarial angles, I agree with the shape that emerges from first principles: **config beside deployment, settings survive rollback, schema-versioned keys with unknown-field preservation, independent axes.** This isn't because a Claude agent would say so — it's because every prior art system that got this right (Kubernetes, protobuf) converged on the same split, and every system that got it wrong (rpm-ostree's per-deployment `/etc`, snapper's full-snapshot revert) suffered from coupling config to deployment lifecycle.

The convergence isn't same-model echo — it's the shape that survives contact with real systems that have shipped.

---

## 6. Summary of recommendations

| ISC | Question | Recommendation |
|---|---|---|
| **ISC-1** | Where does config live relative to a deployment? | **Beside.** Config persists across rollbacks on the NeoFS volume. Not part of the deployment artifact. |
| **ISC-2** | Default on system rollback — settings survive or revert? | **Survive.** Rollback = "this version broke," not "undo my preferences." The anger test. |
| **ISC-3** | Version-skew: newer key on older base | **Ignore unknown keys, preserve on write-back.** Schema-versioned keys. Surgical write-back for JSON files. Fail-safe, never fail-boot. |
| **ISC-4** | Are the two axes independent? | **Yes.** System rollback flips boot pointer. Config revert uses config store's own append-only history. Coupled only by monotonic schema contract. |
| **ISC-5** | What would a Claude analysis miss? | Five blind spots: (1) config is identity not state, (2) surgical write-back on JSON is hard, (3) changed semantics on existing keys, (4) config-level history is required for true independence, (5) boot-time dormant-key reporting surface. |

---

## 7. Primary sources cited

### Immutable OS / rollback mechanics
- OSTree deployment docs: ostreedev.github.io/ostree/deployment/ — `/etc` 3-way merge per deployment, `/var` shared across deployments, staged deployment mitigation
- snapper rollback source: github.com/openSUSE/snapper/client/snapper/cmd-rollback.cc — TRANSACTIONAL vs CLASSIC ambit, btrfs subvolume switch
- NixOS manual v26.05: nixos.org/manual/nixos/stable/#sec-upgrading — generated `/etc`, atomic generations, `/home`/`/var` mutable across generations
- rpm-ostree docs: coreos.github.io/rpm-ostree/layering/ — layered packages baked into commits, `/etc` overlay behavior

### Schema evolution / version skew
- Protocol Buffers schema evolution: developers.google.com/protocol-buffers/docs/proto3#updating — unknown field preservation, forward/backward compatibility, wire-safe changes
- Kubernetes CRD versioning: kubernetes.io/docs/tasks/extend-kubernetes/custom-resources/ — storage version, conversion webhooks, `status.storedVersions`

### Cautionary cases
- Windows 10 1809 KFR incident: blogs.windows.com/windows-insider/2018/10/ — Known Folder Redirection + OneDrive file deletion during upgrade, rollout paused Oct 4 2018

### Internal context (allowed)
- FINDINGS P1: one config store, two views (macos-defaults.com, 3-0)
- FINDINGS P7: never-mutate/A-B/kept-previous (rpm-ostree + transactional-update verified)

---

## 8. Close-gate

**INDEPENDENCE GUARD CONFIRMED:** I did not read `docs/THINK-2026-07-10-ot50-unified-config-substrate.md`, `docs/THINK-2026-07-10-ot51-update-rollback-model.md`, or the OT-50/OT-51 rows in OPEN-THREADS.md. All reasoning is from the dispatch's self-contained problem statement, FINDINGS P1/P7, and external prior art read firsthand.

**Docs-only branch.** No code, no QEMU. `scripts/worker-close-gate.sh` skipped with note — docs-only branch, no CI run exists.

**RETURN:**
- THINK path: `docs/THINK-2026-07-10-config-rollback-seam-crossmodel-prometheus.md`
- ISC-1: Config lives **beside** the deployment (persistent NeoFS volume, not part of deployment artifact)
- ISC-2: Settings **survive** rollback (anger test; Kubernetes model; rpm-ostree per-deployment `/etc` is the anti-pattern)
- ISC-3: **Ignore unknown keys, preserve on write-back** (protobuf model; schema-versioned keys; surgical write-back for JSON)
- ISC-4: **Independent axes** — system rollback flips boot pointer; config revert uses append-only history; coupled only by monotonic schema contract
- ISC-5: Five blind spots identified (config is identity not state; surgical write-back difficulty; changed semantics on existing keys; config-level history requirement; boot-time dormant-key reporting)
- Independence guard honored. Primary sources cited.

*Research THINK. No code. Host-only. Cross-model independent check — Qwen family, did not see Claude agents' answer.*
