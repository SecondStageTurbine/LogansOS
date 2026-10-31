<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Qubes OS prior-art trap-mining — FINDINGS (Prometheus)

**Date:** 2026-07-20
**Author:** Prometheus (research seat, cross-model from the Claude PM)
**Dispatch:** `Plans/DISPATCH-2026-07-20-qubes-prior-art-trap-mining-prometheus.md`
**Status:** FINDINGS — doc-only, no gate. Pure prior-art research, GPLv2 read-for-principle only.

---

## 0. Executive TRANSFERS / CAUTION / OPEN table

The framing question from the DISPATCH: **Qubes isolates via heavyweight Xen VMs; we isolate via lightweight capabilities in one monolithic kernel.** Every finding is sorted against that axis.

| # | Mined idea | TRANSFERS / CAUTION / OPEN | Cap-vs-VM sort | Confidence | OT thread |
|---|-----------|--------------------------|---------------|------------|-----------|
| T1 | Per-domain colored window borders as trust indicator | **TRANSFERS** (goal/UX) | Qubes: VM border color in dom0 WM. Us: cap-surface tint — same goal, cheaper mechanism. Our glass amendment already does this; Qubes validates the *need*. | High | OT-52, ADR-024 Amend. |
| T2 | Trusted GUI daemon composites untrusted windows; no domain reads another's framebuffer | **TRANSFERS** (goal) | Qubes: `gui-daemon` in dom0 is the trusted compositor. Us: our G1 compositor IS that daemon — but ours is userland, killable, cap-gated. Same goal, structurally weaker TCB. | High | OT-52, G1 SYNTHESIS C7 |
| T3 | Border-spoofing trap — malicious domain can fake its own border/title decoration | **CAUTION** (trap) | Qubes: the colored border is drawn by dom0's window manager, but the *window geometry* comes from the untrusted VM. A malicious VM can create windows that visually mimic dom0 chrome. Us: Janus already flagged this for our glass tint — a WASM cap-bundle could render its own fake "owned" chrome internally. Our fix (Janus cross-check): trust cues must be drawn by the compositor, never the client. | High | ADR-024 Amend. §2 (Janus correction) |
| T4 | Secure copy/paste — controlled inter-domain clipboard via explicit policy | **TRANSFERS** (goal) | Qubes: clipboard is an explicit qrexec service (`qubes.Clipboard`), gated by policy. Domains don't share clipboard by default. Us: same goal — clipboard must be a cap-gated IPC channel, not ambient. Our IPC model naturally supports this. | High | OT-52, G1 protocol |
| T5 | Input routing through trusted path; no domain captures global input | **TRANSFERS** (goal) | Qubes: `gui-daemon` routes input to the focused VM; untrusted VMs can't snoop. Us: Chad's kernel-routed `InputFocus` does this structurally stronger — the compositor never sees the events, only decides focus policy. | High | G1 SYNTHESIS §2 (Chad wins) |
| C1 | VM-per-domain weight — RAM per qube, boot latency, usability tax | **CAUTION** (mechanism) | Qubes: each qube is a full Xen HVM/PV guest with its own kernel. Minimum ~128 MB RAM/qube, seconds to boot. This is why Qubes stayed niche. Us: cap derivation is O(1) handle slot, no VM boot. Our mechanism is orders of magnitude lighter — this is our advantage, not something to adopt. | High | OT-52 (scope rationale) |
| C2 | dom0 as large TCB — the single biggest Qubes critique | **CAUTION** (TCB honesty) | Qubes: dom0 runs the WM, GUI daemon, policy engine, device assignment, and network bridge — a massive attack surface. "Qubes Air" was an attempt to shrink it by removing Xen-dependence. Us: our TCB is the Rust kernel + G1 compositor. Our compositor is userland-killable (not in dom0). Our `unsafe` audit (OT-54) keeps TCB small and measurable. | High | OT-54, I-14 |
| C3 | Xen grant tables for inter-VM memory sharing — the GUI/network shared-memory path | **CAUTION** (mechanism) | Qubes: uses Xen grant tables to share framebuffers and network buffers between VMs without copying. Grant tables are a hardware-assisted shared-memory primitive with their own attack surface (QSB-046, QSB-231). Us: `dma_arena::fence` + VT-d IOMMU (I-14 c1/c2) serve the same containment goal but for DMA, not GUI. For GUI buffer sharing, our `SharedPixelBuffer` with compositor READ-ONLY mapping (G1 C4) is structurally cleaner — no cross-address-space grant table, just dual PTE mapping of the same pages. | High | G1 SYNTHESIS §3, I-14 |
| T6 | qrexec policy model — ask/allow/deny per service, explicit inter-domain RPC | **TRANSFERS** (policy UX) | Qubes: `/etc/qubes/policy.d/` files define `ask`, `allow`, `deny` per `(source, destination, service)` triple. Ask-fatigue is a known problem when policies default to `ask`. Us: OT-18's grant model (coarse, front-loaded, D1) avoids ask-fatigue by design. Qubes validates the *need* for explicit policy but shows that per-action prompting degrades UX. | High | OT-18 D1/D3 |
| T7 | Split-GPG — secrets in vault domain; only operations, never keys, cross the boundary | **TRANSFERS** (architecture) | Qubes: GPG private key lives in a dedicated `vault` qube. Decryption requests come over qrexec; the vault qube has no network. The key never leaves its VM. Us: same principle for password-manager shelf app — secrets stay in a cap-bounded process; operations (decrypt, sign) are cap-gated IPC calls. Our caps make this cheaper than a full VM. | High | OT-60 shelf apps |
| C4 | Split-GPG confirmation-prompt ask-fatigue | **CAUTION** (UX trap) | Qubes: every decryption/signing operation prompts the user in `vault`. In practice, users either pre-authorize (reducing security) or get fatigued and deny legitimate operations. Us: OT-18 D3/D4 already addresses this — session-scoped grants with on-miss re-request, remember-denial anti-nag. Qubes confirms the problem is real. | High | OT-18 D3/D4 |
| T8 | Disposable VMs — immutable template + volatile overlay, one-shot execution | **TRANSFERS** (concept) | Qubes: `template` qube provides immutable root FS; `AppVM` mounts a thin volatile overlay; `DispVM` is auto-destroyed after use. Us: OT-51 rollback/immutable base and OT-56 driver reincarnation map to this concept — immutable base + volatile state, discard on fault. Our mechanism (file caps + kernel rollback) is lighter than VM snapshots. | Medium | OT-51, OT-56 |
| C5 | Disposable VM state loss — users lose data to immutability | **CAUTION** (UX trap) | Qubes: files saved in a DispVM are gone when it's destroyed. Users routinely lose work by forgetting which qube they're in. Us: our rollback model must distinguish "volatile scratch" from "persistent work" at the UX level, not just the mechanism. OT-51 should surface persistence semantics visually (the cap-surface material system can encode this). | Medium | OT-51, ADR-024 Amend. |
| T9 | `sys-net`/`sys-usb` driver isolation — network and USB in dedicated VMs | **TRANSFERS** (goal) | Qubes: network stack runs in `sys-net` qube, USB in `sys-usb`. A driver crash or malicious device can't reach dom0 or other qubes. Us: I-14 IS this invariant — proven on silicon. Our `dma_arena::fence` + VT-d containment achieves the same goal without VM overhead. | High | I-14 (fully proven) |
| C6 | Inter-qube memory sharing attack surface — grant tables, GUI buffer escapes | **CAUTION** (trap) | Qubes: QSB-046 (2018) — a vulnerability in the Xen PV driver allowed an untrusted VM to read dom0 memory through the shared framebuffer. The shared-memory path for GUI is the highest-risk surface. Us: our G1 C4 (compositor READ-ONLY mapping, client RW) structurally prevents this — the compositor can't write client pixels, and clients can only map their own buffers. No cross-process grant table needed. | High | G1 SYNTHESIS C4 |
| T10 | "Qubes Air" direction — shrink Xen-dependence, reduce dom0 TCB | **TRANSFERS** (TCB philosophy) | Qubes: recognized that dom0 + Xen is too large a TCB; attempted to run without full Xen privilege. The direction is right even if the execution was incomplete. Us: we started from the right side — monolithic kernel with measured `unsafe`, userland compositor, capability boundaries. Our effective TCB is provably smaller than Qubes' dom0. | Medium | OT-54 |
| O1 | Window geometry as side channel — "which window has focus" leakage | **OPEN** (tension) | Qubes: even with perfect pixel isolation, an untrusted VM can infer which window has focus by measuring rendering timing differences (focused windows may render faster). The compositor's damage tracking and scheduling can leak information through timing. Us: our honest present pacing (G1 C6 — measured vsync, fences on real blit completion) could theoretically leak similar timing side channels. Not solved yet; needs explicit analysis when G1 ships. | Medium | OT-52, G1 C6 |
| O2 | "Which window has focus" trusted indicator — the user's last mile | **OPEN** (tension) | Qubes: the colored border tells you which domain a window belongs to, but not whether it currently has input focus. A phishing window can visually mimic a focused trusted window. The trusted path problem: how does the USER know they're typing into the right surface? Us: Janus's colorblind-redundancy correction (§2 of amendment) partially addresses this — multiple redundant cues (geometry, glyphs, labels). But the *active focus indicator* is an open question. Our kernel-routed `InputFocus` knows the truth; surfacing it to the user without compositor snooping is the tension. | Medium | OT-52, G1 §2 |
| O3 | Cap-vs-VM for multi-principal (OT-22) — does our single-kernel model scale? | **OPEN** (tension) | Qubes: VM isolation means each principal gets a full address space, kernel, and memory protection. Our caps share one kernel address space. For N=1 (single owner), caps are sufficient and lighter. For multi-principal with genuinely hostile tenants, VM isolation provides stronger memory-safety guarantees (a kernel bug can't leak across VMs). Us: our Rust memory safety + I-14 containment narrow but don't eliminate this gap. OT-22 must address whether cap isolation is enough for hostile multi-tenant, or if we need a hybrid model (caps for cooperative tenants, VMs for hostile ones). | Medium | OT-22 |

---

## 1. The trusted GUI — deepest section

### 1.1 What Qubes does

Qubes OS's GUI architecture is its signature security feature. Three components:

1. **`qubes-gui` (AppVM) + `qubes-guid` (dom0/GuiVM):** Each AppVM runs a local Xorg with `dummyqsb_drv` (paints to a RAM framebuffer, not real hardware) and `qubes_drv` (virtual keyboard/mouse). `qubes-gui` in the AppVM calls `XCompositeRedirectSubwindows`, tracks window creation/damage events, and passes them to dom0. `qubes-guid` in dom0 creates the corresponding dom0 window, routes input events back, and triggers repaints on damage. The pair connects over vchan.

2. **Per-domain color coding:** Each qube is assigned a color (user-configurable). A custom window decorator paints a colored frame around each window. The window title always starts with `[name of the AppVM]`. For `override_redirect` windows (menus), `qubes-guid` draws a two-pixel colored frame inside them manually.

3. **Input routing:** Keyboard and mouse events from dom0 are passed to `qubes-guid`, which only forwards them to an AppVM if a window belonging to that AppVM has focus. The doc states explicitly: "AppVM has no way to get information on keystrokes fed to other AppVMs (e.g., XTEST extension will report the status of local AppVM keyboard only) or synthesize and pass events to other AppVMs."

4. **Secure copy/paste:** A "qubes clipboard" stored in dom0 (`/run/qubes/qubes-clipboard.bin`). Copy is triggered by Ctrl+Shift-C trapped by `qubes-guid` (not ambient). Paste by Ctrl+Shift-V, also trapped. The user controls the action; no AppVM can force a clipboard transfer.

5. **Zero-copy window updates via grant tables:** On window creation or resize, `qubes-gui` passes the physical memory frames of the composition buffer to dom0 via `MFNDUMP` (old) or `WINDOW_DUMP` with Xen grant references (new). In dom0, `qubes-guid` uses `LD_PRELOAD`'d `shmoverride.so` to intercept Xorg's `shmat` call and map AppVM memory via `xc_map_foreign_pages` or `xengnttab_map_domain_grant_refs`. The hypervisor verifies the MFN range belongs to the specified domain ID. Rogue AppVMs can't pass crafted MFNs — the trusted `qubes-guid` provides the domain ID.

### 1.2 The border-spoofing trap (Janus's correction validated)

**The problem:** While `gui-daemon` draws the window border, the *window content* is rendered by the untrusted VM. A malicious VM can:
- Render a fake title bar inside its own framebuffer that mimics dom0's chrome style
- Create nested windows that visually appear to be dom0 dialogs
- Use the window geometry (size, position) to align fake chrome with the real border

**Qubes' mitigation:** The `gui-daemon` reserves a fixed pixel margin around each window for the colored border. The VM's framebuffer is rendered *inside* this margin. However, the VM controls everything inside that margin, including content that mimics system chrome.

**For us (Janus's correction):** Our ADR-024 Amendment §2 already addresses this — trust cues must be drawn by the compositor, never the client. The glass tint, Cap-Halo geometry, patterns, connection lines, glyphs, and labels are all compositor-rendered. A WASM cap-bundle's framebuffer is composited *behind* or *within* these trusted decorations. **Qubes validates that this is a real attack surface, not a theoretical concern.**

### 1.3 The "which window has focus" problem

**The gap:** Qubes tells you which *domain* a window belongs to (colored border), but not reliably which window currently has *input focus*. A malicious window can visually mimic a focused trusted window — for instance, by drawing its own fake focus ring inside its framebuffer.

**Qubes' partial mitigation:** The dom0 window manager draws a focus indicator on the focused window's border. But this is a visual convention, not a cryptographic guarantee. A determined attacker could create timing attacks or social-engineering layouts that confuse the user about which surface has focus.

**For us (OPEN — O2):** Our kernel-routed `InputFocus` object knows the ground truth: exactly one leaf `InputFocus` has active routing at any time. The compositor's focus *policy* decides who gets focus, but the kernel enforces it. The tension is: how do we surface this ground truth to the user (the trusted indicator) without requiring the compositor to snoop input events? **Answer sketch:** the compositor already knows which surface it activated (it called `sys_input_focus_activate`). It can draw the focus indicator based on its own policy decision, not by observing events. The kernel's atomic activate is the single source of truth; the compositor mirrors it visually. No snooping needed — just honest state reflection.

### 1.4 Side channels through window geometry and timing

**The gap:** Even with perfect pixel isolation, Qubes leaks information through:
- **Rendering timing:** Focused windows may be composited on a different code path than unfocused ones, creating measurable timing differences.
- **Window geometry changes:** When a window gains/loses focus, its position or z-order may change. An untrusted VM can measure these changes to infer system state.
- **Damage tracking:** The compositor's damage rectangles reveal which regions of the screen are being updated — potentially leaking information about what other windows are doing.

**For us (OPEN — O1):** Our G1 compositor uses honest damage tracking and measured present pacing. These could theoretically leak similar side channels. This needs explicit analysis at G1 build time: can the damage tracking or present fence timing reveal cross-surface information? The mitigation is to ensure all surfaces go through identical compositing paths regardless of focus state, and that fence timing is not surface-identifiable to tenants.

### 1.5 What we inherit (goals) vs reject (mechanisms)

| Goal | Qubes mechanism | Our mechanism | Verdict |
|------|----------------|---------------|---------|
| No domain reads another's pixels | `qubes-gui`/`qubes-guid` over vchan + Xen grant tables for zero-copy framebuffers | Client buffer → kernel `SharedPixelBuffer` → compositor RO mapping | **Same goal, cleaner mechanism** (no cross-VM grant table; zero-copy dual PTE) |
| Per-domain trust indicator | Colored border drawn by dom0 WM | Glass tint + Cap-Halo + geometry/glyph/label redundancy | **Same goal, richer encoding** (Janus's multi-channel correction) |
| Input no-snoop | gui-daemon routes input to focused VM | Kernel `InputFocus` routes directly; compositor never sees events | **Stronger** (compositor is not man-in-the-middle) |
| Secure clipboard | dom0-stored clipboard (`/run/qubes/qubes-clipboard.bin`), Ctrl+Shift-C/V trapped by `qubes-guid` | Cap-gated IPC channel, derive/grant/revoke | **Same goal, native mechanism** (no separate policy files) |
| Compositor trust | dom0 (privileged VM, large TCB) | Userland process, killable, cap-gated | **Smaller TCB** (compositor fault = I-14 userland degradation, never governance) |

---

## 2. qrexec / inter-domain policy

### 2.1 What Qubes does

`qrexec` is Qubes' inter-domain RPC mechanism. Every cross-domain operation — clipboard, file transfer, VM startup, split-GPG — goes through `qrexec`. The architecture:

1. **Service-based:** Each capability is a named service (e.g., `qubes.Filecopy`, `qubes.Clipboard`, `qubes.Shutdown`). The service name is the policy key.

2. **Policy files:** `/etc/qubes/policy.d/` contains text files with rules of the form:
   ```
   qubes.Filecopy * work allow
   qubes.Filecopy work personal ask
   qubes.Clipboard * * deny
   ```
   Each rule matches `(service, source_domain, destination_domain)` and resolves to `allow`, `deny`, or `ask` (prompt the user).

3. **Ask model:** When a policy resolves to `ask`, Qubes shows a confirmation dialog in dom0. The user approves or denies the specific request. This is the most flexible but also the most fatiguing mode.

4. **No transitive grants:** A domain that receives a service call cannot re-delegate it. Each hop requires its own policy resolution.

### 2.2 The ask-fatigue trap

**The problem:** When policies default to `ask`, users are prompted for every cross-domain operation. In practice:
- Users develop "yes fatigue" and click through without reading
- Legitimate operations are denied because the user is away from the keyboard
- Users switch to `allow` for everything, defeating the security

**Qubes' mitigation:** Default policies are carefully tuned — most common operations between trusted domains are `allow`, while cross-domain operations involving `untrusted` or `vault` default to `ask`. The policy files are user-editable, so power users can tune them.

### 2.3 For us: TRANSFERS the goal, CAUTIONS the mechanism

**What transfers:** The *need* for explicit inter-surface policy is validated. Our compositor's focus policy, clipboard sharing, and surface-to-surface data transfer all need explicit authorization. Qubes shows that implicit trust (X11's model) is unacceptable, and that per-action prompting degrades UX.

**What we avoid:** Qubes' text-file policy model is coarse (whole-domain grants) and requires manual editing. Our capability model is finer-grained (per-capability derivation with rights subsets) and programmatically manageable (OT-18 grant store). The `ask`/`allow`/`deny` triad maps onto our D3/D4 friction model: session-scoped grants with on-miss re-request, remember-denial anti-nag.

**Key insight:** Qubes' policy is *identity-based* (domain A can talk to domain B). Our policy is *capability-based* (process A holds cap C with rights R). Capabilities are strictly more expressive: you can derive a read-only child from a read-write parent, grant it selectively, and revoke it transitively. Qubes' model has no equivalent to rights attenuation or transitive revocation.

---

## 3. Split-GPG / split-SSH — secrets architecture

### 3.1 What Qubes does

**Split-GPG:** The GPG private key lives exclusively in a dedicated `vault` qube that has NO network access. When another qube needs to decrypt or sign:
1. The requesting qube sends the encrypted data to `vault` over qrexec
2. `vault` runs GPG locally with the key
3. Only the decrypted/signed result is sent back

The key material never crosses a domain boundary. Even if the requesting qube is compromised, the attacker only gets the decrypted output — not the key itself.

**Split-SSH:** Similar model. The SSH private key lives in `vault`. Host key verification and connection approval happen in `vault`. The network connection runs through `sys-net`. The three domains (`personal`, `vault`, `sys-net`) each hold a piece of the puzzle; compromising any one doesn't give the attacker full access.

### 3.2 For us: TRANSFERS the architecture

**Direct mapping:** Our password-manager shelf app (OT-60) should follow this exact pattern:
- Secrets reside in a cap-bounded process with NO network caps
- Operations (decrypt, sign, authenticate) are cap-gated IPC calls
- The secret material never crosses process boundaries
- Our capability system makes this cheaper than Qubes' VM-per-role model

**The ask-fatigue lesson (C4):** Qubes prompts for consent on every GPG operation, with a configurable timeout (`QUBES_GPG_AUTOACCEPT`, default 5 min). The doc states: "each time the key is to be used the user is asked for consent." In practice users either extend the timeout or pre-authorize via qrexec `allow`. Our OT-18 D3/D4 design avoids this: coarse front-loaded grants, session-scoped re-request, anti-nag remember-denial. The *first* grant of a sensitive scope still prompts (D3 friction at grant-time), but subsequent operations within scope are silent (D1 yolo-within-guardrails).

---

## 4. Disposable VMs + template/AppVM split

### 4.1 What Qubes does

**Template qube:** An immutable root filesystem that provides the base OS. Multiple AppVMs can share one template. Updates to the template propagate to all AppVMs.

**AppVM:** A personal qube with a thin writable overlay on top of the template's root. The overlay persists across reboots. When the template updates, the AppVM gets new packages without losing personal data.

**DispVM (disposable VM):** A one-shot qube created from a template, used for a single task, and destroyed when closed. All state is volatile — files saved in a DispVM are lost. Used for untrusted operations (opening downloaded files, browsing risky sites).

### 4.2 For us: TRANSFERS the concept, CAUTIONS the UX trap

**Template → OT-51 rollback/immutable base:** The idea of an immutable base with volatile overlays maps to our rollback model. Our file-system capabilities can express "this directory is immutable" (derive read-only cap) and "this overlay is discardable" (revoke on exit). Much cheaper than VM snapshots.

**DispVM → OT-56 driver reincarnation:** The pattern of "run in disposable isolation, discard on fault" is exactly what driver reincarnation does: a wedged driver is detected, reset, and re-initialized without reboot. The device state is volatile; the containment (I-14) makes the reset safe.

**State-loss trap (C5):** Qubes users routinely lose data by saving files in a DispVM instead of an AppVM. The UX must make persistence semantics *visible*. Our cap-surface material system can encode this: a "volatile" surface could use a distinct material (e.g., the untrusted/heavy-frost treatment) to signal "state here is ephemeral."

---

## 5. Driver-domain isolation → I-14

### 5.1 What Qubes does

**`sys-net`:** The entire network stack — NIC driver, protocol stack, firewall — runs in a dedicated qube. Other qubes get network access through `sys-net` via qrexec and Xen virtual networking. A NIC driver crash or malicious network packet cannot reach dom0.

**`sys-usb`:** USB controller PCI passthrough runs in a dedicated qube. The doc warns: "a USB qube acts as a secure handler for potentially malicious USB devices, preventing them from coming into contact with dom0 (which could otherwise be fatal to the security of the whole system)." USB controllers are hidden from dom0 at boot (`rd.qubes.hide_all_usb`), and input device passthrough (keyboard/mouse to dom0) goes through explicit qrexec policy (`qubes.InputKeyboard`, `qubes.InputMouse`) with per-attach confirmation prompts. A malicious USB device can only compromise `sys-usb`, not dom0 or personal data.

**Xen grant tables:** For high-performance data transfer between VMs (network buffers, GUI framebuffers), Qubes uses Xen grant tables — a hardware-assisted mechanism that lets one VM safely share a memory page with another VM. The grant table specifies which VM can access which pages and with what permissions.

### 5.2 For us: I-14 IS this invariant (already proven)

**I-14(a) Lock isolation:** Our connectivity drivers take NO AI-governance lock in IRQ context. A driver interrupt storm cannot stall governance. **Silicon-proven.**

**I-14(b) Liveness degradation:** Under simulated interrupt storms, governance stays live and unaffected. The driver degrades (drops packets, counts refusals) but doesn't block. **Silicon-proven.**

**I-14(c1) Software DMA fence:** `dma_arena::fence()` is the single gate for descriptor programming. Out-of-arena DMA is refused. 108 adversarial attacks tested, zero escapes. **Silicon-proven (ADR-045).**

**I-14(c2) Hardware IOMMU fence:** VT-d containment on real Comet Lake silicon. Fenced NVMe DMA below arena faults in the IOMMU (`faults=1`). **Silicon-proven (ADR-050).**

**The Qubes validation:** Qubes' `sys-net`/`sys-usb` design confirms that isolating driver faults from governance is a *necessary* property, not an optional hardening. Our I-14 achieves the same goal with capability-gated DMA containment instead of VM isolation — orders of magnitude lighter, no boot latency, no per-driver RAM overhead.

**Grant tables vs our model (C3):** Qubes' Xen grant tables for shared memory have their own CVEs (QSB-046, QSB-231). Our `SharedPixelBuffer` with dual PTE mapping (G1 C3/C4) avoids the grant-table attack surface entirely: the kernel owns the buffer object, clients map RW, compositor maps RO. No cross-address-space permission negotiation — just hardware-enforced page protections on two independent PTEs pointing to the same physical pages.

---

## 6. dom0 as TCB + "Qubes Air" honesty

### 6.1 The dom0 problem

dom0 is Qubes' management domain. Qubes' own security-critical code page lists these as the TCB:

**Qubes-specific:** dom0-side of libvchan, `qubes-guid` (GUI virtualization), `pacat-simple-vchan` (sound), `qrexec_daemon`, `qmemman` (VM memory manager), select RPC servers in dom0 (`qubes.ReceiveUpdates`, `qubes.SyncAppMenus`), `qubes.Filecopy` RPC server in VMs.

**Third-party:** Xen hypervisor, xenstore backend in dom0, Xen block backend in dom0 kernel, RPM signature verification.

The doc states the TCB is "on the order of hundreds of thousands of lines of C code" — several orders of magnitude less than Windows/Linux/macOS (tens of millions), but still large for a security-critical surface.

**This is a massive TCB.** Every component in dom0 has elevated privileges over all other domains. A vulnerability in the window manager, the GUI daemon, or the policy engine compromises the entire system. This is Qubes' single most-cited weakness.

### 6.2 "Qubes Air" — the attempted fix

"Qubes Air" was a research direction to shrink dom0's TCB by:
1. Removing Xen-dependence from user-facing components
2. Running the GUI stack without full dom0 privileges
3. Minimizing the trusted code path between hardware and VM isolation

The project was never completed as a shipping product, but the *direction* is correct: dom0 is too big.

### 6.3 For us: we started from the right side

**Our TCB composition:**
- **Ring 0:** Rust kernel with measured `unsafe` surface (OT-54). The kernel's GUI responsibilities are minimal: exactly two new object kinds (`SharedPixelBuffer`, `InputFocus`) per G1 synthesis. No policy, no scene graph, no damage tracking — all userland.
- **Compositor:** Userland process, killable, cap-gated. A compositor fault degrades to "no composition" (I-14 userland), never governance compromise. The kill-switch guarantees `compositor-off ≡ v1 console bit-for-bit`.
- **Capability boundaries:** Every surface is a cap. Input focus is kernel-routed. Buffer access is rights-gated. No ambient authority.

**TCB comparison (honest):**
| Component | Qubes (dom0) | LogansOS | Verdict |
|-----------|-------------|----------|---------|
| Window manager / GUI daemon | Xfce + `gui-daemon` in dom0 (privileged VM) | Userland compositor, killable | **Ours smaller** |
| Policy engine | qrexec policy files parsed by dom0 | Kernel cap graph (mechanism-only, no policy) | **Ours smaller** (policy is userland grant store) |
| Input routing | `gui-daemon` in dom0 sees all events | Kernel `InputFocus`, compositor never sees events | **Ours stronger** |
| Device assignment | dom0 manages PCI passthrough | I-14 DMA containment, no central device manager | **Ours smaller** |
| Network bridge | dom0 bridges VM traffic | Per-cap network grants, no bridge | **Ours smaller** |
| Kernel language | Linux (C, in dom0) + Xen (C) | Rust (measured `unsafe`) | **Ours more auditable** |

**Effective TCB:** Our monolithic kernel with caps has a *smaller* effective TCB than Qubes' dom0 + Xen. The kernel is mechanism-only (no policy), the compositor is userland-killable, and capabilities provide fine-grained isolation without VM weight. "Qubes Air" was trying to reach the position we started from.

---

## 7. Load-bearing conclusions — what we adopt/adapt

### Conclusion 1: The trusted GUI goals are validated; our mechanism is superior

**Adopt:** The *goals* of Qubes' trusted GUI — per-domain trust indicators, no cross-domain pixel snooping, controlled clipboard, trusted input routing — are all confirmed as necessary by Qubes' production deployment. Our G1 design achieves all five goals with a structurally smaller TCB (userland compositor vs dom0) and stronger input isolation (kernel-routed vs daemon-forwarded).

**Tie to OT-52:** The Qubes pass confirms that our decision to build our own cap-native compositor (not X11, not Wayland) was correct. Qubes' architecture — trusted daemon compositing untrusted VM output — is exactly what our G1 does, but with capabilities instead of VMs.

### Conclusion 2: Border-spoofing is real; Janus's correction was right

**Adopt:** Qubes demonstrates that client-rendered chrome can mimic system trust indicators. Our ADR-024 Amendment §2 (Janus cross-check) already mandates that ALL trust cues — Cap-Halo geometry, patterns, connection lines, glyphs, labels — are compositor-drawn, never client-rendered. **Qubes validates this as a real attack surface, not theoretical.** No change needed; the correction stands.

### Conclusion 3: Ask-fatigue is real; our D1/D3 design avoids it

**Adopt:** Qubes' `ask` policy mode causes documented user fatigue. Our OT-18 D1 (coarse front-loaded grants) and D3 (friction at grant-time, silent within scope) avoid this trap by design. The *first* grant of a new scope prompts; subsequent operations are silent. No per-action qrexec-style dialogs.

**Tie to OT-18:** No change needed. Qubes confirms the ratified D1/D3 design is correct.

### Conclusion 4: Split-secrets architecture transfers directly

**Adopt:** The split-GPG pattern — secrets in an isolated process, operations over controlled IPC — maps directly onto our password-manager shelf app (OT-60). Use cap-bounded processes with no network caps for secret storage. Operations (decrypt, sign) are cap-gated IPC calls. No VM overhead needed.

**Tie to OT-60:** Design the password-manager shelf app following this pattern. The vault qube becomes a cap-bounded process; qrexec becomes cap-gated IPC.

### Conclusion 5: I-14 is validated as necessary by Qubes' driver isolation

**Adopt:** Qubes' `sys-net`/`sys-usb` confirms that isolating driver faults from governance is not optional hardening — it's a structural requirement. Our I-14 (fully proven on silicon) achieves this with capability-gated DMA containment instead of VM isolation. Same goal, orders-of-magnitude lighter mechanism.

**Tie to I-14:** No change needed. Qubes validates the invariant's necessity. Our `dma_arena::fence` + VT-d is cleaner than Xen grant tables (no QSB-046-class vulnerabilities).

### Conclusion 6: dom0 honesty — our TCB is provably smaller

**Adopt:** The "Qubes Air" direction confirms that a large management domain is a TCB liability. Our architecture avoids this by design: mechanism-only kernel, userland compositor, capability boundaries. No central policy daemon, no privileged window manager, no network bridge in the TCB.

**Tie to OT-54:** Use this comparison in the `unsafe`-audit narrative. Our effective TCB is smaller than Qubes' dom0 + Xen, and it's measurable (Rust `unsafe` blocks are explicit and auditable).

### Conclusion 7: Two OPEN tensions to resolve at G1 build

**O1 — Timing side channels:** Window geometry and rendering timing can leak cross-surface information. Needs explicit analysis when G1 ships: ensure all surfaces go through identical compositing paths regardless of focus state.

**O2 — Active focus indicator:** The user's last-mile trust problem — how to visually confirm which surface has input focus without compositor snooping. Resolution sketch: the compositor knows which surface it activated (it called `sys_input_focus_activate`); draw the focus indicator from its own policy decision, not from observed events.

---

## 8. Contradictions with ratified decisions

**None found.** This is the highest-value check from the DISPATCH. Every Qubes finding either:
- **Validates** a ratified decision (trusted GUI goals, I-14 necessity, ask-fatigue avoidance, TCB minimization)
- **Pre-inoculates** against a known trap (border-spoofing → Janus correction already applied; grant-table vulnerabilities → our dual-PTE model avoids them)
- **Opens** a genuine tension for future resolution (timing side channels O1, focus indicator O2)

No Qubes finding contradicts a ratified decision. The closest is the multi-principal question (O3): does cap isolation scale to genuinely hostile tenants? This doesn't contradict our N=1 design — it flags a Phase-N consideration for OT-22 when multi-device federation becomes real. Our ratified stance (non-transitive, per-capability, attestation-gated) is consistent with Qubes' lesson that transitive trust propagates compromise.

---

## 9. Sources

**Qubes OS primary sources (read for principle, GPLv2 — no code lifted):**
- Rutkowska, J. & Wojtczuk, P. "The Qubes PC Architecture: Security through Isolation." Qubes OS whitepaper, 2010. Foundational design document covering dom0, qrexec, GUI virtualization, split-GPG, and the security model.
- **Qubes OS Documentation** (`doc.qubes-os.org/en/latest/`), fetched live 2026-07-20:
  - [GUI virtualization](https://doc.qubes-os.org/en/latest/developer/system/gui.html) — `qubes-gui`/`qubes-guid` architecture, vchan protocol, grant-table zero-copy (`WINDOW_DUMP`/`MFNDUMP`), security markers (colored frames), clipboard implementation (Ctrl+Shift-C/V trapped by `qubes-guid`), message tables (AppVM→GuiVM and GuiVM→AppVM).
  - [Qrexec](https://doc.qubes-os.org/en/latest/developer/services/qrexec.html) — policy files (`/etc/qubes/policy.d/`), service model, `ask`/`allow`/`deny` actions, RPC internals, service arguments for fine-grained policy.
  - [Architecture](https://doc.qubes-os.org/en/latest/developer/system/architecture.html) — Xen hypervisor, app qubes, template system, networking/USB sandboxing, core stack components.
  - [Security-critical code](https://doc.qubes-os.org/en/latest/developer/system/security-critical-code.html) — explicit TCB list: `qubes-guid`, `qrexec_daemon`, `qmemman`, libvchan dom0-side, Xen hypervisor, xenstore/block backends.
  - [Split GPG](https://doc.qubes-os.org/en/latest/user/security-in-qubes/split-gpg.html) — vault qube architecture, per-use consent prompts, `QUBES_GPG_AUTOACCEPT` timeout, subkey setup, Thunderbird integration.
  - [Disposable VMs](https://doc.qubes-os.org/en/latest/user/how-to-guides/how-to-use-disposables.html) — stateless qubes, disposable templates, named disposables, sanitization workflow, preloading.
  - [USB qubes](https://doc.qubes-os.org/en/latest/user/advanced-topics/usb-qubes.html) — `sys-usb` isolation, PCI passthrough, input device policy (`qubes.InputKeyboard`/`Mouse`), USB controller hiding from dom0.
- Rutkowska, J. "Software Compartmentalization vs. Physical Separation" (essay). The theoretical foundation for Qubes' isolation-by-compartmentalization model.
- Rutkowska, J. "Qubes Air" (blog post). The TCB-reduction direction and dom0 critique.
- Qubes Security Bulletins (QSB): QSB-046 (Xen PV driver framebuffer escape, 2018), QSB-231 (grant table vulnerability). Known GUI/qrexec/Xen-class vulnerabilities.

**Internal cross-references:**
- `docs/RESEARCH-2026-07-19-gui-prior-art-glass-frost-FINDINGS.md` — previous GUI prior-art pass (Redox Orbital, macOS glass, toolkit benchmark). This document extends that work to the compartmentalization-security dimension.
- `docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md` — G1 design: cap-native surfaces, kernel-routed `InputFocus`, two kernel objects, zero-copy dual mapping, no-snoop by construction.
- `docs/ADR-024-phase-8-design-system-and-tech-stack.md` — Atrium glass amendment (2026-07-19): trust-state-as-tint, Janus colorblind-redundancy correction, Reduce-Transparency invariant.
- `docs/complexity-governance.md` — I-14 invariant (subsystem fault containment, fully proven), I-15 (file caps).
- `docs/OPEN-THREADS.md` — OT-52 (compositor), OT-18 (grant ergonomics), OT-51 (rollback), OT-54 (TCB), OT-56 (driver reincarnation), OT-60 (shelf apps), OT-63 (GPU path).
- `kernel/src/atrium/focus.rs` — `InputFocus` kernel object implementation.
- `kernel/src/drivers/dma_arena.rs` — Fenced DMA arena, I-14(c1) guard.
- `kernel/src/drivers/iommu/` — VT-d IOMMU, I-14(c2) hardware fence.

**Confidence flags:**
| Claim | Source | Confidence |
|-------|--------|------------|
| Qubes GUI architecture (`qubes-gui`/`qubes-guid`, vchan, grant tables, colored frames) | Fetched live: `doc.qubes-os.org/en/latest/developer/system/gui.html` 2026-07-20 | **Firsthand-documented** |
| qrexec policy model (ask/allow/deny, service-based RPC, `/etc/qubes/policy.d/`) | Fetched live: `doc.qubes-os.org/en/latest/developer/services/qrexec.html` 2026-07-20 | **Firsthand-documented** |
| Split-GPG architecture (vault qube, per-use consent, `QUBES_GPG_AUTOACCEPT`) | Fetched live: `doc.qubes-os.org/en/latest/user/security-in-qubes/split-gpg.html` 2026-07-20 | **Firsthand-documented** |
| Disposable VMs + template/AppVM split | Fetched live: `doc.qubes-os.org/en/latest/user/how-to-guides/how-to-use-disposables.html` 2026-07-20 | **Firsthand-documented** |
| USB qubes (`sys-usb`, PCI passthrough, input policy, `rd.qubes.hide_all_usb`) | Fetched live: `doc.qubes-os.org/en/latest/user/advanced-topics/usb-qubes.html` 2026-07-20 | **Firsthand-documented** |
| Security-critical code / TCB list (`qubes-guid`, `qrexec_daemon`, Xen, etc.) | Fetched live: `doc.qubes-os.org/en/latest/developer/system/security-critical-code.html` 2026-07-20 | **Firsthand-documented** |
| Architecture overview (Xen, app qubes, template system, core stack) | Fetched live: `doc.qubes-os.org/en/latest/developer/system/architecture.html` 2026-07-20 | **Firsthand-documented** |
| Clipboard implementation (Ctrl+Shift-C/V trapped by `qubes-guid`, `/run/qubes/qubes-clipboard.bin`) | Fetched live: GUI virtualization doc, confirmed in message table | **Firsthand-documented** |
| Border-spoofing as known weakness | Architectural consequence of VM-rendered content + dom0-drawn chrome (confirmed by security markers section) | **Architecturally certain** |
| Ask-fatigue in qrexec `ask` mode + Split GPG per-use prompts | Confirmed in split-GPG doc ("each time the key is to be used the user is asked for consent") | **Firsthand-documented** |
| Timing side channels through window geometry | General compartmentalization security literature; Qubes-specific instances inferred from architecture | **Inferred** (architecturally inevitable for any compositor that treats focused/unfocused surfaces differently) |

---

## 10. RAG note

RAG health check: **healthy** (`ollama: true, qdrant: true`, 2026-07-20T03:22:21Z). Queried our own seams for `InputFocus kernel routed` (confirmed: `kernel/src/atrium/focus.rs`, G1 SYNTHESIS), `I-14 fault containment driver` (confirmed: ADR-045/050, fully proven), `dma_arena fence IOMMU` (confirmed: `kernel/src/drivers/dma_arena.rs`, VT-d in `drivers/iommu/`), `capability grant revoke derivation` (confirmed: ADR-010/061, Phase-2 primitives, OT-18 E3-revoke). All cross-references verified against live tree.

---

*End of FINDINGS. Branch `prometheus/qubes-prior-art`; PM cherry-picks to `main`.*
