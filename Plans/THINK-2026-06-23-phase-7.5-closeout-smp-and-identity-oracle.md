<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase-7.5 closeout — the SMP AP-bringup hang (OT-27c) + the 7.5-D identity-wiring capstone

**Date:** 2026-06-23
**Author:** Oracle
**Branch:** `codex/phase-7.5-closeout-smp-and-identity-oracle` (off `origin/main` `90774f9`)
**Dispatch:** `Plans/DISPATCH-2026-06-23-phase-7.5-closeout-smp-and-identity-oracle.md` (one of four parallel closeout-research lanes; the PM synthesizes all four into the Phase-7.5-closeout PLAN).
**Status:** RESEARCH/DESIGN THINK — **no code, no QEMU, no kernel change.** Charter A is characterization + a repro/discriminator plan (not a fix); Charter B is a design (not a build).
**Method:** A 7-agent parallel read of `smp.rs` / `scheduler.rs` / the mt7601u driver / `tlb_shootdown.rs` / `net/device.rs` / `process.rs` / `file_cap_logic.rs` + OPEN-THREADS / complexity-governance / bible Art 13 / the two prior 7.5 THINKs, with the load-bearing claims (the I-14(a) lock-isolation check, the TLB-shootdown readiness filter, the AP-bringup lock set, the Handle inventory) re-verified first-hand. Cited `file:line` are at `90774f9`.

---

# Charter A — the intermittent SMP AP-bringup hang (OT-27c)

## A.0 What is documented (and what is not)

OT-27(c) (`docs/OPEN-THREADS.md:60–61`) records *"the intermittent SMP AP-bringup hang"* — **OPEN, no known repro, not the bare-metal-connect blocker.** Crucially, **no symptom is recorded**: not the hang's CPU, not a stack, not a timeout, not a frequency. So this charter does two things: (1) it **narrows the search space by refuting the structurally-impossible classes** against the actual code, and (2) it leaves a **ranked discriminator bank** so the *next* occurrence self-localizes instead of being another undated "it hung once."

## A.1 Where AP bring-up actually sits (the path is shorter than it looks)

Bring-up is **Limine-MP-driven**, not hand-rolled INIT-SIPI-SIPI (`arch/x86_64/smp.rs`):

1. **BSP side — `bring_up_aps()` (`smp.rs:69–118`):** for each AP, `apic::register_lapic_id(...)` then `cpu.bootstrap(ap_entry, cpu_id)` (`:113`). **Limine issues INIT-SIPI-SIPI and the long-mode trampoline; the kernel never touches real mode, A20, or the SIPI vector.** `bring_up_aps()` is **fire-and-forget** — it logs *"started N AP(s)"* and returns **without waiting for any AP** (`:117`). There is **no BSP↔AP per-AP spin here.**
2. **AP side — `ap_entry()` (`smp.rs:121–163`), interrupts masked throughout:** `enable_sse` → `per_cpu::init_ap(cpu_id)` (writes `IA32_GS_BASE`) → GS sentinel check `current_cpu_id()==cpu_id` else `halt_forever()` (`:142–150`) → `init_traps_ap` (GDT/IDT/LAPIC/syscall MSRs) → `mm::slab::init_per_cpu(cpu_id)` → `sched::run_ap_bootstrap(cpu_id)`.
3. **`run_ap_bootstrap()` (`scheduler.rs:277–288`):** `init_cpu_scheduler()` — a **lock-free** `Box::into_raw` + `PER_CPU[idx].scheduler_ptr.swap(.., AcqRel)` with `assert!(old.is_null())` (`scheduler.rs:303–339`) — **then** `enable_interrupts()` (`:282`, the **first** point the AP takes maskable interrupts) → `mark_tlb_shootdown_ready()` (`:283`) → idle loop `{ yield_now(); spin_loop(); }` with **interrupts on**.

Two readiness flags gate the AP, both published by the AP and read with `Acquire`: `scheduler_ptr != null` (scheduler routing; `scheduler.rs:1292`) and `tlb_shootdown_ready` (TLB targeting; `per_cpu.rs:224`). **The only lock on the entire AP path is the AP's own per-CPU scheduler `SwitchMutex`, taken (interrupts-off) inside `yield_now()`/`rotate()` — after the AP is already live.**

