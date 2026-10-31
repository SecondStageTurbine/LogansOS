<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-51 — the whole-system update / rollback model (A/B, verified base, kept-bootable-previous)

**Date:** 2026-07-10
**Author:** Cerebro (main-tier coder)
**Source:** Plans/DISPATCH-2026-07-10-ot51-update-rollback-model-think-cerebro.md
**Branch:** codex/ot51-update-rollback-model-think-cerebro
**Scope:** Design THINK — research + design only. No code, no QEMU, host-only. Design-ahead-of-build so the update model is *designed rollback-first, not bolted on*. Recommend a shape; the PL ratifies. NOT near-term (v1 is boot-the-OptiPlex-and-talk; this lands post-first-boot, near OT-28/OT-44).

**Inputs (read firsthand, not summarized):**
- Internal: `Plans/RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md` **P7 + the immutable-substrate addendum** (5 claims re-verified against upstream, 2026-07-08 — the vetted prior-art foundation this THINK stands on); `docs/OPEN-THREADS.md` OT-51 (banked summary), OT-50 (the unified config substrate — Oracle's parallel THINK), OT-44 (install-is-data / H2 verify-then-place pipeline), OT-19 (per-action reversibility + the D2 NeoFS-snapshot banking); `docs/specs/neofs-format.md` §1 (NeoFS is "copy-on-write ready" + content-addressing-ready, neither built); `boot/limine.conf` (Limine, `default_entry`, native multi-entry menu — Bible Article 4.4).
- External prior art (primary upstream docs, read directly — no summarizing fetch): libostree / rpm-ostree; openSUSE MicroOS / transactional-update. Cited in Sources.

**Acquisition honesty:** the dispatch (and the OT-51 relay) pointed at `docs/RESEARCH-…FINDINGS.md`; the file is actually at `Plans/RESEARCH-…FINDINGS.md` (the OT-51 ledger row cites it correctly; the PL confirmed and is fixing both dispatches). I located and read the correct file. RAG was healthy (`{"status":"healthy","ollama":true,"qdrant":true}`) and used for the NeoFS/boot facts above.

---

## 0. The one idea

**Never mutate the running system.** Build the next system state as a complete, verifiable artifact *beside* the current one; flip a boot pointer; keep the old state bootable, bit-for-bit. This is not a new invention we have to get right under pressure — it is **our kill-switch discipline promoted from per-feature to whole-system.** Our standing invariant is *"disabled ≡ the previous layer, bit-for-bit"*; the OS-image analog is *"the previous deployment is kept and bootable, bit-for-bit."* Two independent, adversarially-verified implementations (Fedora Silverblue/rpm-ostree/libostree; openSUSE MicroOS/transactional-update) converge on exactly this move, and one mechanism serves both audiences: the **beginner** gets unbreakability as a *default* (verified base, every change power-loss-safe and reversible from the boot menu, no user care required); the **power user** gets depth through the *same* recorded-delta ledger (`neo> rollback`/`reset` to a known state). "Undo a bad update" and "undo my customization" become the **same reflex**, not two subsystems.

The thesis this THINK adds: **because LogansOS is a monolithic single-artifact OS with a capability substrate and one typed config store, we get the immutable-A/B model *more cheaply and more cleanly than the distros that invented it* — and the smallest first slice needs no filesystem work we haven't already designed for.**

---

## 1. ISC-1 — The A/B deployment mechanism (concrete, grounded in verified prior art)

### 1.1 What the two verified systems actually do (read firsthand)

**libostree / rpm-ostree** (ostreedev.github.io/ostree): a **content-addressed object store** like git — files are checksummed and stored once in `/ostree/repo`; a **deployment** is a set of *hardlinks* into that store at `/ostree/deploy/$STATEROOT/$CHECKSUM`, so it is immutable (hardlinks require immutability) and **deduplicated by construction** (identical files across deployments cost one copy). Read-only `/usr` (a read-only bind mount "to prevent inadvertent corruption"). Each deployment has its **own writable `/etc`** (upgrades do a **three-way merge** of local `/etc` changes onto the new tree, leaving the old untouched) and there is **exactly one shared `/var`**. An upgrade builds a **new deployment offline** and applies it **at reboot**, keeping **≥2 bootable deployments**; the atomic commit point is the **bootloader config swap**, and libostree's guarantee is verbatim: *"if the system crashes or you pull the power, you will have either the old system, or the new one."* Rollback = pick the previous boot entry or `rpm-ostree rollback`; layered packages are **recorded reversible deltas on the pristine base**, and `rpm-ostree reset`/`uninstall` returns to a pristine known state.

**openSUSE MicroOS / transactional-update** (kubic.opensuse.org): the *same pattern* on **btrfs snapshots** — snapper **creates a new snapshot before any update**, the update is applied *into that snapshot* (`zypper -R`), and on success the snapshot is set **read-only and as the new default**; on error it is deleted and the running system is untouched. The atomicity is at reboot: *"if the power would have been pulled before, the unchanged old system would have been booted; now the new, updated system will boot… not possible to have a running system in an intermediate state."* Read-only root gives each snapshot an **`/etc` overlay** in `/var/lib/overlay`; rollback on a read-only system is a direct btrfs default-swap (`snapper rollback` = a true subvolume swap — kernel, RPM db, everything rolls back together *because they are one snapshot*).

The convergence is the signal: two independent teams, two different storage substrates (content-addressed hardlink store vs btrfs CoW snapshots), **same three load-bearing moves** — (a) build-beside-never-in-place, (b) atomic activation at a boot-pointer flip, (c) keep the previous bootable.

### 1.2 The mechanism proposed for LogansOS

A **deployment** is our unit of update and rollback: a self-contained, verifiable `{kernel ELF + verified userland root + the base config it requires}`. Concretely:

1. **Verified base, built beside.** An update never touches the running deployment. It **writes a new deployment** into a free slot (§6) and **verifies it as data** — hash + publisher signature + structural check — *before* it is ever eligible to boot. This is not a new pipeline: it is **OT-44 H2 "install-is-data" at whole-system scale** (discover → download → verify → place-atomically → register). A deployment is a signature/hash-gated verifiable artifact exactly like an app bundle.
2. **Atomic boot-pointer flip.** Activation is a single crash-safe mutation: point Limine's `default_entry` at the new deployment's entry (and register the new entry) via a **write-then-activate** step so a power loss lands on *either* the old default *or* the new one, never a corrupt config. Limine already reads `boot/limine.conf` and natively supports multiple entries + `default_entry` (Bible Article 4.4), so "flip a pointer, keep the previous bootable" is **not new bootloader work** — it is registering a second entry and swapping one field, atomically. The flip is the commit point, mirroring libostree's "bootloader config swap is the atomic step."
3. **N kept-bootable-previous.** Keep the last N deployments as Limine entries (N≥2 is the floor: current + one previous = A/B). Rollback for anyone, including a beginner whose userland is broken, is **pick the previous entry at the Limine boot menu** — the power-loss-safe floor that works even if nothing above the bootloader runs.
4. **A content-addressed/versioned store — designed-for, sequenced.** Deployments live in NeoFS. NeoFS is already **content-addressing-ready and copy-on-write-ready** (`neofs-format.md` §1) but implements neither yet, so the *first* slice stores whole deployments (2× space for A/B — cheap, no FS work), and **block-level dedup between deployments** (ostree's hardlink-store payoff) arrives when NeoFS builds the CAS/CoW it is already designed to accept (§6). Nothing about the mechanism blocks on that.

---

## 2. ISC-2 — The LogansOS framing (why we may have it *easier*)

The distros that invented this model paid a tax we mostly do not owe, because they are **package graphs over a POSIX filesystem** and we are a **monolithic single artifact over a capability substrate**.

- **No package-graph drift.** rpm-ostree's complexity (layered-package trees, the three-way `/etc` merge, the "software that straddles `/usr`+`/var` or drops into `/opt` will over time cause drift" caveat the FINDINGS verified) exists because a distro is thousands of independently-versioned packages scattering files across the tree. **We ship one kernel artifact + one verified userland root.** A "deployment" for us is far closer to *a single versioned image* than to a package tree — there is no scattered-package drift to reconcile because there is no scattered package set.
- **No split `/etc` merge, because config is ONE store.** ostree needs a per-deployment writable `/etc` and a three-way merge because POSIX config is a pile of files. Our config is (per **OT-50**) **one typed, capability-gated store** — so "carry the user's config across an update" is a *defined operation on one store* (§5), not a filesystem merge heuristic. The hardest, most bug-prone part of the prior art largely evaporates.
- **The deployment is a first-class capability artifact.** Because everything is a verifiable artifact in our world, a deployment is **signature/hash-gated like any OT-44 bundle** — the *same* verify-then-place primitive secures an app install, a config change, and a whole-OS update. One trust mechanism, three scales. A deployment that fails verification is rejected *as data*, never executed (the P4 "no step executes vendor code with elevated power" invariant, at OS scale).
- **The kill-switch is already our native reflex.** Every build already ships "disabled ≡ previous layer, bit-for-bit" with a regression proving it. Promoting that to "previous *deployment* kept and bootable, bit-for-bit" is a *generalization of a discipline we already practice*, not a foreign concept we import. The mental model is home-grown.

The one place we are *not* easier: rpm-ostree's hardlink store and btrfs both give **cheap** dedup for free; our first slice does not (whole-image A/B is 2× until CAS/CoW lands). That is the honest cost, and it is a storage cost, not a correctness cost (§6, §7).

---

## 3. ISC-3 — One mechanism, two audiences

The whole point (P7): **one recorded-delta ledger, viewed at two depths** — the OT-50 promotion-affordance lens (layer-2 visible-but-quiet from layer 1), applied to time/versioning.

- **Beginner — unbreakability as a *default*, requiring zero user care.** The base is verified and read-only; every change is built beside and power-loss-safe; if an update or a change makes the machine unhappy, the **boot menu** offers the previous known-good deployment. Arthur surfaces it in plain language ("that update didn't sit well — I've put yesterday's system back; nothing of yours was lost"). The beginner never learns the word "deployment"; they experience *the machine cannot be bricked.*
- **Power user — depth through the *same* ledger.** `neo> status` lists deployments; `neo> rollback` boots the previous; `neo> reset` returns to a pristine known state; layered customizations are **recorded reversible deltas on the pristine base**, inspectable and revertible. The power user gets a versioned, scriptable, deterministic view (S1–S8 of the duality) of *the exact same substrate* the beginner rides blind.
- **Never two systems.** This is the FINDINGS' one-sentence result — *one substrate wearing two views, the user chooses when to go deeper, the safe posture never degrades.* There is no "advanced update mode" that turns off the safety. Depth here is *more of the ledger made visible*, never *fewer guarantees* (the P5 power-is-not-a-jailbreak invariant, at the update layer).

---

## 4. ISC-4 — The unification (the payoff): one reflex, a delineated family

The prize is folding *"undo a bad update"* and *"undo my customization"* into ONE boot-menu / `neo>` / Arthur reflex — **"put it back the way it was"** — that resolves to the right layer **by scope**. The reversibility family, each layer owning a distinct granularity so they reinforce and never duplicate:

| Layer | Reverses | Granularity | Owner / mechanism |
|---|---|---|---|
| **OT-19** | an *action* | a single operation inside a running system | userland snapshot-before-destructive-op + undo window |
| **OT-44 install-is-data** | an *install* | one app/bundle | verify-then-place; uninstall = delete + revoke (zero residue) |
| **OT-50** | a *setting* | one config key/delta | typed config store; delta revert |
| **OT-51 (this)** | a *system version* | the whole deployment (kernel + userland root + base config) | verified A/B deployment + atomic boot-pointer flip + kept-previous |

**How they compose into one reflex.** The owner says "put it back" (to Arthur, or `neo>`, or by picking a boot entry). Arthur resolves the **scope**: *an action* → OT-19 undo; *an app* → OT-44 uninstall; *a setting* → OT-50 delta revert; *a bad update / a broken system* → OT-51 deployment rollback. The **boot menu is the floor** — the one that works when userland is too broken for Arthur to answer (the beginner's power-loss-safe net). The unifying property across all four: **reversibility is enforced by the substrate, never by user care** — which is precisely why one reflex can be the beginner's safety net and the expert's escape hatch. OT-51 is the *outermost* ring: when the finer rings can't help (the update itself broke the thing that runs them), the deployment rollback still boots.

Consistency check against the kill-switch: an OT-51 rollback IS the whole-system kill-switch firing — "revert to the previous layer, bit-for-bit," where the "layer" is a deployment. The discipline is identical; only the granularity changed.

---

## 5. ISC-5 — The OT-50 config seam (stated from the update-model side; PM synthesizes with Oracle)

> **PM note — SUPERSEDED by the ratified cross-model correction (2026-07-10; see OPEN-THREADS.md OT-51 seam clause).** This section's "config as reversible deltas on the OT-50 store" framing was corrected by a Qwen independent check (`docs/THINK-2026-07-10-config-rollback-seam-crossmodel-prometheus.md`): **config is *identity, not state* — it keeps its OWN independent append-only history and does NOT ride this (OT-51) reversible-delta ledger**, coupled to the system only by a monotonic schema contract. The "settings survive rollback" conclusion below is RATIFIED and correct; only the "rides the ledger" mechanism is superseded. See the ledger seam clause for the authoritative contract.

A whole-system rollback must have **stated, non-surprising** semantics for the one config store (OT-50). Three shapes:

1. **Config *inside* the rolled artifact** (the ostree/transactional-update default — `/etc` or the snapshot travels with the deployment). Rolling back the system rolls config back too. Simple and internally consistent ("the system exactly as it was"), but *"roll back the bad update, keep my new settings"* is impossible without extra machinery.
2. **Config *beside* the artifact as recorded reversible deltas** (config is the separately-versioned OT-50 store; a deployment references a config-generation but config rolls independently). Both "roll back system, keep settings" and "keep system, roll back a setting" are possible. More flexible; the hazard is **schema drift** — a delta written against deployment N may reference a key deployment N-1 doesn't define.
3. **Span both** — base/system config that is semantically *part of* a deployment travels inside it; *user* config lives beside it as OT-50 deltas.

**My recommendation: shape 3, defaulting to "your settings survive a system rollback."** User configuration is recorded as reversible deltas on the OT-50 store *beside* the deployment, so the non-surprising default is: **"roll back the system" keeps your settings; rolling back a *setting* is a separate, equally-easy `neo>`/Arthur reflex** (OT-50 delta revert). This matches the beginner's intuition ("undo the broken update, don't throw away my stuff") and the power user's expectation (system version and config version are independently addressable). *System*-level config a deployment genuinely requires (e.g. a kernel parameter tied to that build) travels *inside* the artifact so a deployment is always self-consistent.

**The honest hazard to design against (flag to Oracle + PL):** config-schema drift across deployment versions. The OT-50 store must **version its schema**, and a rolled-back deployment must **degrade gracefully on unknown keys** (ignore-with-note, surfaced by Arthur — never fail-boot on a config key from the future). This is the one place shape-3 costs more than shape-1, and it is worth it. I state this seam from my side; the PM reconciles with Oracle's OT-50 THINK — the two must agree that config is a *view/version over one store*, and that "keep my settings" is the default across a system rollback.

---

## 6. ISC-6 — The NeoFS snapshot-primitive dependency (honest)

**What A/B needs, minimally:** the ability to store two complete deployments and boot either. That is **two files/regions and a boot entry** — it needs **no CoW, no snapshots, no CAS.** It works on today's NeoFS (single-tier, physical-block-addressed, monotonic-bump allocation) at the cost of storing each deployment whole (2× for A/B). So **the smallest slice has no filesystem dependency at all.**

**What the *space-efficient, N-deep* version needs:** block-level dedup between deployments (so "base + deltas" replaces "N whole images") and cheap snapshotting. NeoFS is explicitly **"copy-on-write ready" and preserves the fields for content-addressing** (`neofs-format.md` §1) but implements **neither** yet (Phase-3 BUILD-2/3C scope guards excluded CoW hardening and free-space reuse beyond bump allocation). This is the ideal posture: **the primitive is designed-for, not a retrofit.** It also aligns with OT-19 D2, which already banked a NeoFS snapshot primitive for per-action reversibility — OT-51's dedup/snapshot need and OT-19's snapshot-before-op need are **the same NeoFS capability**, built once.

**Honest sequencing (the answer to "hard dependency or approximate-first?"):** **approximate with whole-image A/B first** (ships now, 2× space, one kept-previous), then land **CAS/CoW in NeoFS** to turn 2× into "base + deltas" and make N-kept-previous cheap. CoW/CAS is an **optimization NeoFS is already designed to accept, not a prerequisite.** One more small primitive to name: the **atomic boot-pointer swap** (write-then-activate of `default_entry`) must itself be crash-safe; on the boot medium this is a single-field/one-file atomic replace — bounded and well-understood, and the commit point the whole guarantee rests on.

---

## 7. ISC-7 — Honest trade-offs + smallest-first path + forks

### 7.1 Verified trade-offs (from the FINDINGS, plus ours)

- **Activation needs a reboot** (both prior arts, verified). This is the *source* of the atomicity guarantee, not a defect — "old-or-new, never intermediate" requires the switch to happen where nothing is running. For a personal OS this is a fine cost. **Explicitly out of scope: live-kernel-patching** — the immutability/atomicity is worth more than avoiding a reboot.
- **Layering is expected to be sparing** (verified). Deep, constant per-package layering fights the model; our monolithic artifact makes this a non-issue (we don't have a package graph to layer).
- **`/usr`+`/var`-straddling / `/opt` drift** (verified) — the prior art's chronic pain. **We largely avoid it** (§2): one artifact, one config store, no scattered ecosystem.
- **Our own cost:** whole-image A/B is **2× storage** until CAS/CoW lands (§6). A storage cost, not a correctness cost, and it buys unbreakability from slice 1.

### 7.2 Smallest-first path (NOT near-term — post-first-boot, near OT-28/OT-44)

- **Slice 0 (today):** one mutable image; Limine boots `/boot/kernel`. No A/B.
- **Slice 1 — whole-image A/B + Limine two-entry + boot-menu rollback.** Two deployment slots; an update writes the new slot, verifies it as data (OT-44 H2), and atomically flips `default_entry`; the previous slot stays a Limine entry; rollback = pick it at the menu. **No new FS primitive.** This is the smallest slice that delivers *unbreakability-by-default* — ship this first.
- **Slice 2 — verified read-only base.** The base deployment is root-hash/signature-verified at boot (a NeoFS root-hash check, the dm-verity analog) and read-only. Makes "the deployment is a verifiable artifact" (§2) and the kill-switch framing concrete.
- **Slice 3 — recorded config/customization deltas on the OT-50 store** (power-user depth + "keep my settings across a rollback"). Depends on OT-50 landing (§5).
- **Slice 4 — CAS/CoW dedup in NeoFS → cheap N-kept-bootable-previous.** NeoFS builds the CoW/CAS it is already designed for (shared with OT-19 D2); "base + deltas" replaces "2× whole image."
- **Slice 5 — the unified "put it back" reflex** across OT-19/44/50/51 (§4), surfaced in Arthur / `neo>` / the boot menu, resolving by scope.

Each slice is additive and kill-switchable (disabled ≡ the prior slice); Slice 1/2 touch boot + FS layout (kernel-touching ⇒ the eventual builds inherit the QEMU gate); Slices 3–5 are largely userland + FS.

### 7.3 Forks needing PL ratification (with my recommendation)

1. **Config-survives-rollback default (the real one).** *My rec:* shape 3 — "roll back the system" **keeps your settings** by default (config beside as OT-50 deltas), rolling back a setting is a separate reflex. Fork because the simpler ostree-like alternative (shape 1: config rolls with the system) is more internally consistent but violates beginner intuition. **Compose with Oracle's OT-50 THINK** — the PM synthesizes.
2. **Whole-image A/B now vs wait for CAS/CoW.** *My rec:* ship whole-image A/B first (accept 2× storage), land CAS/CoW later. Really a sequencing call, but the PL should ratify accepting the interim storage cost.
3. **Reboot-to-activate accepted; no live-kernel-patch.** *My rec:* yes — accept the reboot; it *is* the atomicity guarantee. Confirm the PL agrees this is fine for a personal OS.
4. **N — how many kept-bootable-previous.** 2 (A/B) is the floor; deeper history is cheap only after CAS (Slice 4). A policy knob; *my rec:* start at 2, make N a config key once CAS lands.

---

## RETURN-relevant summary (for the relay)

- **Recommended A/B mechanism:** a **deployment** = `{kernel ELF + verified userland root + required base config}`; update = write-beside → **verify-as-data (OT-44 H2)** → **atomic Limine `default_entry` flip** → keep the previous entry(ies); rollback = boot the previous (menu floor) or `neo> rollback`. Grounded firsthand in libostree (content-addressed store, deployments as hardlinks, read-only `/usr`, bootloader-config-swap = atomic commit, "old-or-new on power loss") and transactional-update (btrfs snapshot-before-update, atomic-at-reboot, snapper subvolume-swap rollback).
- **Monolithic-artifact-makes-it-easier:** one artifact + one config store ⇒ no package-graph drift, no three-way `/etc` merge, no `/usr`+`/var` straddle; a deployment is a signature/hash-gated capability artifact (same verify-then-place primitive as an app install and a config change).
- **Unification with the reversibility family:** OT-19 (action) ⊂ OT-44 (install) ⊂ OT-50 (setting) ⊂ **OT-51 (system version)** — one "put it back" reflex resolving by scope, boot-menu as the floor; an OT-51 rollback *is* the whole-system kill-switch firing.
- **Seams:** OT-50 — config beside the deployment as reversible deltas, "keep my settings" is the default across a system rollback, schema-versioning guards drift. NeoFS — whole-image A/B needs no new primitive; CoW/CAS dedup is a designed-for optimization (shared with OT-19 D2), not a prerequisite; the atomic boot-pointer swap is the one small crash-safe primitive.
- **Verified trade-offs:** reboot-to-activate (the source of atomicity, not a defect), sparing-layering, `/usr`+`/var` drift (which we avoid); our own cost is 2× storage until CAS/CoW.
- **Forks:** config-survives-rollback default (rec: yes, compose w/ Oracle); whole-image-A/B-first (rec: yes); reboot-accepted/no-live-patch (rec: yes); N-kept-previous (rec: start at 2).

---

## Sources (primary upstream docs, read directly — no summarizing fetch)

**Internal:** `Plans/RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md` (P7 + immutable-substrate addendum, 5 claims re-verified vs upstream 2026-07-08); `docs/OPEN-THREADS.md` OT-51 / OT-50 / OT-44 / OT-19; `docs/specs/neofs-format.md` §1 (CoW-ready / CAS-ready, neither built); `boot/limine.conf` + Bible Article 4.4 (Limine, `default_entry`, multi-entry menu).

**External:**
- libostree — overview & deployment model (content-addressed store, `/ostree/deploy/$STATEROOT/$CHECKSUM`, read-only `/usr`, three-way `/etc`, shared `/var`, bootloader-config-swap as the atomic commit, "old system or new on power loss") — https://ostreedev.github.io/ostree/ ; introduction — https://ostreedev.github.io/ostree/introduction/
- Fedora Silverblue / rpm-ostree (new deployment offline → reboot, ≥2 kept, `rpm-ostree rollback`/`reset`, layers = reversible deltas) — https://docs.fedoraproject.org/en-US/fedora-silverblue/ ; https://coreos.github.io/rpm-ostree/
- openSUSE transactional-update / MicroOS (snapshot-before-update, atomic-at-reboot, read-only root + `/etc` overlay, `snapper rollback` subvolume swap) — https://kubic.opensuse.org/documentation/man-pages/transactional-update.8.html ; how-it-works — https://kubic.opensuse.org/documentation/transactional-update-guide/tu-howitworks.html ; Forster, "Transactional Updates with Btrfs and RPM" (Linux Foundation) — https://events19.linuxfoundation.org/wp-content/uploads/2017/11/Transactional-Updates-with-Btrfs-and-RPM-Ignaz-Forster-SUSE.pdf

---

## Gate + close

- **Docs-only ⇒ no CI run exists** (paths-ignored: `docs/**`, `**.md`). No push CI, no QEMU — correct for a research/design THINK.
- Ran the `scripts/worker-close-gate.sh` host-set intent locally: this branch adds exactly one file (`docs/THINK-2026-07-10-ot51-update-rollback-model.md`), no source touched — fmt/clippy/host-test legs are no-ops on this diff (skip-with-note per the docs-only carve-out); SPDX header present (CC-BY-4.0, first line). Self-stat below is the Step-4 self-inspection.
- **RETURN evidence:** `git diff --stat origin/main` = 1 file added, docs-only, no phantom deletions. Diffed this artifact against the RETURN summary before sending (accuracy axis).
- **Compose, not contradict:** the OT-50 seam (§5) is stated from the update-model side for the PM to synthesize with Oracle's parallel config-store THINK; I did not read Oracle's THINK.
- **Disk reclaim:** after the PM merges (or declines) this branch, delete the isolated worktree `E:/AI Automation/AltimitNeo-ot51` (`git worktree remove`) — docs-only, no `target/`, cheap; reclaim once the work is safe on `main`.