## A.2 I-14(a) lock-isolation — VERIFIED on both paths (the invariant holds)

The dispatch asks: does the driver IRQ/poll path take any lock the AP-bringup path holds? **No — verified first-hand:**

- **Driver IRQ/poll-drain** takes only `xhci::EVENT: IrqSafeMutex` (`drivers/usb/xhci.rs:254`, drained at `:623` from both the ISR `:615` and the poll path); driver *state* is a `SwitchMutex` at lock-order **`USB_DEVICE`=29** (`mt7601u/mod.rs:3010`), above every AI level (≤28). **No AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) is reachable from any xHCI completion** — matching I-14(a), proven on silicon BUILD-A/B (`complexity-governance.md:43`, *"no AI-governance lock on any USB ISR path"*).
- **AP-bringup** takes only the per-CPU scheduler `SwitchMutex` — **not** an AI lock, **not** the `USB_DEVICE` lock.
- **∴ the AP-bringup path and the driver share no lock.** A classic shared-lock deadlock between them is **structurally impossible**, and I-14(a) is intact for both paths. *(This is the charter's first concrete result: the hang is not a governance-lock contention.)*

## A.3 Refuted classes (narrowing — each with its refuting evidence)

The obvious "USB-driver-spin breaks AP-bringup" stories do **not** survive the code:

- **R1 — TLB-shootdown to a not-ready / wedged AP → initiator spins forever. REFUTED as a *silent hang*.** `shootdown()` intersects its target set with `shootdown_ready_cpus_mask()` (`tlb_shootdown.rs:164–166, 211–221`) — a CPU is only targeted **after** it sets `tlb_shootdown_ready` (i.e., after `run_ap_bootstrap` enabled interrupts), so a mid-init AP is **never** targeted. And `wait_for_acks_or_panic` is **bounded**: it re-pokes after 1 ms and then **`panic!("tlb_shootdown_timeout … unacked_slots=0x… repoked=0x…")`** (`:234–258`). So a shootdown fault **announces itself as a panic with the stuck CPU mask** — it cannot be the *silent* intermittent hang. **Corollary discriminator: the absence of a `tlb_shootdown_timeout`/`tlb_shootdown_slot_busy` panic in the captured serial REFUTES the entire TLB class; its presence localizes the culprit CPU directly.**
- **R2 — real-mode / A20 / SIPI-vector corruption (cf. generic OSdev priors). N/A.** Limine owns the INIT-SIPI-SIPI + long-mode trampoline; `ap_entry` is a 64-bit entry. No kernel code touches that window.
- **R3 — maskable IPI delivered to a half-initialized AP (bad GS / uninit APIC). REFUTED.** The AP runs all of `ap_entry` (GS, IDT, LAPIC, slab) **interrupts-masked**; `enable_interrupts()` is the *last* step of `run_ap_bootstrap` (`scheduler.rs:282`), after GS+IDT+APIC are up. A reschedule/TLB IPI arriving earlier is **latched in the APIC IRR**, not delivered to a bad context.
- **R4 — AP blocks on a *shared* scheduler lock the BSP holds. REFUTED.** Per-CPU schedulers are independent; `init_cpu_scheduler` is **lock-free** (`AtomicPtr::swap`), and the AP's first `yield_now()` takes **its own** per-CPU lock. There is no global scheduler lock for the AP to block on.
- **R5 — driver bulk-poll deadlocks bring-up by holding `USB_DEVICE` across a multi-second spin. REFUTED as a *deadlock*; reclassified as I-14(b) liveness.** The driver *control* path (`bind_inner`/`associate`) does hold `USB_DEVICE`/`PRIMARY` across `inband_out_sync` (~1 s) and `mcu_wait_resp` (~5 s worst case) spins (`mt7601u/mod.rs:850–957`), **interrupts enabled** (no `cli`; `spin()`/`udelay` at `:638–649`). But the AP path needs none of those locks (A.2), so there is no deadlock — only the BSP being **busy**, which is exactly the I-14(b) *liveness-degradation* vector (governance degrades to the P4b backstop, never fails). It can starve, not wedge.

## A.4 The surviving live hypotheses (ranked) + discriminator bank

With the lock-deadlock, TLB, trampoline, and half-init-IPI classes refuted, an **intermittent *silent* SMP hang** points at the two places that genuinely combine SMP + context-switch + a missed-wakeup/lock-timing window:

### H1 (top) — scheduler cross-CPU wakeup race: the first work routed to a freshly-ready AP, or a coalesced reschedule IPI
**Mechanism.** Routing work to an AP uses `scheduler_ptr != null` as the readiness gate, but the AP only services a reschedule IPI **after** `enable_interrupts()` (`scheduler.rs:282`), which is *after* `scheduler_ptr` is published (`:268`-ish via `init_cpu_scheduler`) and *after* `mark_tlb_shootdown_ready`. In the window **[scheduler_ptr published → enable_interrupts]** the AP looks routable but cannot yet take the wakeup IPI; if the enqueue+IPI lands there, the APIC IRR latches one edge and the AP, on entering its idle loop, must re-observe the ready queue on its own. If the enqueue path relies on the IPI edge (rather than the idle loop re-checking the queue), the item sits unran and whoever waits on it hangs. The same shape produces a **coalesced/lost reschedule IPI** generally: two same-vector IPIs to a CPU with interrupts momentarily masked collapse to one IRR bit → one delivery → a missed wakeup.
**Why intermittent.** The bad window is microseconds wide and only matters when an enqueue+IPI to a specific AP coincides with that AP's `[publish → enable_interrupts]` gap or a masked instant — timing-dependent on boot interleave and on the driver's long spins shifting when the BSP enqueues.
**Discriminator (capture-before-you-need-it — these must be added before a confirming run):** a per-CPU **reschedule-IPI `sent` vs `handled` counter pair** (mirror the *existing* `neo-test-instrumentation` TLB counters `record_initial_ipi_send`/`record_ipi_receipt` in `tlb_shootdown.rs:172–194`), plus an **AP-lifecycle timestamp trail** (`ap_entry` entered / `scheduler_ptr` published / `enable_interrupts` / first ready-queue dequeue), each `serial_println!`'d with `cpu_id` + `rdtsc`. **Confirms** if, at the hang, an AP shows `sent > handled` (a swallowed wakeup) or shows `scheduler_ptr published` + a routed item but **no** `enable_interrupts`/first-dequeue line. **Refutes** if every routed AP logs `enable_interrupts` and `sent == handled`.

### H2 — OT-31: the `exit()`/reap lock-release-across-context-switch window, under SMP spawn/reap churn
**Mechanism.** `exit()` (`sched/process.rs`) releases its lock across a context-switch boundary **without** the `context_switch_with_release2` guard the IPC block paths use (`OPEN-THREADS.md:45–46`). On SMP, a reaper on another CPU can observe the exiting thread's scheduler/slab state mid-release — a real, **unaudited** race. The W1/W2 lane spawns/reaps processes (Arthur, workers; the `sched_spawn_reap_spawn_heavy` stress is the archetype), so it exercises exactly this. OT-31 was **excluded as the OT-29 *slab-corruption* cause** (the poison-instrument trial reorder failed identically) but **was never proven safe as a hang source.**
**Why intermittent.** Needs a reap on CPU B to land inside the exit's release window on CPU A — a narrow cross-CPU interleave that heavy churn makes occasionally hit.
**Discriminator.** Audit the `exit()` release boundary against the `context_switch_with_release2` precedent (a static read, no run needed); then a **spawn→reap-under-`NEO_SMP=4` repro** with a lock-state breadcrumb at the exit/reap boundary (the lock owner + the thread state at release). **Confirms** if the hang reproduces under churn and the breadcrumb shows a reaper acting on a thread whose exit hadn't completed its release. **Refutes** if the audit shows the release is in fact ordered, or the churn repro never wedges.

### H3 — a per-CPU field read before the AP published it (a reader not gating on the readiness flags)
**Mechanism.** A1's flags (`scheduler_ptr`, `tlb_shootdown_ready`) are correct *only if every reader honors them*. A reader of `PER_CPU[ap]` that does not gate on the relevant flag (e.g., a scheduler-routing or accounting path that assumes a CPU is up) reads stale/uninit state in the bring-up window.
**Why intermittent.** Same narrow window as H1.
**Discriminator.** A grep+audit for `PER_CPU[..]` / `current_per_cpu()` consumers that touch a CPU other than self **without** first checking `scheduler_ptr`/`tlb_shootdown_ready`; for any found, an `init-done` sentinel + a *"read of CPU N before ready"* breadcrumb. (Overlaps H1's routing path; rank below H1.)

**Ranking rationale.** H1 is top because it is the one *silent* missed-wakeup window the code actually leaves open and it is keyed to "AP-bringup" timing (the first cross-CPU dispatch). H2 is a documented, unaudited SMP race the lane provably exercises. H3 is a narrower variant of H1's window. R1–R5 are refuted and should not be re-investigated without new evidence.

## A.5 OT-31 vs OT-27(c): related or orthogonal?

**Orthogonal subsystem, plausibly-shared race class — and the verdict hinges on the (undocumented) symptom phase.** If OT-27(c) is literally a **boot-time AP-bring-up** wedge (no steady-state process churn yet), OT-31 (steady-state `exit`/reap) is **orthogonal** → H1/H3 are the candidates. If "AP-bringup hang" is the looser "SMP hang in the W2 lane" (with the lane's spawn/reap running), OT-31 is **a live candidate** (H2). Both are SMP+context-switch+lock-timing, so a single root could shadow both — but proving OT-31 safe would **not** prove OT-27(c) fixed (different code), and vice-versa. **Recommendation:** the OT-31 audit is cheap and standalone (static read of one boundary) — do it regardless; it either removes H2 or surfaces a real fix. Capturing H1's symptom (A.4) is what actually disambiguates the two.

## A.6 Repro / discriminator plan (not a fix)

1. **Pre-instrument (build requirement, the captures named in H1/H2/H3):** extend `neo-test-instrumentation` with (a) per-CPU reschedule-IPI sent/handled counters (sibling to the TLB ones already at `tlb_shootdown.rs:172–194`), and (b) the AP-lifecycle timestamp trail in `smp.rs:ap_entry` + `scheduler.rs:run_ap_bootstrap`. These are diagnostic-only, behind the test feature.
2. **Drive the repro on `NEO_SMP=4`** with the W1/W2 workload (the spawn/reap churn that co-occurs with the reported intermittency). The run must be **conclusive on pass or fail**: a hang now leaves the counters + trail + (if any) the `tlb_shootdown_timeout` panic on serial.
3. **Read the verdict off the breadcrumbs:** `tlb_shootdown_timeout` present → R1 was wrong, follow `unacked_slots`. Else an AP with `sent>handled` or a missing `enable_interrupts` line → H1. Else a churn-correlated wedge with the exit/reap breadcrumb → H2. **No fix is proposed here** — the next dispatch builds the fix once a discriminator fires.

---

# Charter B — the 7.5-D identity / interface-wiring capstone

## B.0 What 7.5-D is (grounded in the lane plan, refined by this dispatch)

The 7.5 lane plan (`THINK-2026-06-11`, §2) scopes **7.5-D = "identity wiring: hotplug interface lifecycle (link up/down on USB attach / WiFi associate), multi-interface extension of `select_source`, interface naming/identity — software/policy, (a)/(b) only."** It is explicitly *"wiring (hotplug + multi-interface), not stack-building."* This dispatch (Charter B) refines it with the **cap/identity shape** question. So 7.5-D is the **neutral interface-identity layer** that unifies `eth0`-class (USB-Ethernet) and `wlan0` (WiFi) under one nameable model the bible-Art-13 connection-management surface (Art 9.10 / ADR-017) and userland can name — **not a second per-device control plane.**

## B.1 Today's surface and the gap (verified)

Both backends implement the **same** kernel-internal `NetDevice` trait (`net/device.rs:63–74`) and carry a `NetDeviceIdentity { driver, name, pci_bdf }` (`:10–15`) — but:
- **Names are `&'static str` literals** baked per driver (`"usb-ecm0"` at `cdc_ecm.rs:538`, `"wlan0"` at `mt7601u/mod.rs:2772`).
- **The registry is an index-`Vec`** (`drivers/net/mod.rs` `register_device`/`get_device`) — identity is *insertion order*, the names are log strings.
- **Net access is ambient** — `socket.rs` hardcodes `get_device(0)` (`:312, :346`); ARP/NDP take an `interface_id` that is always a constant; sockets are minted with full rights and ambient address selection (`process.rs` `Handle::Socket`, `full_socket_rights()`); `process.ipv6_addresses` is the only "identity," and it is kernel-provisioned, not cap-held.
**Gap:** no stable interface identity, no name-based lookup, no multi-interface routing, and **no capability naming an interface** — so the connection-management surface has nothing neutral to name.

## B.2 The interface-identity model (the core of 7.5-D)

A small **interface registry** above `NetDevice`, transport-agnostic:
- A stable **`InterfaceId`** (a generation-stamped index or small id) + a registry keyed by it, wrapping today's `Vec` (so `register_device` returns/records an `InterfaceId` and a **name** assigned by a runtime scheme — `eth0/eth1/wlan0/…` — instead of a baked literal). This fixes B.1's naming/lookup gap and gives the connection-management surface a neutral handle.
- **Hotplug lifecycle:** USB attach/detach and WiFi associate/disassociate drive each interface's `LinkState`/presence in the registry (the events already exist at the driver edge; 7.5-D *wires* them to registry transitions). The connection-management surface observes link up/down here.
- **Multi-interface `select_source`:** extend Phase-4's RFC-6724 source selection (`net/interface.rs`) from its current single-/dual-family pick to choose among the registry's **UP** interfaces (eth0 *and* wlan0), replacing the hardcoded `get_device(0)`. This is the "wiring" half — routing across both backends.

## B.3 The cap / identity shape (reuse, don't invent)

Design space for "who holds the cap to an interface," all on the **frozen** Phase-2 primitives (`Handle` carries `CapRights{meta:USE/DERIVE/GRANT/REVOKE, semantic}`; `derive_handle`/`ipc::grant`/`revoke_handle` at `process.rs:937–1030`):

- **Option 1 — new `Handle::Interface` variant. REJECTED.** A new Handle kind is a new primitive — exactly what the dispatch + the invariant budget forbid.
- **Option 2 — `Handle::Socket` + an `interface_rights::BIND_INTERFACE` semantic bit** (attenuate a socket's bind authority to one interface). Lighter, zero new namespace — but it conflates *socket-bind* with *interface-identity*, and gives no **nameable** interface object the connection-management surface can hold or that can be granted/audited on its own. A useful *socket-scoping* mechanism, not an identity model.
- **Option 3 (RECOMMENDED) — an interface is a named node addressed by a `Handle::File` cap (I-15).** The registry publishes each interface as a named node under a connection-management namespace (e.g. `/net/if/wlan0`), and the cap to an interface is a **file-cap** via the already-landed I-15 mapping (`file_cap_logic.rs`). This is the most identity-faithful and the *highest-reuse* option:
  - **Naming** comes for free (the node path is the neutral name the surface + userland use).
  - **The cap rides I-15 + the frozen graph unchanged** — `derive_handle`/`ipc::grant`/`revoke_handle` already work on `Handle::File`, so an interface can be **granted and revoked exactly like the OT-18 file scopes** (the E1b-derive / E3-revoke lifecycle applies verbatim: a user grants Arthur `wlan0`, revokes it, and the kernel cap is the boundary per Art 29.3). **Zero new Handle kind, zero new cap mechanism.**
  - **Composition:** `select_source`/`socket bind`, instead of `get_device(0)`, resolves the interface node → `InterfaceId` → `Arc<dyn NetDevice>`. A socket bound under a held interface cap is gated by it; revoke kills the binding through the existing graph walk.
  - **Cost (stated honestly):** Option 3 needs a **small synthetic interface namespace** to host the nodes (a minimal devfs-style surface, or interface nodes minted through the existing `install_cap` path). That is new *surface*, but it *composes* I-15 + the registry — it modifies no frozen primitive.

**Recommendation:** Option 3 for the identity/cap model (it is what "identity wiring" means and it reuses the E1–E3 grant lifecycle), with Option 2 available as the lighter socket-scoping mechanism if the PLAN wants to defer the namespace. **Critically, 7.5-D's *core* deliverable is the identity + naming + hotplug + multi-interface `select_source`; capability-*gating* of net access (sockets *requiring* an interface cap instead of ambient `get_device(0)`) is the kill-switchable extension on top** — so that 7.5-D disabled is bit-for-bit today (B.4).

## B.4 Kill-switch (Rule 1)

Feature `interface-identity` (or `identity-wiring`), **default-off**. Off ⇒ `register_device` keeps index semantics, `select_source`/`socket` use the ambient `get_device(0)` path, hotplug transitions are no-ops, and no interface cap is minted — **bit-for-bit identical to 7.5-C2.** The regression: the existing `eth0`/`wlan0` bring-up + DHCP/DNS scenarios pass unchanged with the feature off (the "disabled ≡ previous layer" proof, `complexity-governance.md` Rule 1). The cap-gating sub-switch is a second default-off layer so the identity/naming can land before net access is ever gated.

## B.5 Invariant budget (Rule 2)

**No new cross-cutting invariant is expected.** The interface cap rides **I-15** (existing); the grant/revoke semantics are the **frozen** Phase-2 graph (no new row). The registry adds at most one new lock — a **leaf or an ordered lock below the USB band**, inside the existing `1..=31` lockdep domain (`USB_DEVICE`=29/`USB_XHCI`=30/`USB_CORE`=31), so it **does not breach the OT-13 lock-order ceiling** (no domain widening). The only candidate new invariant — *InterfaceId/name uniqueness* — is a **local registry property**, not a cross-cutting ledger row, and is enforced by the registry's own insert check; it should be named in the build only if the implementation finds it must be ledger-global (per Rule 2, named+justified+guarded in the same build, not smuggled). If net-access cap-gating later wants a "no ambient net access" invariant, that is a *future* build's row, not 7.5-D's.

## B.6 I-14 fault-containment analysis (mirroring the W2 §4 pattern)

7.5-D is **software/policy, no DMA, no ISR** — so, like the W2 lane, the vectors mostly collapse, stated explicitly so an audit reads it rather than re-deriving:

- **(a) Lock isolation — N/A by construction.** 7.5-D has **no interrupt handler**; the interface registry + `select_source` + cap paths run in process/socket/hotplug-event context and acquire **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`). A connectivity-driver fault cannot deadlock or contend the registry (the registry lock is never taken from an xHCI/IRQ path). *Guard:* the existing grep-checkable "no-AI-lock-in-IRQ" assertion + a grep that the registry lock is not acquired in any ISR.
- **(b) Liveness degradation — inherited/bounded.** A driver dying (interrupt storm, firmware crash) makes its interface go **link-down** in the registry; `select_source` falls back to another UP interface or to "no route," and **governance is untouched** (the residency daemon never reads the interface registry; under CPU monopolization it degrades to P4b's synchronous direct-reclaim backstop, the I-2 kill-switch — slows, never fails). Stated, not new.
- **(c) Memory isolation — N/A (no device-initiated DMA on this path).** 7.5-D holds only **metadata** (an `InterfaceId`, a name, link-state, an `Arc<dyn NetDevice>`); it issues no DMA. Device DMA stays at the `NetDevice`/driver layer, already fenced — **(c1)** the `FencedDmaAddr` arena gate and **(c2)** VT-d, both silicon-proven (`complexity-governance.md:43–44`, BUILD-A/B/V). A driver fault cannot reach governance or tensor memory **through the identity layer** because the identity layer has no memory path to reach it by.

**Conclusion:** 7.5-D is I-14-containable on all three vectors today, with (a)/(c) N/A-by-construction and (b) inherited — exactly the W2 disposition, and the reason the lane plan tagged 7.5-D "(a)/(b) only."

---

## Deliverable / RETURN

This THINK delivers both charters: **Charter A** — the OT-27(c) characterization (the AP path, the I-14(a) verification that the bring-up and driver paths share no lock, the five refuted classes with their refuting evidence, the ranked H1–H3 + discriminator bank with the captures to add, the OT-31 related-vs-orthogonal verdict, and a conclusive repro plan — *not a fix*); **Charter B** — the 7.5-D identity-wiring design (the registry/identity model, hotplug lifecycle, multi-interface `select_source`, the recommended I-15 file-cap interface-identity shape reusing the E1–E3 grant lifecycle, kill-switch off ≡ 7.5-C2, no new invariant, and the three-vector I-14 analysis). **Path:** `Plans/THINK-2026-06-23-phase-7.5-closeout-smp-and-identity-oracle.md`. Host-only; no QEMU gate opened.
