<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# LogansOS RELEASE LEDGER — the five journeys, honestly audited (2026-07-29)

**From:** PM (Claude). **Status:** the ledger `HANDOVER-2026-07-25` §2 instructed the incoming PM to create and which did not exist on `main`. Audited against HEAD by independent repo sweep, not from memory.

## The finding in one line
**Two of the five 1.0 journeys are real engineering programs with code, ADRs, OT-numbers and named blockers. The other four have zero code, zero ADR, zero OT-number and no owner** — they exist only as five lines in one handover doc. That is not a failure; it is the map we have been missing.

## The audit

| # | Journey | Status | What exists on `main` | The gap |
|---|---|---|---|---|
| 1 | **Install** | ⛔ **Untracked, zero code** | Host-side only: `tools/neofs-mkfs/` (deterministic image), xtask `cmd_iso` → `boot/neoplutarch.iso`. Deployment today = **`dd` the ISO onto NVMe from a live USB**. | No on-device installer, partitioner, GPT/MBR writer, on-device mkfs, or user provisioning. Nearest plan is OT-51 (A/B *update* model, design-only). **OT-87 opened 2026-10-27** (THINK: PR #149, S3; cross-family read: relay #86). PL decisions 2026-10-27: one USB with **Try or Install** (like Ubuntu); Install offers **"Erase disk and install"** (typed confirmation, identity-bound one-shot claim, Muse review); **Try opens an installed LogansOS disk read-only**. OT-65 folds into OT-87. |
| 2 | **Login / session** | ⛔ **Untracked, zero code** | Nothing. `kernel/src/init/boot_frame.rs:110` is a TODO admitting the boot line stands in for a login prompt. (`vault-core` = a password *vault*, ADR-075 Phase A — not OS auth. `net/identity.rs` = NIC identity.) | Principal is `ProcessId`; **no human-user object**, credential store, lock screen, or session broker. `LaunchCap` exists only as future rung DS-1. **No OT-number.** |
| 3 | **Desktop** | 🟢 **Most advanced** | G1 compositor slices **1–5 merged**, glass GPU-P1 slices **1–6 merged**, `kernel/src/atrium/*`, HID keyboard+mouse, syscalls `0x1A0–0x1AD`, ADR-073/074/076/077. | One client, one transcript — no WM/shell/multi-window. **DW-1 dispatched 2026-07-29** (zero kernel delta). Then DW-2 → DS-1 → DI-1. |
| 4 | **Browse** | 🟡 **Real program, deep** | C0 + C0.5 + C1.0 merged (real `std` ELF runs on-target); full net stack merged (`tcp/tls/dns/dhcp`), `NetDialCap` merged (default-off). | **Blocked at C1.1** (threads+futex). Then C1.2 → C2 → C3 (SpiderMonkey no-JIT) → SWGL floor → modern-site battery. The deepest program we have. |
| 5 | **Write a report** | ⛔ **Untracked, zero app** | Persistence primitives only: `SYS_OPEN/READ/WRITE/FSYNC/READDIR`, `kernel/src/fs/neofs/`, ADR-061 file caps. Text is bitmap glyphs. | No editor, no text engine (cosmic-text unintegrated), no document format, no export. ADR-024 defers `neo-editor` to "Phase 8.5+". **No OT-number.** |
| 6 | **Print** | ⛔ **Nothing at all** | `printer\|spooler\|IPP\|CUPS\|usblp\|PDF\|postscript` across `kernel/`+`userland/`+`tools/` → **zero hits.** | No USB printer class driver, no raster/PDF pipeline, no spooler. **No OT-number, no ADR, no design.** |

## The asset the audit surfaced — we are already networked, without WiFi
`kernel/src/drivers/usb/cdc_ecm.rs` (844 L, **merged**, ADR-047 Accepted) — CDC-ECM **plus** a real AX88179/178A ASIX vendor bring-up — over the merged xHCI + DHCP + ARP + DNS + TCP + TLS stack. **Silicon-proven** (README §Status 2026-06-13: real DHCP lease `<optiplex-ip>` + DNS over a USB-Ethernet dongle on the OptiPlex, "zero new networking code"). ADR-017 §D3-C already ratifies **wired-network posture** as the supported path for users without WiFi.
**The only gap is product wiring:** `usb-host` is a **default-off** feature and USB-Ethernet lives behind *test scenarios* — there is no boot-time "bring up eth0, get a lease" product flow in `kernel/src/main.rs` / `kernel/src/init/`. **⇒ WiFi is a FEATURE, not a release blocker.** (Dispatched 2026-07-29 to Chad.)

## Recommendation — add a **0.1 "Arthur Preview"** milestone; do NOT redefine 1.0
The PL ratified (2026-07-12) that **1.0 is browser-gated** ("an OS that can't browse the internet isn't shippable"). That stands — this ledger does not touch it. But 1.0 is gated behind the *deepest* program we own (C1.1 → C1.2 → C2 → C3 → SWGL → battery) **plus four journeys with zero code**. Waiting for all of it means shipping nothing for a long time.

**0.1 "Arthur Preview" is reachable from merged assets** and is the thing no other OS can demo:
> Boot on real hardware → a real desktop (DW-1) → **talk to Arthur, our own LLM on our own kernel** → it is online (eth0) → your work persists (NeoFS).

Every piece is merged or in flight today: bare-metal boot ✅ (proven), Arthur answering on silicon ✅ (proven, `provenance=real_silicon`), compositor + glass ✅, desktop → DW-1 (dispatched), network → eth0 wiring (dispatched), persistence ✅. **No browser, no installer, no printer required** — 0.1 ships from a USB/ISO the way it boots today, and is honestly labelled a preview.

**1.0 remains the ratified five-journey bar.** 0.1 is a milestone on the way, not a redefinition.

## Outstanding PM actions (this ledger's own follow-ups)
1. **Open OT-numbers for the four untracked journeys** (install, login/session, document/editor, print) so they stop being invisible. Each needs a THINK before any build dispatch — printing especially (zero prior art in-tree).
2. **Sequence 0.1:** DW-1 (dispatched) + eth0-at-boot (dispatched) are the two open lanes; a 0.1 integration/showcase build closes it.
3. Keep the browser C-ladder as the **1.0** critical path (C1.1 is the live blocker) — it is not on 0.1's path.

## Update 2026-10-23: Zenbook S14 bring-up (bare-metal target since 2026-10-21)

The July audit above is left as written. This section records what the Zenbook lane has **proven on silicon**
versus what is **merged and awaiting its acceptance run** (CLAUDE.md, merge bar vs acceptance bar). A merge
claims nothing about silicon; only the first table does.

**Proven on the Zenbook (PL runs; photos are valid evidence on this target):**

| Milestone | Evidence |
|---|---|
| Image A boots to the desktop on silicon: x2APIC, the LAPIC TSC-deadline timer (#90/#91), RTC wall time read correctly (OT-85) | PL runs of Janus images `0294942e` (main 4e9ea28c) and later |
| USB keyboard and mouse work behind a hub on the south xHCI `00:14.0` | Janus test image `a4119c32` (USB test branch 6f09e826), PL run 2026-10-22 |
| The "ask Arthur" freeze is diagnosed by the watchdog overlay: after asking, `cpu0 ticks/s=0`, `model load-build weight 0/0`, Arthur inside syscall 0x110 for 29 s+ (the boot-model load ran at IF=0 and overran the 32 KiB thread stack) | Janus watchdog image `d9ad3424`, PL photos; `Plans/NOTE-2026-10-22-zenbook-ask-freeze.md`, `Plans/NOTE-2026-10-23-zenbook-model-load-freeze-s1.md` |

**Merged to `main`, acceptance run pending** (Janus relay #36 builds image A from `cb165c11` with all of these):

| PR | What | Acceptance check on the Zenbook |
|---|---|---|
| #93 (`5f5685b5`) | Zenbook USB input: the xHCI a keyboard/mouse is on, port service, the `usb-summary` block | USB keyboard/mouse on the desktop, from the merged image |
| #98 (`334f5ba4`) | i8042 PS/2 driver: the built-in keyboard (ATK3001, IRQ1 via the I/O APIC) | the built-in keyboard types |
| #95 (`829af5a8`) | Freeze watchdog overlay (product ISO only) | the yellow bottom-left block draws and its counters move |
| #99 (`cb165c11`) | Boot-model preload: the model is built once at boot on the guarded inference stack at IF=1; `sys_model_load` hands out a shared `Arc` | `BOOTMODEL preload … ok=true` at boot; Ask does not freeze the pointer; a second Arthur answers |

**Host-only, merged:** #96 (`ff25f384`), OT-86 ACPI/AML rung 0: corpus + grading harness; `acpi` 6.1.1 chosen as
the base (J.c., 2026-10-23).

**In flight (not merged):** #100 Zenbook touchpad rung 1 (I2C-HID, mouse mode); #101 RTL8125 2.5GbE rung 1 (main
PC); #102 OT-86 Fix 1 (L3 value-kind confusion; PL merge reserved).

Cross-family review RETURNs for these lanes are banked under `Plans/RETURN-2026-10-2{2,3}-*.md`.

## Update 2026-10-25: Zenbook acceptance, images A2–A5

**Proven on the Zenbook (PL runs and photos):**

| Milestone | Evidence |
|---|---|
| The built-in keyboard types; holding Backspace repeats (`key-repeat: rep=10 typematic=7`) | Image A2 (main `6ff86bc0`), PL photos 2026-10-24 |
| Arthur answers on silicon with the boot-model preload (`BOOTMODEL preload … ok=true`); the footer names the model; answers end without `<|im_end|>`; about 30 s per answer | Image A2, PL photos |
| The AVX2 matmul runs on every CPU: `ai: matmul avx2=Yes (census: 16/16 cpus reported)` | Images A2–A5 |
| The RTL8125 driver is inert where the chip is absent (`rtl8125: pci 10ec:8125 absent`), and the Ethernet summary prints | Image A3 (main `4d6dd94c`) |
| **The built-in touchpad works** (I2C-HID over the LPSS DesignWare controller `00:19.1 8086:e351`, IRQ GSI 105, PIO only). The fix came in three images: A3 found the controller with `bar0=0x0`; A4 (#113) showed firmware never assigned its 64-bit BAR0 (`nsr=1`, no soft reset); A5 (#114, `i2c-hid-bar-assign`) assigns BAR0 from a probed free slot, and reports parse (`reports=64 id=1 parsed=64 unexpected=0`) | Image A5 (main `5880e075`, relay #51), PL photos 2026-10-25: "It works!" |

**Still pending:** the main-PC (MSI MS-7E57) RTL8125 run: link, DHCP lease and gateway ARP with `dma=arena-only`.
Any of images A3–A5 carries it.

## Update 2026-10-25 (later): images A6 and A6-hwp on the Zenbook

| Milestone | Evidence |
|---|---|
| Keys pressed during the boot screens no longer break typing: they're dropped when the desktop takes the keyboard (#118) | A6-hwp, PL photo: `pre=78`, typing into Arthur worked at once; A6 (no keys pressed): `pre=0`, typing at once |
| The touchpad's service thread sleeps when idle (#115): `loops` tracks `reports` | A6: `tp: bound mode=irq loops=132 reports=130`; A6-hwp: `loops=835 reports=833` |
| **HWP is the dominant Arthur latency factor on the Zenbook.** With firmware's state (`hwp=off`, ratio 11), the generating CPU runs at 1071 MHz; with `hwp-performance` (`hwp=on`, ratio 35) it runs at 3287 MHz | A6: "What's 2+2?" took ≈57 s (spawn→model 34.2 s, prefill 642 ms/token × 25, decode 942 ms/token × 7). A6-hwp: ≈18 s (10.7 s, 208 ms × 25, 304 ms × 7). About 3.2× on every phase. PL stopwatch and overlay photos |

**Open after A6 (on silicon):**
- Spawn→model still takes 10.7 s at full clock (CPU-bound; S1).
- No frames are presented during generation (`presents=0`; S1).
- The generating CPU is still the slowest partition (`part us 92..216 (c0)`; S4).
- `hwp-performance` becomes default in image A (S2).

## Update 2026-10-26: image A7 on the Zenbook

Image A7: main `745dd314`, relay #63, ISO BLAKE3 `46b536b4…`. It has HWP by default (#124) and the build-std
opt-level fix (#126).

| Milestone | Evidence |
|---|---|
| **Arthur's start-up is about 5× faster.** Spawn→model: 10.7 s on A6-hwp, **1.96 s** on A7. | Overlay `start` line: `arthur=10790 vocab=10792..10792 model=10986` (100 ticks/s). The vocab read finishes inside one tick; the remaining ~1.9 s is from the vocab close to the model handle (the parse). PL: "about 2 seconds for the Arthur box". |
| HWP is on by default, and the generating CPU runs at full clock. | `cpu gen=c0 3301MHz`, `hwp=on ratio=35` |
| Per-token cost is unchanged from A6-hwp, as expected (A7 has no speed-lane code). | `pre n=26 ms/t=213`, `dec n=9 ms/t=319`. A6-hwp: 208 and 304. |
| The touchpad's service thread still sleeps when idle. | `tp: bound mode=irq loops=345 reports=343` |

**Open after A7 (on silicon):**
- **Generation compute is 8.4 s** (prefill 5,542 ms + decode 2,877 ms). The PL's stopwatch felt about 2 s to the answer.
  The next run times from Enter to the answer to settle which is right.
- `presents=0 gap=8419ms shown=-`: no frame was presented across the whole generation, even with #126's think fix.
  Either the UI is frozen for the full answer, or the counter is still wrong (S1).
- Spawn→model is still about 1.9 s of parse at 3.3 GHz. A prebuilt binary vocab would remove it (S1, candidate).
- Merged since A7, for A8:
  - #127: model provenance runs IF-on;
  - #128: lever 1, soft_f32 add/sub/mul on SSE;
  - #129: a test-only fix.
- Queued for A8: lever 1b (S4).
- PL decision 2026-10-26: **keep debug assertions on in image A**, and hoist the hot-path asserts out of the inner
  loops (S4). Turning them off is revisited once the bare-metal runs are clean.
- **PL decisions 2026-10-26 (evening):**
  - **(a):** image A's kernel is built with `-Zub-checks=no`. Every debug_assert! and the overflow checks stay
    on, and every CI/QEMU test kernel keeps UB checks (S4).
  - **Build the binary vocab** (NEOVOCB1, S1).
  - **A8 waits for all of it** (#130, #131, #132, ub-checks, binary vocab). Standing preference: keep ISOs to a
    minimum and put as many changes and checks into each one as possible.

## Update 2026-10-26 (later): image A8 on the Zenbook: "What's 2+2?" in under 2 s

Image A8: main `87730200`, relay #72, ISO BLAKE3 `0dae4e38…`. It has levers 1 and 1b, `-Zub-checks=no`, the binary
vocab and the review fix-forwards.

| Milestone | Evidence |
|---|---|
| **Enter → answer in ≤ 2 s** (PL stopwatch: "2 sec or less"; "almost too quick to time"). The overlay agrees: prefill 1,302 ms + decode 572 ms = 1.87 s. | A6: ≈57 s. A6-hwp: ≈18 s. A7: ≈2 s box + 8.4 s compute. **A8: ≈1.9 s.** |
| **Spawn→model 30 ms** (A7: 1.96 s; A6-hwp: 10.7 s). The binary vocab has no parse. | `start`: `arthur=5394 vocab=5395..5395 model=5397` |
| **Per token: prefill 50 ms (A7: 213, 4.3×), decode 63 ms (A7: 319, 5.1×).** These are levers 1 and 1b plus UB checks off. | `perf pre n=26 ms/t=50 \| dec n=9 ms/t=63` |
| **The UI is live during generation.** It isn't a freeze: 81 frames were presented, and the longest gap was 265 ms. | `presents=81 gap=265ms shown=+265ms` (A7: `presents=0`; S1's NOTE said "nothing asked the WM to draw", and the pointer moving confirms it) |
| HWP on; touchpad and boot keys as before. | `cpu gen=c0 3111MHz hwp=on`; `tp: loops=919 reports=917`; `pre=18` |

**Where the time goes now** (A8 `perf`): `mv=36% sw=32% at=17% bar=10% lm=2%`, generating partition `part us 50..87 (c0)`.

**Speed-lane go/no-go, by S4's pre-registered thresholds:**
- **Lever 1c** (read the KV cache in place): build if `at=` ≥ ~10%. A8 has **17%: build.**
- **Lever 2** (parallelize the per-element soft work): build if 1 − mv − bar ≥ ~35%. A8 has **54%: build.**
  SwiGLU alone is 32%.

**Next image (A9):** batches #136, #137, #138 and #139 (build id, test and CI fixes, Docker safe.directory, rebuild
script), plus levers 1c and 2 once they gate.

## Update 2026-10-27: image A9 on the Zenbook: levers 1c and 2, Inter, no boot holds

Image A9: main `149431a1`, relay #84, ISO BLAKE3 `7e990c2f…`. It has levers 1c and 2, Inter as the proportional voice,
the boot without evidence holds, and a real build id in the banner.

| Result | Evidence |
|---|---|
| **"What's 2+2?": too fast for the PL to time.** Compute 1.16 s (A8: 1.87 s). | `pre=897ms dec=265ms presents=58 gap=20ms` |
| **Per token: prefill 31–35 ms (A8: 50), decode 37–40 ms (A8: 63).** SwiGLU's share is down from 32% to 4%. | `perf pre n=25 ms/t=35 \| dec n=7 ms/t=37`; moon: `pre n=57 ms/t=31 \| dec n=255 ms/t=40` |
| **"Tell me about the moon": about 12 s** (PL: "about 12 seconds"). That's 57 prompt and 255 reply tokens. | `pre=1780ms dec=10466ms`; `mv=60% sw=4% at=16% bar=14% lm=7%` |
| **Open: the UI didn't present during the 12 s answer.** | `shown=- presents=0 gap=12247ms`, against 58 on the short answer and 81 in A8. Sent to S1 to diagnose (is lever 2 starving the WM, or is the reply shown only at the end?). |
| **Open: the reply loops.** Greedy decoding with no repetition control repeats one sentence up to the 256-token cap, then `[answer cut short]`. | Sent to S1: a deterministic repetition penalty and loop stop, behind a kill-switch. |
| **Open: Arthur's reply can't be scrolled.** Enlarging the window shows more, but not all of it. | Sent to S1: touchpad, wheel and key scrolling. |
| Inter renders cleanly in Arthur's surface. | PL photos |

**Where the time goes now** (moon answer): `mv=60%` (quantized matvec) is the next target. It's followed by
`at=16%`, which grows with context, and `bar=14%`. The generating CPU ran at 2,731 MHz on the long answer and 3,067 MHz
on the short one.

## Update 2026-10-27: the release tiers and the MVP (PL decision)

See `Plans/DECISION-2026-10-27-release-tiers-mvp.md`.
- **0.1**: every rung is on main and proven on its own, but **the one-boot five-rung showcase has not been run**.
  That's the outstanding 0.1 item (PL question, 2026-10-27).
- **MVP (0.5)**: install (OT-87, which absorbs OT-65), login (OT-66), the text editor (OT-67), terminal, files and
  settings (OT-88), Arthur, wired networking, and clean shutdown and restart (OT-90).
- **1.0**: adds the browser, WiFi, sleep and wake, safe updates and media.
- **Self-hosting**: post-1.0.
Print (OT-68) stays 1.0 or later.

## Update 2026-10-27 (overnight): batch 3 merged, batch 4 gating

**Batch 3** (run `40440467911`, 22/22 green, QEMU shards on four runners in one wave) merged in order:
- **#150 → `c06b3a99`: OT-61 C1.2, per-thread TLS.** This is the browser ladder's next rung after C1.1. Next is
  C1.3 (std threads), then C2. Review: relay #89, GPT-6.1-Sol.
- **#151 → `839b893e`: A9-A.** A9's `presents=0` meant the WM had nothing to draw, not that it was frozen. Adds pointer
  counters and a CPU-0 liveness guard. Review: relay #91.
- **#152 → `22fd551f`: OT-87 R1.** `xtask vm-image` / `vm-run` boot LogansOS as an installed VM disk under pinned OVMF.
  `vm_installed_two_boot` proves the data disk persists across boots. This is the MVP install journey's first rung.
  Review: relay #92.

**Batch 4** (run `40446328993` at `eedfba68`, all green) merged about 12:50 am EDT, and main's code trees equal the
gated ones:
- **#154 → `2f066957`: OT-87 R2.** Arthur's model loads from the installed NeoFS volume, held to the same pins.
- **#155 → `d3603830`: A9-D.** The emblem turns while Arthur thinks. Measured at about 12 presents/s through two 56 s generations.
- **#156 → `500606e9`: A9-B.** The loop stop for degenerate greedy answers. The penalty is inert at 1.0 until Qwen's config is
  fetched.

Reviews: relays #95, #96 and #97 (GPT-6-Luna high). #155 and #156 conflicted with #154 at two adjacent list and const
additions. They were union-resolved on the integration branch, and the same resolution is used at merge.

**Also overnight:**
- **OT-66 login THINK, #157 (S2).** Under review on relay #98. Two decisions wait on J.c.: the recovery code, and the
  idle-lock default.
- **C1.3 (S4).** A worker's `SYS_EXIT` now ends the whole process (exit_group, CP-85). A sibling running in ring 3 on
  another CPU at exit isn't stopped, so the process hangs. Memory stays safe. That's CP-86, OPEN, and C1.3b fixes it.
  APs get no timer tick, which is now recorded as an open fairness item.
- **Arthur scrolling (S1).** C (keys) is ready and C2 (wheel) is built. Both wait on batch 4, which has now merged.

**Batch 6** (run `40457391685`, all green) merged about 2:50 am EDT. It also covered batch 5, whose run was cancelled
by ci.yml's per-ref `cancel-in-progress`; the CI runbook now records the lesson.
- **#158 → `908b35b5`: OT-61 C1.3, std threads.** It brings `std::thread`, futex Mutex/Condvar/RwLock/Once,
  `thread_local!` and `Instant`. A worker's exit ends the whole process. CP-86 (a sibling running in ring 3 on another
  CPU at exit) is OPEN: it's a hang, not a memory bug. C1.3b fixes it and is built, with four-role falsifications. Next
  on the browser ladder is C2. Review: relay #99, GPT-6.1-Sol.
- **#159 → `675495c9`: A9-C.** PgUp/PgDn and the arrows scroll Arthur's reply. Wheel scrolling (C2) follows. Review:
  relay #100.
- **#160 → `7c771c0b`: OT-87 R3.** One GPT disk carries the ESP and a NeoFS partition. The write guard arms exactly one
  LogansOS partition, and never one marked INCOMPLETE. The guard's parser moved, with no decision changed (a
  differential KAT against a frozen copy of the old guard). Ledger row I-17. Reviews: relay #101 (Muse, security) and
  relay #102 (Janus).

**Open defect, found 2026-10-27 (S4, while designing OT-61 C2.1a / OT-66 K1a).** Revoke (`process.rs`
`revoke_handle`) walks a cap's descendants first and removes them afterwards. Three paths can install a new child of a
doomed cap in that gap, and that child then outlives the revoke:
- `derive_handle`;
- `complete_grant`;
- spawn inheritance into a not-yet-registered child.

**A second, deterministic escape (same day):** a derivation edge lives in the holder's own `derivations` map, and
`sys_close` or process exit deletes it. So deriving from a granted cap and then closing the intermediate cap launders
the grandchild out of the revoke tree, with no race needed. Logout's kill-then-revoke order would hit this.

The fix is **K1a-0**: a global `CAP_GRAPH` lock; a re-check at install; spawn registering the child before it runs;
and re-parenting a closed or exiting cap's children to its parent (an ADR-010 amendment). It
lands as its own PR with a falsifier for each path, new CP entries, and GPT-6.1-Sol + Muse reviews. K1a (`RootDirCap`,
shared by OT-61 C2.1a and OT-66 K1) depends on it.

**#161 → `35e01723`: OT-61 C1.3b** (gated on its own branch, run `40465015979`, about 4:10 am EDT). A retiring process now
stops siblings on every CPU: a check at each ring-3 boundary, a reschedule IPI to every CPU that published the pid, and
futex refusal. **CP-86 is closed.** Each mechanism has a falsifier that hangs only its own role. One limit stays
documented: the timer-path race. APs have no timer tick, which is recorded as an open fairness item. Review: relay
#103, GPT-6.1-Sol.
- **#162 → `9a1c35bb`: A9-C2** (batch 7, run `40465526041`, about 4:55 am EDT). Mouse-wheel and touchpad scrolling of
  Arthur's reply. The wheel travels through the pointer route under `DELIVERY`, and the input queue no longer merges a
  wheel report away. Review: relay #104, GPT-6.1-Sol.

**Batch 8** (run `40477098079`, all green) merged about 6:00 am EDT:
- **#163 → `cec200af`: OT-87 R4.1.** Device identity (NVMe, ATA and virtio), the registry generation, `media_generation`,
  and the one-shot install claim with `ClaimedWriter`. It's off with the `installer` feature. Review: relay #105, Muse.
- **#164 → `58ab4966`: OT-61 C2.0.** std entropy over `SYS_GETRANDOM`, with no weak fallback. Review: relay #106.
- **#165 → `0d2608f9`: OT-87 R4.2.** The `NEOINST1` bundle with byte oracles, and two FAT32 ESP templates built by
  vendored `fatfs` (MIT, xtask only). The bundle is reproducible. Review: relay #107.

Next: R4.3 (the writer and the W0–W6 state machine), and #166 (K1a-0) as batch 9.

**OT-87 R4.3, a gap found (S3, 2026-10-27).** Kernel NeoFS can't write a file over 8 direct blocks (32 KiB): there's no
indirect-block writer, and every write is a whole-file copy-on-write through an unbounded block cache. So the
installer can't put the model (485 MB), the vocab or `/deploy/A/` on disk as designed. PM decision: a separate rung
first, **NFS-W1, a NeoFS large-file writer.** It streams data through the guard with a bounded-memory indirect tree and
one commit at the end, behind `neofs-large-write`, with a byte oracle, a host cross-check and a cut-mid-write leg. R4.3
stacks on it. Also found: no block driver presents 4096-byte sectors, so R4.3 adds virtio-blk BLK_SIZE negotiation
behind its own feature.

**#166 → `149d4a19`: K1a-0** (batch 9, run `40484657565`, about 6:50 am EDT). This **fixes the open revoke defect** above:
- a global FIFO `CAP_GRAPH` lock, interrupts off;
- a re-check at every install with a parent, including a cross-process parent;
- re-parenting on close and exit, through `HAS_CHILDREN`;
- spawn registering the child before it runs;
- a source-level edge gate;
- ADR-010 amended.

`cap_graph_revoke` passes all 7 legs, and all 7 leak under the control. Reviews are pending: relay #108 (Muse) and
relay #109 (Janus, GPT-6.1-Sol). A confirmed FAIL there is handled revert-first.
- **#167 → `fa99f551`: NFS-W1** (run `40493398124`, about 8:00 am EDT). Kernel NeoFS can create files of up to 2 GiB
  in bounded memory (135,168 B), with a contiguous data and indirect run, streamed BLAKE3 and one commit. It's behind
  `neofs-large-write`. This unblocks R4.3's W5 (writing the model to the installed disk). Review: relay #110.
- **#168 → `2922bf0c`: K1a / OT-61 C2.1a** (batch 10, run `40500773798`, about 9:00 am EDT). This adds
  capability-rooted open:
  - `RootDirCap` is an ordinary directory handle on "/" that only init holds.
  - The root-form open needs it, gives EPERM without it, and never widens rights.
  - Opened files are its children, so revoke reaches them.
  - Spawn inheritance is widened to directory caps only.
  - Two new syscalls: `SYS_HANDLE_INFO` and `SYS_PROCESS_SELF`.
  It's behind `cap-rooted-open` and off by default. Reviews: relay #111 (Muse) and relay #112 (Janus).
- **#169 → `7d760b99`.** The close-gate now runs CI's product-feature userland clippy, read from `ci.yml` (scripts
  only).

**Muse security RETURNs, 2026-10-27 (pasted by J.c. about 9:35 am EDT).** Banked as
`Plans/RETURN-2026-10-27-*-muse.md`.
- **#108 (K1a-0 revoke, #166): PASS.** Follow-ups that don't block:
  - The edge gate misses aliases, macros, helpers and field reassignment. Making `derivations` module-private would
    close those by construction.
  - No direct accept or fence leg in the scenario.
  - ADR-010 wording: "before any other operation can observe the removal".
- **#111 (K1a RootDirCap, #168): PASS.** Two notes:
  - Defence-in-depth nit: add `is_directory()` to the own-dir spawn-inherit branch (S4, on its next push).
  - The `.text` claim rests on the gate run.
- **#101 (R3 partition rule, #160): FAIL, item 5.** Overlapping GPT entries arm a foreign span. Details:
  - The case: an ESP and a LogansOS entry over the same LBAs, plus a sealed superblock.
  - Crafted disks only. A real Windows disk never has overlapping entries.
  - Not in any product image: `neofs-partition` is off.
  - The PM re-checked it at `8e80abde`: `gpt_logic::validate` has no pairwise check.
  - **Fix-forward, PR A to S3**, due 17:40 UTC (1:40 pm EDT). Otherwise #160 is reverted.
- **#105 (R4.1 install claim, #163): FAIL, item 4.** NVMe has no namespace-change notices (no AER at all) and no
  reset notices. Details:
  - The PM re-checked it at `8e80abde`.
  - Not reachable today: `installer` is off and no installer writes exist on `main`.
  - **PM call: fix, not sign it into the stated limit.** THINK §2a item 5 makes driver reporting a prerequisite.
  - **PR B to S3:** NGUID/EUI64 identity, AER namespace notices, and the reset argument.
  - It must merge before R6 and before `installer` boots on hardware. It does not block #170 (VM only).
  - Dispatch: `Plans/DISPATCH-2026-10-27-ot87-review-fails-fix-s3.md`.

**Codex review bundle RETURNs, 2026-10-27 (J.c. pasted the verdicts at about 9:45 am EDT).** The RETURNs are banked
on main from the reviewers' branches. 1 PASS, 10 FAIL. The PM re-checked every FAIL against `origin/main`, and all
hold except C1.2 item 5.
- **PASS: #92 R1 vm-image (#152).**
- **In the product image. Fix-forward to S1, due 18:00 UTC (2:00 pm EDT), else revert:**
  - **#104, #162 wheel.** Two findings. F1: a forwarded kind-5 wheel evicts a queued key. F2: a press plus a wheel
    outruns RESYNC into a stale grab.
  - **#100, #159 scrollback.** `view_top` goes stale after the 512-line eviction.
  - **#97, #156 repetition guard.** At `p=1.0`, a signalling NaN is quieted. Overlapping windows cut a short phrase
    repeated three times. The PM kept the contract: windows must not overlap.
  - Dispatch: `Plans/DISPATCH-2026-10-27-review-fails-fix-s1.md`.
- **`browser-runtime` only. Fix-forward to S4, ahead of C2.2's merge:**
  - **#89, #150 C1.2.** A user `mov fs, 0x23` zeroes the hardware FS.base under the per-CPU cache. Items 2, 3 and 7
    are real. Item 5 is ruled no change: the `%fs:` instruction scan was in the PM's checklist, not the build's
    contract.
  - **#99, #158 C1.3.** Its late park is folded into #103.
  - **#103, #161 C1.3b.** Two holes:
    - a refused `SYS_THREAD_JOIN` spins forever in CPL0;
    - the retire IPI can be used up in the IF=1 tail after the last check.

    CP-86 is reopened.
  - Dispatch: `Plans/DISPATCH-2026-10-27-review-fails-fix-s4.md`.
- **Off in product, or host-only. S3 PR C:**
  - **#107, #165 R4.2.** The `dirent_offset + 32` add is unchecked.
  - **#110, #167 NFS-W1.** A reservation leaks on a write error.
  - **#95, #154 R2.** The vocab fallback mislabels the model source.
  - Dispatch: `Plans/DISPATCH-2026-10-27-ot87-review-fails-fix-s3.md`.
- **#98, OT-66 THINK (design).** Four gaps:
  - Try-mode exposure of the installed disk is unstated.
  - NEOCRED1's torn-write guarantee is unspecified.
  - The wrong-password delay resets when the WM restarts.
  - `SYS_MODEL_LOAD`, which is already cap-gated by K1a #168 (Muse #111 item 1), needs saying.

  These go to S2's #157 revision.

**Janus RETURNs, first 3 of 7 (pushed about 9:20–9:40 am EDT; banked about 10:00 am).** All three FAIL. The PM
re-checked the main findings.
- **#109, #166 K1a-0: FAIL, in the product image (`CAP_GRAPH` is ungated).** Three findings:
  - **F1:** a snapshot `Arc<Process>` can be the last owner. Its `Pmap::drop` then runs a shootdown under
    `CAP_GRAPH` while another CPU spins IF=0, which ends in a 50 ms timeout panic.
  - **F2:** the last tensor release parks in `guarded_clone` under `CAP_GRAPH`.
  - **F3:** `TicketMutexGuard` is `Sync` without `T: Sync`.

  **Cross-family divergence:** Muse's #108 item 7 PASSed "no park or wait under `CAP_GRAPH`", and Janus broke it. The
  same lesson as C1.1. **Fix-forward to S4, due 18:00 UTC, else revert #168 then #166.**
- **#112, #168 K1a: FAIL (behind `cap-rooted-open`, off in product).** Two findings, now with S5:
  - **F1:** readdir, mount and umount check the backing `OpenFile`'s rights, not the attenuated handle's. A LOOKUP-only
    handle can readdir, and a client root without MOUNT can umount. This contradicts Muse's #111 item 1.
  - **F2:** the spawn `is_directory()` takes a NeoFS `SwitchMutex` at IF=0.

  Dispatch: `Plans/DISPATCH-2026-10-27-k1a-rootdircap-fix-s5.md`.
- **#91, #151 A9 presents/liveness: FAIL, diagnostic overlay only.**
  - The counter windows aren't atomic, so `in=0 rx=1` is possible.
  - The PR has no CP table.
  - No revert: it never changes a reply. Sent to S1 as PR 4, no deadline.
- Still due from Janus: #93, #96, #102, #106.
- **#117, Chad's re-check of the OT-66 THINK: FAIL on 2 items (pushed `02d94be1`).** Chad reviewed `3c3939c5`, because
  he started before the relay was repointed to `b555159e`.
  - **Item 2 (K1 handle-rights rule): resolved at `b555159e`.** The PM checked §3.4 (THINK:156–159) and K1b's close
    gate.
  - **Item 5: open.** L2's frame-digest check doesn't include a client spawned while locked that commits pixels. Sent
    to S2.
  - Items 1, 3, 4 and 6 PASS.
- **#157 → `ee5df102`: the OT-66 login/session THINK merged** (docs only, about 10:10 am EDT). Chad's item 5 is closed
  at `6d30b39d`: L2's digest gate now covers a client spawned while locked that draws (PM-checked). D3 (the recovery
  code) and D4 (the idle-lock default) are still open with J.c.; they amend the THINK when decided.
- **D3 and D4 decided by J.c. (about 10:10 am EDT):** yes to the recovery code shown once at account creation, and
  yes to a 10-minute idle lock on by default. The OT-66 THINK §10 rows 2 and 4 are updated on main.
- **#156 item 2, PM call reversed (14:15 UTC).** Keeping the "short phrase never cut" contract would make a one-token
  loop show 16 repeats before the stop: the oracle changes from `11,0,3,4,10,3` to `11,0,3,4,10` plus sixteen 3s. So
  the contract is reworded instead: cut only after 16 tokens of exact periodic repetition, and keep one copy. The code
  and oracle stay as they are. S1's PR 3 keeps only the `p=1.0` identity fix plus the KATs.

**Janus RETURNs, the other 4 of 7 (J.c. relayed all 7 at about 10:20 am EDT).** All FAIL. The PM checked each one.
- **#102, #160 R3: FAIL item 7.** A primary with `last_usable == last_lba` (`alternate_lba` unchecked) lets a
  partition claim cover the backup GPT array and header (I-17). This goes into S3's #172, under the same 18:00 UTC
  fix-forward.
- **#96, #155 busy emblem: FAIL item 1, in the product image.** A refused `OP_BUSY` returns 120, and neo-init exits,
  which takes down the whole desktop. **Fix-forward to S2, due 18:30 UTC.** #155 can't simply be reverted, because
  #159 and #162 build on it. Dispatch: `Plans/DISPATCH-2026-10-27-wm-busy-refusal-fix-s2.md`.
- **#93, #153 OT-90 THINK (open, docs): FAIL items 2, 4 and 8.**
  - Item 4 exposed a **pre-existing NeoFS crash-consistency defect on `main`**. Every superblock copy gets the new
    generation in the same drain as the roots, with one flush after all of it. A write-back cache can persist the
    superblocks first, and then no generation is valid.
  - S5 builds the commit barrier first, then THINK rev 2. Nothing to revert. Dispatch:
    `Plans/DISPATCH-2026-10-27-neofs-barrier-and-ot90-think-s5.md`.
- **#106, #164 C2.0: FAIL item 7 (docs only).** The `getrandom` crate was missing from the prior art. The PM added
  it to #164's body: MIT OR Apache-2.0, `7ce68004`, 2026-10-26, checked by a shallow clone.
- Janus's tally for the day: 7 of 7 FAIL. Two of them broke Muse PASSes: #166 F1/F2 and #168 F1.
- **#171 → `02e9993c`: OT-61 C2.1b, sealed std fs** (batch 11, run `40511219700`, 22/22 green, merged about 10:35 am
  EDT). All nine code trees equal the gated `5177c10d`. Reviews: relay #113 (Jarvis) and relay #114 (Muse), both now
  pointing at `02e9993c`.

**PAUSED at about 10:35 am EDT, 2026-10-27, at J.c.'s request (the weekly usage limit is near; it resets tomorrow).**
- Every seat (S1–S5) was told to commit and push its work in progress, then stop.
- All scheduled check-ins are disabled, including the 18:01 UTC fix-forward deadline check and the OptiPlex reminder.
- **The fix-forward deadlines are suspended, not missed.** On resume, restart the clock from that moment.
- **Batch 12 is running on the CI runners** (no Claude usage): main `8bf71c02` + #173 `cbf9ae23` (wheel) + #175
  `c0171f3c` (scrollback) + #177 `a700f17b` (repetition) + #170 `40137eae` (R4.3). The integration head is `ffec56bf`.
  Local fmt and the host KATs passed.
- **On resume:**
  1. Read batch 12's result. If it's green, merge #173, #175, #177 and #170, after checking each tree.
  2. Batch 13 is the #166 fix (S4, `claude/k1a0-review-fixes`, after the IF-off pinned-Arc question), #172 (once it
     carries both R3 fixes), #174 and #176. Then S2's WM refusal fix and S5's NeoFS barrier.
  3. Re-enable or recreate the check-ins as needed.
- **Pause notes (from the seats' last messages):**
  - **#172 is at `6f4521d4`** and carries both R3 fixes. S3's deviation is accepted: a primary's `alternate_lba` may
    be the image's own end, not the device's last LBA. Our boot ISO, written to a larger disk, has its backup at the
    image's end, and the log sink past it must still arm. The rule enforces `last_usable + A < alternate`, and
    `partition_claims` drops anything touching any metadata span. S3's QEMU regressions were still unfinished at the
    pause.
  - **S2's WM refusal fix is at `cdf4c4a5`, not yet verified, no PR yet.** A refusal answers in the reply status
    word: 3 for out-of-bounds, 7 for the wrong channel. The scene is unchanged and the WM keeps running.
    - Still to do: the re-run, red on today's code, the kill-switch diff, then the PR.
    - S2 proposes: a client closing its channel stays fatal for the MVP; five other client-error arms (Commit,
      Configure, Destroy, Yield, `_`) become a follow-up lane.
  - **S4's #166 fix is at `d7ef6358`, no PR yet.** The sweep stopped at 11 of 45, all PASS. `.text` grew 5,248 B.
    C2.2 is at `b28c4d44`.
  - **S5's NeoFS barrier is at `c4da915d` (WIP).** It also found a **second pre-existing NeoFS defect**: truncate's
    freed blocks are reused before the freeing generation is durable. Mount checks file content hashes, so the last
    durable generation can be invalidated too. The WIP fixes it with deferred reuse. Still to do: the cut KATs, prior
    art, numbers, the close-gate and the PR, then the #153 revision.
  - **All five seats confirmed paused, about 10:36 am EDT.** Clean trees, nothing unpushed, no check-ins armed.
  - **S1:** PR 4 (#151's counter windows) is at `fc4c901f`, no PR yet.
  - **S3:** #172 at `6f4521d4`, whose QEMU GPT regressions (four scenarios) PASS; its close-gate, `.text` hashes and
    PR body are still to do. PR B (NVMe) is at `f5920d65`, close-gate green, no PR yet.
- **#118, Janus's re-check of the OT-90 THINK rev 2 (#153 @ `15379e82`, pushed `ee63abfb`): FAIL on item 2.** The
  byte-scan `\_S5` rule can accept a lookalike inside a `Buffer` payload. For example,
  `Name (DUMY, Buffer { 0x08 '_S5_' 0x12 … })` with no real `_S5` gives SLP_TYP 0. The planned KATs also lack an SSDT
  case. Items 4 (durability, resting on #180) and 8 (dates) PASS. Back to S5.
- **#119, Janus's second re-check of the OT-90 THINK rev 3 (#153 @ `8ecbde6b`, pushed `bb87230b`): FAIL on two
  evidence and claim points, not a lookalike accept.**
  - The prototype returns at the first `_S5`, so it never walked the remaining SSDTs, and the "93/93 walked" claim is
    unsupported.
  - A call to a method declared later (`CALL` assumed to take 0 args) misaligns the walk without failing closed, so the
    THINK's "a misaligned walk fails closed" isn't guaranteed. In Janus's input it still found a genuine `_S5`.
  - No non-root lookalike was accepted in the listed cases. Back to S5.
- **#120, Janus's third re-check of the OT-90 THINK rev 4 (#153 @ `a6d9def1`, pushed `50f2e67c`): FAIL on one
  narrow point.**
  - **Passes:** the rerun (93/93 walked, 10/10 decoded) and all four arity probes. The wrong-declared-arity limit is
    bounded in the cases checked.
  - **The fail:** a `Name` inside `If (Zero)` leaks into the unconditional declaration set and satisfies a later
    strict lookup. In Janus's example the `_S5` is still the genuine root one.
  - **Next:** back to S5 for rev 5. The PM verifies rev 5 itself, by rerunning the prototype with Janus's probe, unless
    the change goes beyond scoping conditional declarations.
- **#153 → `8a977362`: the OT-90 shutdown and restart THINK merged** (docs, plus the `_S5` walk prototype and probes).
  Revision 5 (`eaa07679`) closes Janus #120. PM-verified by rerun: 93/93 tables walked, 10/10 sets decoded, probes
  10/10. D1 and D2 are still with J.c. S5's P1 waits for #180 to merge.
- **J.c.'s decisions (about 9:00 pm EDT on 10/28):**
  - **Icons:** Arthur A, Lens A, Terminal A, Files A, Settings A (the sliders), Install B, Browser B. The B shapes
    are redrawn with Silver structure and a blue accent, for the 3:1 contrast floor.
  - **OT-90:** D1 and D2 both take the recommendation. Only guard-armed disks are flushed and shut down; the
    Zenbook's Windows NVMe is never touched. The MVP ships without `_PTS(5)`, and the bare-metal checks F1, F2 and F7
    decide whether that holds.

**Batch 12b merged** (run `40787440872` at `679fd739`, all green, about 9:10 pm EDT on 10/28). Seven squashes:
- `#178 → 7bd9a9a0`: the virtio_accel test race.
- `#173 → 36fe4784`: the wheel fix.
- `#175 → ee223760`: the scrollback fix.
- `#177 → 953a27d3`: the repetition guard.
- `#170 → d72d4470`: OT-87 R4.3, the installer writer.
- `#174 → 57b7960e`: the K1a fixes.
- `#176 → 2fe2b399`: OT-87 fixes C.

main's code trees equal the gated ones. Batch 12 (run `40522393923`) had been red on
`virtio_accel_identity_roundtrip`, a test race fixed by #178.

Review relays:
- Chad bundle #121–#124 (Sol high);
- Jarvis bundle #116 (R4.3) and #125 (the guard);
- Janus #126 (#174, Sol medium);
- Muse #115 (R4.3 writes).

**Batch 13 dispatched** (run `40799093977` at `0395982f`): #183, #184, #172, #182, #180, #181 and #179. Four
conflicts were union-resolved: `test_harness.rs`, `concurrency-pitfalls.md`, `Cargo.toml`, and the `virtio_blk.rs`
DriverIdentity.
- **OT-87 R4.4 started (S3, `claude/ot87-r4.4-ui`).** The PM approved the entry-point design:
  - a kernel-minted `Handle::Installer` cap, held only by neo-init, neo-shell and the installer role;
  - `SYS_INSTALL_LIST` and `SYS_INSTALL_RUN` at 0x1C0; LIST takes its verdict from `preflight`, and RUN is
    synchronous;
  - an `InstallRowV1` byte oracle;
  - the console `neo-shell install`;
  - the Atrium installer as a neo-worker `installer` role, not a fifth `neo-installer` module, which avoids a bundle,
    slot and pin change.

  Reviews: Muse for the syscalls, the cap and the claim; Codex GPT-6-Luna high for the UI.
- **Post-merge reviews of batch 12b, first returns:**
  - **Janus #126 (#174, the K1a fixes): PASS.** F1, F2 and the three discriminators all pass. #112 is closed.
  - **Chad bundle (pushed `codex/review-bundle-2026-10-29-chad`):** #121 wheel PASS, #122 scrollback PASS, #124
    virtio test PASS. The motion-first mutation fails the wheel KAT, as it should.
  - **#123 (#176) FAIL, one narrow NFS-W1 path.** `reserve_inode_block_and_generation` allocates the inode block before
    checking `next_generation + 1`. At `next_generation == u32::MAX` it returns an error and leaks that block, and
    without a free map the whole run.
    - Reachable only after 2^32 generations or from a crafted superblock.
    - Behind `neofs-large-write`, off in product, so no revert.
    - Fix to S3: check before allocating, and add a KAT.
- **Jarvis bundle (pushed `17841aaf`): both items FAIL on docs only. The implementation checks pass.**
  - **#116 (#170, R4.3):** the W0–W6 durability, recovery, GPT writer, W5 path, virtio BLK_SIZE and kill-switch all
    PASS. Jarvis also re-ran the cut sweep with a fresh Finish after every incomplete result, and it passed. The one
    gap: THINK §4.5's refusal list lacked the no-flush row. The PM added it on main.
  - **#125 (#177, the guard):** the NaN fix and the oracle PASS. The new module wording still says a phrase of any
    length said twice is never cut, but `(0..32)%16 → Some(16)`. The comment wording goes to S1, with no code change.
- **Muse #115 (#170, the R4.3 installer writes): PASS on all seven items.**
  - Scope note 1(a), for R4.4: a claim doesn't lock out a concurrent plain-guard-path writer. R4.4 adds the first
    production caller, so it should close that while a claim is live. Sent to S3.
  - The protected-disk table is still empty, which R4b erase needs.
- **J.c. approved the icons for the product image (about 10:00 pm EDT on 10/28)**, after the
  `desktop_icons_boot` screenshot. S1 turns `atrium-app-icons` on in the product and image-A mappings inside #185,
  with a mapping KAT. It merges in the batch after batch 13; the Codex review (GPT-6-Luna high) follows the merge.
- **Evening of 10/28 (EDT):**
  - The README was refreshed to 2026-10-29 (`493c1472`).
  - S3 found an escaped defect in its own R4.3 code: the boot-disk refusal never fired. The `limine` crate 0.6.3 reads
    `gpt_disk_uuid` at offset 56 instead of 64. It's a fix-forward, #186, off in product.
  - S3's NFS-W1 generation check is pushed (`a54ae657`).
  - Queued for batch 14: #185 (icons, product-on, J.c.-approved), #186, the NFS gen check, and C2.2 if it's ready.
  - J.c. went to bed at about 10:10 pm EDT.
- **OT-87 R4.4 UI decision (PM, about 10:15 pm EDT on 10/28).** S3 found the approved "installer role in neo-worker"
  rests on a precedent main doesn't have. Worker roles are cargo features, and only Arthur receives input. The PM
  picked S3's option A: the installer is a WM view in neo-init (`neo-init/src/installer_view.rs`), launched from a
  Home Spine chip. The Installer cap stays with neo-init and neo-shell. Runtime worker roles with pointer forwarding
  (option B) are deferred until a second input-receiving client is needed for some other reason. J.c. hears this in
  the morning summary.
- **Batch 13 merged (about 10:55 pm EDT on 10/28).** Run `40799093977` was green at `0395982f`, and main's code trees
  equal it. Squashes:
  - #183 `86ed5214`, the K1a-0 CAP_GRAPH fixes (product);
  - #184 `7636372b`, the C1 fixes;
  - #172 `cf438ab8`, R3 GPT;
  - #182 `bbba7381`, the WM refusal fix;
  - #180 `c805f2b2`, the NeoFS barrier (product);
  - #181 `ee629498`, NVMe events;
  - #179 `de8547c1`, the prefill-perf window.
- **Post-merge reviews, relays #127–#136:**
  - Janus bundle #127–#130 (#183, #182, #179, #172 item 7), GPT-6.1-Sol high:
    `Plans/DISPATCH-2026-10-29-review-bundle-janus.md`.
  - Jarvis #131 (#184), GPT-6.1-Sol medium: `Plans/DISPATCH-2026-10-29-ot61-c1-fixes-recheck-jarvis.md`.
  - Muse bundle #132–#134 (#172 item 5, #180, #181): `Plans/DISPATCH-2026-10-29-review-bundle-b13-muse.md`.
  - Chad bundle #135–#136 (#180's cut KAT, #181's spec rows), GPT-6.1-Sol high:
    `Plans/DISPATCH-2026-10-29-review-bundle-b13-chad.md`.
- **The R4.4 launch path:** typed `install` at Home ships in R4.4. The Spine chip is R4.4b, after #185.
- **Fix-forward deadline check, 03:30 UTC on 10/29 (11:30 pm EDT): every fix is merged, so nothing is reverted.**
  - (a) #160's R3 fixes: #172 `cf438ab8`.
  - (b) S1's A9 fixes: #173 `36fe4784`, #175 `ee223760`, #177 `953a27d3`.
  - (c) #166's fix: #183 `86ed5214`.
  - (d) #155's fix: #182 `bbba7381`.
- **Batch 14 was red (run `40808500747`), on a test bug in #185, not an OS bug.** `desktop_icons_boot`'s xtask
  post-check read `desktop_home_boot.ppm`, another scenario's capture, instead of its own. It passed on S1's machine
  only because that file was left over from an earlier run. S1 fixed it in `f90b7b7c`, failing on the old code and
  passing on the new with the capture directory cleared.
- **Batch 14b merged (about 2:05 am EDT on 10/29).** Run `40817887458` was green at `67f023bc`, and main's code trees
  equal it. Squashes:
  - #186 `e51d8a00`, the boot-disk GUID at Limine's offset;
  - #185 `40f60300`, the app icons (product-on, J.c.-approved);
  - #187 `876dbdab`, the NFS-W1 generation check, with #180's deferred-free skip ported into `alloc_logic`;
  - #188 `5c03fe01`, OT-90 P1, the host-only power logic;
  - #189 `46d6ba1e`, every WM client error refused instead of fatal;
  - #190 `130a7ef3`, OT-61 C2.2 memory quotas.
- **Post-merge reviews, relays #137–#143:**
  - Muse #137–#138 (#186, #190's containment): `Plans/DISPATCH-2026-10-29-review-bundle-b14b-muse.md`.
  - Chad #139 (#187), GPT-6.1-Sol medium: `Plans/DISPATCH-2026-10-29-nfs-w1-gen-recheck-chad.md`.
  - Janus #140 (#188), GPT-6-Luna high: `Plans/DISPATCH-2026-10-29-ot90-p1-review-janus.md`.
  - Jarvis #141–#143 (#185, #189, #190), GPT-6.1-Sol high: `Plans/DISPATCH-2026-10-29-review-bundle-b14b-jarvis.md`.
- **New lanes:**
  - S2: OT-66 L1, login (`Plans/DISPATCH-2026-10-29-ot66-l1-login-s2.md`). Its hardware timing run is an acceptance
    item after merge, and J.c. schedules it.
  - S5: OT-90 P1 merged; P2, the mechanism, is next.
- **Not MVP, noted:** a Lens that sends a second commit after blooming is never served and blocks in its reply (S2).
- **OT-88 app-model THINK, #191 (S1, `7ff0a3b5`).** It recommends Design B, one binary per app with one app in
  front, and no kernel change for the app model. The terminal needs one kernel rung, T1, a pipe handle, and Files needs
  F0, NeoFS `mkdir_at`/`unlink_at`/`rename_at`/`stat_at`. Decisions D1–D5 wait on J.c. (morning summary). Cross-family
  read: relay #144, Chad, GPT-6-Luna high (`Plans/DISPATCH-2026-10-29-ot88-think-review-chad.md`).
  - Other lanes: S4 gets #183's two-CPU grant leg, then Muse's #166 items and K1b's private `derivations`, then C2.3a.
- **OPEN-THREADS OT-83 marked resolved** (#92, `4086a8ed`). S1 found the row still said "in flight".
- **OT-67 editor THINK, #192 (S1, `3c5f0a3e`).** A plain-text MVP editor on neo-text T0: an in-tree gap buffer, a 4 MiB
  cap, and a bounded undo list. Atomic save needs a new kernel rung, W1 (`SYS_FILE_REPLACE`, one commit through
  NFS-W1's indirect writer). Its decisions (E-D1–E-D5) wait on J.c. Cross-family read: relay #145, Janus, GPT-6-Luna
  high (`Plans/DISPATCH-2026-10-29-ot67-think-review-janus.md`). S1 starts E1 (`neo-edit-core`, host KATs only).
- **AAG on #195 (OT-90 P2, S5) before merge: two Claude lenses found five defects, all re-verified by the PM. Bounced
  to S5.**
  - Lens 1, port I/O, interrupts and unsafe:
    - F1: the 10 s S5 retry never ends on a 24-bit PM timer, which most of the corpus has, the Zenbook included, so
      "safe to turn off" is never reached;
    - F2: a SystemMemory reset or sleep register is accepted with no address check, and a high address panics;
    - F3: the S5 path can allocate when it maps an MMIO page on first use.
  - Lens 2, completeness:
    - F4: the restart judge doesn't count boots, so a crash in the boot after the reset passes;
    - F5: no scenario runs a wait that times out.
  - Clean: the register order, the IF masking (CP-64), the QMP handshake, and hermeticity. `.text` with `power-control`
    off was reproduced for `default`.
  - A cross-family read still follows the merge.
- **Batch 16 candidates:** #196 (S1, E1 `neo-edit-core`, host-only), #197 (S4, Muse's #166 follow-ups and the private
  `Derivations`), and #195 once its fixes land.
- **#195 AAG fixes F1–F5 are in, at `e183ee66`.**
  - F1: the PM-timer wait sums masked steps, and the KAT runs the kernel's own loop.
  - F2: `phys_ok` gates SystemMemory registers.
  - F3: register pages are premapped at init.
  - F4: the judge counts boots by BEGIN frames, allowing one or two RESETs.
  - F5: the fallback's test-only step 1 waits 6 s across a 24-bit wrap on the real PM timer.
  - #195 goes into batch 16.
- **#198, OT-66 L1 login (S2, `71256639`):** in batch 16, with login off in the product. Turning it on (it adds
  kernel-rng to the product kernel) is J.c.'s call, after Muse's L1 review.
- **Product-image defect found (S4, about 3:30 am EDT on 10/29; the PM re-verified it on main): one syscall from any
  process panics the kernel.**
  - `SYS_TENSOR_WRITE` (and `SYS_TENSOR_CREATE`) copy a caller-chosen length into the kernel heap through
    `copy_user_bytes` (`syscall.rs:1474`, an infallible `Vec::with_capacity`). That happens before any handle lookup or
    length check, and the kernel has no `alloc_error_handler`. `copy_user_bytes` has 25 callers.
  - It's a pre-existing defect, not a new escape. S4 fixes it now as its own PR:
    - a fallible copy (ENOMEM);
    - a per-site ABI bound (EINVAL);
    - a scenario that panics on the old code.
  - Then Muse plus a cross-family check.
  - The other heap-exhaustion vectors S4 found (spawn loops, handle slots, sockets that may outlive their process,
    buffer-map pins, the NeoFS cache) go to the C2.2b design.
- **#199 (C2.3a servo-host-smoke) found CP-94:** neo-std-pal's heap assumed contiguous growth and wrote into a thread
  stack's guard page. It's fixed in #199. **#200 (C2.3b closure spike, docs):** two supply-chain decisions for J.c.
- **Socket leak confirmed by a code read (S4).** A socket handle leaving a table any way other than
  `SYS_SOCKET_CLOSE` (exit, kill, revoke) never closes the socket: the registries' own `Arc` keeps `Drop` from running,
  and `Detached::release` releases only tensor counts. So a bound port stays bound forever, and bind-and-exit in a loop
  leaks kernel heap. Fix, a small PR after the `copy_user_bytes` fix: close the socket when its last handle leaves.
  Included: a lock-order pitfall pass, a race argument, and exit, kill and revoke discriminators.
- **C2.2b design decisions (PM):**
  - bytes with declared weights, KAT-checked to over-count;
  - the parent pays for a spawn;
  - the ABI extends `SYS_SPAWN_BUDGETED` only if it already refuses a non-zero spare argument, otherwise a new
    syscall;
  - order: `copy_user_bytes`, then the socket leak, then C2.2b.
- **New SMP-core defect, found by S4 while working on the socket leak (to become OT-93 / CP-95): a force-kill of a
  thread that is Ready inside a syscall leaks what its kernel stack frame owns.**
  - `kill_thread_by_pid` zombifies Ready threads, and the reaper frees the stack raw, so destructors never run (the
    CP-30/CP-65 class).
  - Concrete site: `ipc::Channel::send`'s rendezvous arm holds `Arc<Pmap>` locals across `handoff_to_rendezvous_peer`.
    A receiver that kills the sender there leaks both address spaces.
  - Seen in 2 of 9 runs, as a frame leak of +52, with `pmap_strong=3` at the kill.
  - Product-exposed through `SYS_KILL`.
  - Fix after the socket PR, before C2.2b: the general rule that a thread Ready in kernel mode exits at its syscall
    boundary, never zombified. It needs a deterministic discriminator and a cross-family AAG lens before the merge.
- **Batch 15 merged (about 4:40 am EDT on 10/29).** Run `40831274927` was green at `5a37066a`, and main's code trees
  equal it.
  - #193 `fec09323`, the CAP_GRAPH two-CPU grant leg (test-only).
  - #194 `e01368bf`, OT-87 R4.4, the installer UI: a neo-init WM view opened by typing `install`, a console fallback,
    and the Installer cap.
  - Relays: #146 Jarvis (#193, GPT-6.1-Sol medium), #147 Muse (#194's cap and syscalls), #148 Chad (#194's UI and
    scenarios, GPT-6-Luna high).
- **Batch 16 gating:** run `40843436986` at `fb039fbd` = main `e01368bf` + #201, #195, #196, #197, #198 and #199.
  - Two conflicts, each union-resolved: `syscall.rs` (the installer arms and #198's shared test arm), and xtask
    (modules and scenario entries).
  - Locally, on a code-identical trial merge: fmt, the changed KATs, xtask's 157 tests, and kernel checks with
    `power-control`, `installer`, `user-copy-bounds` and `servo-host-smoke`.
- **#202, the socket teardown leak fix (S4, `d1453ada`), queued for batch 17.**
  - A per-object handle count closes a socket, TCP connection or listener when its last handle leaves (exit, kill or
    revoke).
  - Closes are matched by `Arc::ptr_eq`, and a listener's orphan connections are closed with it.
  - It's in the product: the socket syscalls are ungated and ambient in image A.
  - Also found, a new row to come (OT-94): `passive_open` doesn't bound half-open connections, and a full backlog leaves
    handle-less connections in `CONNECTIONS`. That's a remote-peer DoS, a small lane after OT-93.
- **R4.4b (S3):** the Install chip ships with an interim visibility rule (this boot can install: the cap plus a
  non-MEDIUM LIST row). R5 rewires it to the Try flag.
- **#203 R4.4b, the Install chip (S3, `42f11aff`)**, and **#204 OT-93, the kill-of-Ready-in-kernel fix (S4,
  `b50c047f`)**, are up.
  - #203 goes into batch 17 with #202.
  - #204 is the general fix: a thread Ready inside a syscall is spared and exits at its syscall boundary, never
    zombified. It's SMP core, so it waits for its pre-merge cross-family AAG (relay #149, Janus, GPT-6.1-Sol high,
    `Plans/DISPATCH-2026-10-29-ot93-aag-janus.md`), then gates on its own.
  - Open question: `threads_tls` failed once in six on #204. S4 runs 20× on main and on the branch to attribute it.
- **Batch 16 merged (about 6:15 am EDT on 10/29).** Run `40843436986` was green at `fb039fbd`, and main's code trees
  equal it. Squashes:
  - #201 `1faa0bec`, the `copy_user_bytes` panic fix (product);
  - #195 `83d8e74f`, OT-90 P2, power-off and restart behind `power-control`;
  - #196 `57f23455`, OT-67 E1 `neo-edit-core`;
  - #197 `cbe57444`, the K1a follow-ups;
  - #198 `80fa7b1b`, OT-66 L1 login (off in product);
  - #199 `81d72147`, OT-61 C2.3a and CP-94.
- **Relays #150–#156:**
  - Muse #150–#152 (#201, #197, #198): `Plans/DISPATCH-2026-10-29-review-bundle-b16-muse.md`.
  - Jarvis #153–#156 (#201's table, #195, #196, #199), GPT-6.1-Sol high:
    `Plans/DISPATCH-2026-10-29-review-bundle-b16-jarvis.md`.
- **Product defect found by S1 (PM re-verified on main): every NeoFS overwrite leaks the previous version's
  blocks.**
  - `commit_regular_write` (`inode.rs:555-610`) reserves new data blocks and a new inode block, and never releases the
    old ones (truncate does, at `:667`).
  - `run_gc_locked` exists only under `neo-test-phase3-neofs-3c`.
  - Affects Arthur's memory writes and any `sys_write`. Fix to S1 now, before W1: release through #180's deferred-free
    rule, with KATs and a 100-overwrite scenario.
- **OT-67 W1 design** (`SYS_FILE_REPLACE`, built on truncate's whole-file COW path) is in #192 §5.2. Janus's #145 read
  predates it.
- **threads_tls:** 1 failure in 26 on #204, 0 in 21 on main. The PM's hypothesis is a test race in #184's check 8
  (a switch-in restores FS.base between the selector load and the read). S4 confirms it, then makes a test-only fix.
- **C2.2b ABI:** `SYS_SPAWN_BUDGETED` ignores a5 today, so C2.2b adds `SYS_SPAWN_BUDGETED2`.
- **#206, the NeoFS supersede-free fix (S1, `e9c78ff0`).** Every commit leaked the file's old blocks, the old directory
  chain and the old free map, and permanently, because mount never rebuilds the map.
  - The pre-merge Claude AAG lens (crash consistency) found:
    - C1, confirmed by the PM: `free_superseded` isn't atomic. A refusal partway leaves live blocks free, not deferred.
      It's reachable through a crafted image that names a block twice, which mount doesn't reject.
    - U1: the expected-old check compares only the inode block number, so a two-writer ABA race can free live blocks.
  - Both are bounced to S1 before batch 18. U2–U4 go in the PR body as stated limits.
- **#205 C2.2b `030dce25` and #207, the threads_tls step-6 test fix (`48ff8fd3`), are queued for batch 18.**
  - S4's next lanes: OT-94 (a bounded SYN queue, a product remote DoS), then OT-91.
- **Possible pre-existing defect, sent to S3 to confirm:** after a NeoFS `grow`, rewritten records keep the old
  `fs_block_count`, and the read and mount checks use it. The installer's W5 may hit this.
- **Product security defect found by the pre-merge AAG on #208 (PM re-verified on main): any process can take the
  keyboard route.**
  - `sys_input_focus_create` needs no authority, and `sys_input_focus_mint_activator` (`syscall.rs:8213`) turns
    RECEIVE into an ACTIVATE sibling in the caller's own table.
  - `sys_input_focus_activate` then swaps `ACTIVE_INPUT_FOCUS` (`focus.rs:200`).
  - So any client can read every key the user types: Home's intent, the sign-in and lock passwords. The rights
    comment ("never ACTIVATE authority") is false in practice.
  - Fix to S2 first: route activation becomes the input-system holder's alone, with an EPERM discriminator. Muse
    reviews after the merge.
- **#208 (L2) is bounced** for F2 (a channel flood starves `lock::service`) and F3 (Arthur's control waits skip
  `lock::service`), plus U2: `arm` requires the sak-lock activity bit. It's out of batch 18 until they're fixed.
- **Batch 17 merged (about 7:45 am EDT on 10/29).** Run `40855721787` was green at `e29f21d0`, and main's code trees
  equal it.
  - #202 `34c6c132`, the socket teardown leak (product).
  - #203 `1c1df371`, R4.4b, the Install chip.
  - Relays: #157 Muse (#202); #158–#159 Chad (#202's concurrency, #203), GPT-6.1-Sol high.
- **L2's send-side stall (CP-57):** the WM's synchronous sends to a client that never receives park the WM, so the
  lock can't engage. Decided (PM): a kernel-side lock route, rung L2b. On Super+L the kernel swaps the route to the
  WM's registered lock focus. It comes after F1 and #208's F2/F3, with a cross-family AAG.
- **Up for batch 19:**
  - #210 (S2, `138a0698`), the input-route activation fix: activate requires InputSystem (product security);
  - #211 (S4, `9ca9d6f1`), OT-94: half-open connections are bounded per listener, there are no orphans, and a 3 s
    handshake timeout.
  - #209 (S1, W1 `SYS_FILE_REPLACE`) is stacked on #206 and re-stacks after it merges.
- **Facts from #211:** there's no wire TCP receive path yet; loopback reaches `inject_segment` only from syscall
  context. And nothing calls `net::tick` on a product kernel after bring-up, so TCP retransmits never advance (a new
  OT row).
- **CP numbering:** CP-94 is #199's and CP-95 is #204's. S2's #210 entry is renumbered to CP-96.
- **#208 (L2) is fixed and re-queued, at `12371084`.**
  - F2: `lock::service` now runs at the top of every loop turn. The flood leg doesn't discriminate, which is stated:
    a synchronous send keeps at most one message pending.
  - F3: `poll_control` and `recv_control` serve the lock, and the new `login_lock_stall` was falsified by
    removing `service` from `poll_control`.
  - U2: `arm` requires the activity bit.
  - Residuals stated: the CP-57 send-side stall (L2b), U1 and U3.
- **Batch 19 plan:** #210 (the input-activate fix, first), #211 (OT-94) and #208 (L2).
- **#212 (S3, the NeoFS grow fix), #209 (S1, W1) and #213 (S1, U2/U3) all touch `commit_regular_*`.** Each
  re-stacks onto main after #206 merges, then is gated one at a time.
- **Wired networking gap (S4's trace while working on OT-95): no NIC path delivers to TCP or UDP after bring-up.**
  - rtl8125, boot_eth0 and mt7601u demux only ARP and DHCP, and only inside bring-up loops.
  - `phase4_https_fetch` runs over loopback, and `smoltcp_wrap` is harness-only.
  - The product therefore has a DHCP lease and ARP, nothing more. To be filed as OT-96, the post-bring-up wire RX
    and IPv4/UDP/TCP demux. S4 is designing it, and it's MVP-blocking for "wired networking".
- **#215 (OT-95, `8bc8d0fa`)** adds a `net-tick` kthread, kept out of image A: without OT-96 a ticking lease would
  expire with no renewal. It's in CI's product-real-lm arm and goes into batch 19 or 20.
- **#216, L2b, the kernel lock route (S2, `bf92cd59`, stacked on #208).** Super+L sets a locked bit in kernel route
  state lock-free, and every route read goes to the WM's lock focus.
  - `login_lock_park` is falsified without `sak-lock-route`.
  - Pre-merge cross-family AAG: relay #160, Jarvis, GPT-6.1-Sol high (`Plans/DISPATCH-2026-10-29-l2b-aag-jarvis.md`).
    Muse covers input isolation, relay #161.
  - It gates after #208 merges and it's rebased.
- **Batch 18 merged (about 9:40 am EDT on 10/29).** Run `40867249786` was green at `bcab0517`, and main's code trees
  equal it.
  - #206 `78991d10`, NeoFS supersede-free (product), with AAG C1/U1;
  - #205 `3bcc2f99`, C2.2b obj-quota;
  - #207 `d422dfa0`, the threads_tls step-6 retry.
  - The scheduled check never fired (its trigger stayed pending past 13:05 UTC), so the PM merged by hand.
  - Relays: Muse #162–#163 (`Plans/DISPATCH-2026-10-29-review-bundle-b18-muse.md`); Chad #164–#166, GPT-6.1-Sol high
    (`Plans/DISPATCH-2026-10-29-review-bundle-b18-chad.md`).
- **Batch 19 gating:** run `40883250031` at `2a313a39` = main `d422dfa0` + #210, #211, #214 and #208. #214 was
  union-resolved against the new scenario wiring.
- **J.c.'s decisions (about 10:00 am EDT on 10/29): yes to every PM recommendation.**
  - #191 D1–D5:
    - one app in front;
    - the Spine holds Arthur, Terminal, Files and Settings, with Files replacing the Lens;
    - network and display are read-only in the MVP;
    - no file dates in the MVP;
    - delete asks once, with no trash.
  - #192 E-D1–E-D5: plain text, Inter, new files via Files, a 4 MiB cap, no autosave.
  - Login goes on in the product once Muse's L1 review (#152) passes.
  - C2.3b: vendor the html5ever/selectors closure into the repo.
  - IPv6 in the MVP if it's not much extra work, otherwise later. S4 is scoping it.
  - Still open: where the C2.3b closure's first build may run, since it executes third-party build scripts.
- S1 starts A1 (the WM client table). #191 and #192 merge as docs once their decisions are recorded, and their
  cross-family reads follow the merge.
- **#215 is held: a CP-37 deadlock, found by S4 while building OT-96 R2.**
  - Syscalls take the net spin locks with IF=0 (SFMASK). A kthread taking them IF-on can be preempted while holding
    one, and a syscall on that CPU then spins forever. The pitfall row in #215 had the rule wrong ("only if an ISR
    takes it").
  - Proof: socket_teardown hung 3 of 4 runs, and passed 10/10 with the thread's net work IF-off.
  - The fix is in #215 and OT-96: net-lock sections run IF-off, and the harness does too.
  - S4 audits main for an existing IF-on taker (it would be a live product deadlock). The rule goes into CP-37, and
    an OT row is filed for converting the net locks to IrqSafeMutex.
- **J.c., about 10:15 am EDT:**
  - **D1 changed: side-by-side windows are in the MVP.** Multitasking is a must; "one app in front" was not enough.
    S1 reworks #191 §2.3 and §7 for N visible windows. The layout style (floating with snap halves, the PM's
    recommendation, or tiling) goes to J.c.
  - **C2.3b's first build runs on the CI runners**: vendored closure, a CI build job, and the `parking_lot_core`
    patch. It's queued behind the MVP lanes.
- **#192 merged as `46e83a4c` (docs).** The SHA passed to the merge call had a mistyped tail. GitHub merged anyway,
  and the PM verified the landed file equals the PR head `1e40ea80f12c`. Full SHAs now come from `git rev-parse`
  only.
- **J.c.: the window layout is floating with snap** (move, resize, overlap; drag to the left or right edge snaps to
  that half). Every #191 decision is now settled.
- **#191 merged as `ff145c6e` (docs):** the OT-88 app model, with side-by-side floating windows plus snap. A1 is split
  into A1a (client table, per-client turns and faults, routing, probe app), A1b (N windows composited) and A1c (move,
  resize, snap). Relays #144 (Chad) and #145 (Janus) now point at the merged THINKs on main.
- **#200 merged as `a3c477c1` (docs):** S4's C2.3b closure spike. It lists 28 linked crates and 15 that run only on
  the build host, all permissive or MPL-2.0. J.c.'s calls are vendor the sources and run the first build on the CI
  runners. Two items are carried into C2.3b: the `parking_lot_core` SYS_FUTEX parker, and an AVX/XSAVE check for
  memchr.
- **#209 and #213 bounced by the pre-merge AAG**, both re-verified by the PM.
  - **#209 (security):** `SYS_FILE_REPLACE` copies from the user pointer without `validate_user_buf`. A process with a
    writable NeoFS file could copy up to 4 MiB of kernel memory into it per call. The feature isn't in default or
    product. The THINK's §5.2.1 table had the same gap.
  - **#213:** the read re-check compares against the OpenFile's captured generation, which a writer on the same
    OpenFile refreshes. A read racing a truncate or replace through a shared handle can return reused blocks. The fix
    is to snapshot the generation before the read.
  - Non-blocking: the reservation release misses the NoSpace-after-layout path and cache-write errors.
  - Both return to S1 and gate in batch 21.
- **#212's AAG passed on code.** One test gap: `neofs_grow_write` may pass with the fix reverted, since first-fit can
  reuse blocks freed below the old end. S3 is tightening it.
- **#215 is fixed** (`d9ec27e2`): the net tick and pump run IF-off (CP-37), and its AAG lens is running.
- **Batch 20 = #212 + #215**, pre-merged on the batch 19 tree as `bf47baa1`. It fires when batch 19 merges.
- **#215's AAG found the reverse CP-37, confirmed by the PM.** The drivers/net device table and the identity registry
  are plain spin locks. The new tick takes them IF-off. The USB hotplug thread and `/net/if` reads take them IF-on, and
  so do UDP-send syscalls at IF=0 on main today. A holder preempted IF-on deadlocks the IF-off taker on that CPU.
  The fix is `IrqSafeMutex` for both registries (CP-56) and printing outside the lock. It's back with S4. Batch 20
  takes it if it lands in time; otherwise batch 20 is #212 alone.
- **Batch 19 merged (about 11:10 am EDT on 10/29).** Run `40883250031` was green at `2a313a39`, and main's code trees
  equal it.
  - #210 `ff4bdd51`: input-route ACTIVATE needs the route's own authority (product security, CP-96);
  - #211 `cb5d633a`: OT-94, TCP half-open and accept queues bounded;
  - #214 `1bcd2a1f`: OT-91, exit codes kept only while claimable;
  - #208 `ed56c60d`: OT-66 L2, the lock screen.
  - Relays: Muse #167–#169 (`Plans/DISPATCH-2026-10-29-review-bundle-b19-muse.md`); Chad #170–#172, GPT-6.1-Sol high
    (`Plans/DISPATCH-2026-10-29-review-bundle-b19-chad.md`).
  - The scheduled check again didn't fire, so it was deleted and the merge done by hand.
- **Batch 20 gating at `942f20e0`:** main `ed56c60d` + #212 `c6e9b073` (grow-write, discriminator tightened) + #215
  `0e21ab7f` (net tick IF-off; both net registries IrqSafeMutex).
- **OT-98, a kernel panic when a kill races a natural exit (found by S1 in A1a).** (First written here as OT-97; S4's
  #215 had already filed OT-97 for converting the net locks to IrqSafeMutex.)
  - When a process holding a ProcessHandle kills a target that is mid-exit, `terminate`'s `debug_assert`
    (`process.rs:4832`, "neither Ready nor parked") panics.
  - The product image keeps debug assertions on, so any spawner can trigger it.
  - S5 diagnoses it and fixes it on main, with a looping kill-at-exit discriminator, CP-98, and a cross-family
    pre-merge lens. #204 rewrites the same function, so the two are kept apart.
  - A1a works around it: the WM closes and waits rather than killing.
- **#209 and #213 are fixed** (`215024c4`, `a8645341`). The user-pointer check is in, the copy runs in 64 KiB chunks,
  and reads re-check against a snapshot. They go into batch 21.
- **S2:** #216 rebased (`948979d9`), and product-login is ready (`3a6e86d0`). S2 is next on the NEOSET1 settings
  store (the host crate plus the WM side).
- **S4's audit of IF-on net-lock takers on main:** the only live product cases are the two registries, which #215
  fixes. The bring-up loops run before the scheduler starts, so nothing contends with them. Test-only:
  - `tcp_listen_bounds` gets wrapped in OT-96;
  - `phase4_tcp_connect_stream_close`, `phase4_dhcpv4_ipv4_udp` and the smoltcp_wrap smoke take the net locks IF-on
    and hang if built with wire-rx. No CI kernel combines them, and the OT-96 PR states it.
- **#215's head moved to `061963bf`:** docs only, plus a merge of main. The gate stays on `0e21ab7f`, and the merge is
  checked for code-tree identity.
- **IPv6 data scope (S4, OT-96 design §12 on `claude/ot96-wire-rx` `effa53b0`), reported to J.c. about 11:55 am EDT.**
  - SLAAC, RDNSS, NDP and v6 send are built. Missing:
    - (a) v6 RX validation;
    - (b) answering NS, plus a real DAD probe;
    - (c) ICMPv6 echo, and UDP/TCP RX;
    - (d) off-link egress: no default router is kept;
    - (e) MLD reports.
  - Cost: R6 (a–c) is one host cycle plus one or two gate cycles. R7 (d) is one or two gate cycles with a router peer.
    (e) is of unknown depth on bare metal.
  - S4 and the PM recommend after the MVP, behind `wire-rx-v6`. It stays recorded that way unless J.c. says otherwise.
- **#218, OT-96 R1+R2: wire receive after bring-up (S4, `b2c8fe40`, feature `wire-rx`).**
  - It adds the bounded demux (host KATs, a byte oracle, and a 200k mutation sweep), the `net-rx` thread running
    IF-off, ARP replies, and UDP/TCP/ICMP delivery.
  - With `wire-rx` off, the kernel is byte-identical to main plus #215's registry commit.
  - Pre-merge: a Claude AAG (running), plus the cross-family relay #173 to Chad, GPT-6.1-Sol high
    (`Plans/DISPATCH-2026-10-29-ot96-aag-chad.md`). It gates after both.
  - Post-merge: Muse on R1's parser. R3 (DHCP renewal, then net-tick and wire-rx into image A) follows.
- **#217 (S2, the NEOSET1 settings record, host-only, `e3d64798`)** merges on green push-CI.
- **Muse RETURNs banked (pasted by J.c.):** #162 PASS (`Plans/RETURN-2026-10-29-neofs-frees-review-muse.md`) and #163
  FAIL on item 1 (`Plans/RETURN-2026-10-29-ot61-c2.2b-review-muse.md`).
  - #163: `SYS_BUFFER_MAP` pushes one uncharged, unbounded pin per map into `borrowed_pins`, so a budgeted process
    outgrows its object budget. J.c. verified the chain locally. It's behind `obj-quota`, in no image, and the design
    deferred it (§13). Fix forward: charge or cap the pins.
  - Muse's tally:
    - FAIL: #161 (item 2), #157 (item 3), #167 (item 3), #168 (item 3), #163 (item 1);
    - PASS: #147, #132, #137, #138, #162, #169;
    - partial: #134;
    - blocked by a GitHub rate limit, to retry: #133, #134 items 4–5, and #150–#152, with #152 first.
  - The four other FAILs' RETURN texts are still to be pasted.
- **#218 bounced by its pre-merge AAG**; the DHCP finding was re-verified by the PM. `wire-rx` makes DHCP, RA, ARP and
  NDP reachable from the LAN all the time, and none was hardened:
  - a forged DHCPACK is accepted with no xid, state or server-id check;
  - RA skips the checksum, hop-limit and source checks;
  - attacker-built tables are uncapped, and nothing ticks in a `wire-rx` build;
  - echo replies go to a group source MAC.
  Back to S4.
- **#219, OT-98 kill vs natural exit (S5, `82a7cffe`).**
  - Diagnosis: in the default build every user thread runs on the BSP, so two states panicked: exited but not yet
    reaped, and parked in `sys_wait`. #204 covers only the other-CPU case.
  - Fix: the exit claim moves to the commit point, a `sys_wait` waiter is extracted, and Arcs are dropped per
    CP-30/CP-91. Discriminator `kill_at_exit` with a must-panic control.
  - Pre-merge: a Claude AAG (running) and relay #174 to Janus, GPT-6.1-Sol high, after #149. S1 runs the scenario,
    the control and the end-to-end repro, since S5's seat has no QEMU.
- **Codex RETURNs read and routed (24 from today, banked on main in `0dfc91d7` with 71 older unbanked ones).**
  - **PASS:**
    - #128, #130 (Janus);
    - #135, #139, #159 (Chad);
    - #141, #142, #146 (Jarvis).
  - **Pre-merge FAILs:**
    - **#149 (#204):**
      - F2, sys_wait completes before the survivor leaves the kernel: must fix;
      - F1, an unbounded infallible device wait: as on main, becomes an OT row.
      - #204 moves from S4 to S5.
    - **#160 (#216):**
      - items 6 and 1: input in flight across the chord (forwarding checks before DELIVERY; receive check vs pop);
      - item 7: drop publishes LOCK=0 before clearing ACTIVE.
      - Back to S2, with epoch-stamped records and a fixed order.
  - **Post-merge FAILs, all fixed forward. A revert would reopen the original bugs, or the code is in no image.**
    - **S3:**
      - #148, installer Back starts an install (urgent);
      - #143, the mem-quota leaf frame leaks on a failed map;
      - #163, pins;
      - #202 F1, a TCP listener retiring against in-flight admission.
    - **S5:**
      - #202 F2, wait before sockets close (folded into #204 F2);
      - #140, unsupported sleep-register GAS geometry;
      - #154, HHDM overflow of an accepted reset or sleep GAS, and the judge not checking the duration.
    - **S2:**
      - #127, a grant recipient's last Process Arc dropped under CAP_GRAPH, plus the deferred-drain gap;
      - #153, an immutable tensor write copied before EPERM.
    - **S1:**
      - #144, app-model THINK gaps (blocking reply sends, death-local coverage, a 9-item wait set vs the cap of 8, futex wake exists, stdio kill-switch, the open-with protocol), before A1a goes further;
      - #145 and #155, editor fixes, later.
    - **S4:** #156, a vacuous Condvar check in the servo smoke.
    - **Low (docs and diagnostics):** #129, pointer-window reader snapshot; #131, wording; #136, NVMe spec notes.
- **#219's pre-merge Claude AAG: FAIL.** Two more assertion states: the waiter's target is mid-terminate (FAIL-A), and the waiter is in flight between take and wake (FAIL-B, verified). Back to S5 with #204.
- **Muse's four FAIL texts (#157, #161, #167, #168) are still to be pasted by J.c.**
- **Batch 20 is red: shard 4 failed** (run `40898712909`, job `123540467242`). The log host is blocked from the cloud
  PM, and the visible tail shows `std_*`–`wpa3_*` all PASS, `tcp_listen_bounds` and `usb_hotplug` included. Prime
  suspect: #212's `neofs_grow_write`, to be confirmed. Neither #212 nor #215 is merged.
- **Chad's b18/b19 and Janus's #174 banked** (`31485810`):
  - #164 and #166 PASS; #172 PASS, with its known residual;
  - #165 FAIL: obj-quota coverage, and the handle-table capacity charge;
  - #171 FAIL: an in-flight wait's claim pin, and a failed table leaking claims;
  - #170 FAIL: listener retirement;
  - #173 FAIL on #218: more IF-on takers, an unbounded flush, RA growth;
  - #174 FAIL on #219: the exit-code payload race, the in-flight waiter, a false CP-91 claim.
- **Pausing for J.c.'s account switch.** `Plans/HANDOFF-2026-10-29-pm.md` is the handoff.
- **#217 merged as `6fc4b407`:** the NEOSET1 settings record host crate (S2; host-only, push-CI green). Batch 20
  shard 2 passed, so only shard 4 failed; #212's `neofs_grow_write` is the prime suspect.
- **Muse #152 PASS on OT-66 L1 (#198)**, all 7 items (`Plans/RETURN-2026-10-29-ot66-l1-review-muse.md`). Login in
  the product needs `kernel-rng`; without it, first boot and recovery fail closed. S2's product-login branch
  `3a6e86d0` already adds `kernel-rng` and `sak-lock` with a pairing test, so the PR opens next session and gates in
  a batch. Muse's limit is now spent.
- **Muse #151 PASS** (#197, the K1a follow-ups), with J.c.'s own verification of the key claims at `cbe57444`.
  #133 is running.
- **New PM took over (10/29, about 2 pm EDT), on J.c.'s original account.** The old S1–S5 sessions belong to the
  other account and are not used. J.c. chose **three seats**, started as new sessions:
  - **Seat A**: #204 + #219, kill vs exit (`Plans/DISPATCH-2026-10-29-seat-a-kill-exit.md`);
  - **Seat B**: #216 fixes, then the product-login PR (`Plans/DISPATCH-2026-10-29-seat-b-lock-route-login.md`);
  - **Seat C**: #148 installer Back, then #143, #165 and #202 F1/#170
    (`Plans/DISPATCH-2026-10-29-seat-c-installer-quotas.md`).

  #218 (wire RX, old S4) and #220 (A1a, old S1) wait for a free seat.
- **Batch 20's shard 4 red does not reproduce.** The PM ran all 108 shard 4 scenarios locally at `942f20e0` with
  NEO_SMP=4, as CI runs them: **108/108 PASS**, `neofs_grow_write` and `phase4_ndp_ipv6_neighbor` included (both also
  pass at SMP=1).
  - Local setup: QEMU 8.2 (CI uses 10.2), Limine v8.6.0 in `/opt/limine`, and OVMF from QEMU v10.2.3 `pc-bios`,
    sha256-checked against the Dockerfile pins.
  - #212 is **not** confirmed as the culprit. Shard 4 was re-run on CI (`rerun_failed_jobs` on `40898712909`) for a
    second sample on the real runner. Green means batch 20 merges; red again means the failure is
    runner-specific or intermittent, and the PM bisects by re-gating #212 and #215 separately.
- **Batch 20 merged (about 4:40 pm EDT, 10/29).** Shard 4's rerun on CI was green (run `40898712909`, attempt 2,
  runner `m910q-1`). The first attempt's red was on `ryzen-1` and didn't reproduce locally (108/108), so it is
  recorded as an intermittent or runner-specific red. Its scenario was never seen, because the log host is blocked;
  if shard 4 reddens again on `ryzen-1`, ask J.c. for that job page's `event=FAIL` line.
  - #212 merged as `e3d88fcd` (NeoFS grow-write fix).
  - #215 merged as `46dcda0e` (OT-95 net tick). Its code equals the gate tree up to scenario-registry ordering
    (checked line-for-line; fmt and kernel check pass for both reordered features).
  - #215's OT row and two CP-37 mentions are renumbered **OT-98 → OT-97**, the net locks becoming IrqSafeMutex.
    OT-98 stays kill vs exit (#219).
- **#223 merged as `45bb4b32`** (C2.3a Condvar check). It's test-only; `servo_host_smoke` PASS locally at SMP=4
  with the new wake check, and push-CI was green. Jarvis's narrow re-check of #156 goes in the next relay round.
- **Batch 21 gating at `8a7366c8`:** main + #209 `215024c4` + #213 `a8645341` + #221 `38f8de07`.
  - **A semantic conflict fixed in the gate tree.** #209's `commit_regular_replace` cloned the pre-grow inode record
    without refreshing `fs_block_count`, which is #212's bug on the replace path. The gate adds the same one-line
    refresh, so the fix reaches main with #209. Still owed: a replace-after-grow discriminator. Chad #176 item 1 checks
    every record-rebuild site.
  - **#221's pre-merge AAG (Claude, 5 lenses): no surviving finding.** The PM re-checked the core claim at `pmap.rs:1016–1038`:
    the cap check and the charge come before the push, under `inner` with IF=0, and every refusal returns before any
    state change. Three notes, none blocking:
    - the kill-switch delta is the cap **plus** a lock hoist;
    - unmapped pins keep their buffers' frames until the Pmap drops (the stated deviation; capped at 1024 per Pmap);
    - a map refused late, by a page-table frame shortage, keeps its charge until the Pmap drops (bounded).

    The cross-family read is Chad #177.
- **Relays queued for J.c.:** Chad bundle b20, #175–#177 (`Plans/DISPATCH-2026-10-29-review-bundle-b20-chad.md`), and
  Jarvis #178 (`Plans/DISPATCH-2026-10-29-c23a-recheck-jarvis.md`).
- **#204 + #219 bounced by the pre-merge AAG (Claude); batch 22 cancelled.** Details are in #219's PR comment.
  - **Item 1, re-verified by the PM: a deferred kill can hang every `sys_wait` forever.**
    - The survivor is reaped between `kick_kill_survivors` and the `DYING` insert.
    - `on_thread_reap`'s "natural reap skipped" branch (`process.rs:4040`) never records `last_reaped`.
    - So `finish_kill(TeardownDone)` waits forever on an ordinary cross-CPU kill.
  - **Item 2:** CP-98's ordering claim fails in the reap `pop_front`→`finish_kill` window. The debug assertion is on
    in the product build.
  - **Item 3:** the IPC `close_peer`/send-handoff transit window.
  - **Closed this round:** #174 F1 to F3 and leg C.
  - **Coverage gap:** every scenario kill runs on CPU 0.
  - Back to seat A. Janus's re-checks #179/#180 wait for the fixed heads.
- **Batch 23 gating at `49828888`** (branch `claude/pm-gate-2`): main + #216 `3ea3e987` + #224 `45798eb3` + #225
  `f83a6212` + #226 `c29f6c28`, all merged clean. The Claude AAG is running.
  - Two integration branches are now in use: `claude/pm-handoff-document-ypgky3` and `claude/pm-gate-2`. ci.yml's
    concurrency group cancels a second dispatch on the same branch.
- **Seats, round 2** (`Plans/DISPATCH-2026-10-29-seats-round2.md`):
  - A: the #204/#219 bounce first, then power #140/#154;
  - B: the app-model THINK amendments, then #220;
  - C: #218 wire-rx, the next round.
  - PM decision on #229: keep the backlog-scaled listener weight. It is a fail-safe over-count.
- **Batch 23's AAG (Claude): no blocking finding.**
  - **#216:** items 1, 6, 7 and 10 PASS. "The WM never blocks on a client" fails, but that is the known #144/#172
    residual (blocking reply sends) that A1a's rework owns.
  - **#224:** fails closed without kernel-rng (PASS). **Fail-open skip, PM-verified at `login.rs:118–145`:** sign-in
    is skipped when `.system/` is missing, or when the credential file can't be opened for writing for any reason.
    Muse accepted this in L1, but it now ships in the product. Open to J.c. as a product decision; it doesn't block
    merge, since the product has no login today. Also, `kernel-rng` makes the product image depend on RDRAND at boot.
  - **#225** and **#226** PASS. One note on #226: `reap()` is also called from the NeoFS I/O retry paths, so its drain
    runs wherever those do (this predates #226).
  - Cross-family pre-merge relays: #181 Jarvis (#216), #182 Janus (#226)
    (`Plans/DISPATCH-2026-10-29-b23-rechecks.md`). #224 and #225 merge on a green gate; Muse reviews #224
    post-merge when her limit resets.
- **Batch 21 merged (about 6:50 pm EDT):** #209 as `47cbc537` (SYS_FILE_REPLACE), #213 as `d48cd07e` (refused-commit
  release and read re-check), and #221 as `2db9506f` (the pin charge). Run `40951368071` was green at `8a7366c8`, and
  main's code trees now equal that gate exactly.
  - **Merged in three buildable steps, not via `prep.sh`.** `prep.sh` takes every conflicted file whole from the gate
    tree, so #209's squash would have carried #213's and #221's scenario entries. On its own it failed to build two
    scenario features (`neo-test-neofs-read-recheck`, `neo-test-obj-quota`). Instead the PM built main+#209 (T1),
    +#213 (T2) and +#221 (T3), and checked that each builds: fmt, xtask tests, and kernel check of the touched
    features. T3 equals the gate.
  - **Lesson for stacked or overlapping batches:** `prep.sh` is safe only when each PR's conflicts are registry-only
    and the PRs don't share them. Otherwise build the per-PR trees in merge order.
  - **#213's base was #209's branch** (stacked). It was retargeted to `main` before merging. The first merge attempt
    was refused with "conflicts", which is the signal; compare #84, which merged into its dead stack base.
- **Batch 23 re-gated at `bfb59bab`** on main after batch 21. The run at `49828888` was cancelled, because main's code
  had moved.
- **Batch 24 gating at `477a68f3`:** #227 installer hit boxes, #228 mem-quota leaf frame, #229 obj-quota table slots,
  #230 TCP listener retirement.
  - **AAG (Claude): no blocking item on any of them.** Notes:
    - #228's frame-count discriminator is global, a possible SMP flake;
    - #229's scenario covers only `charge_next` plus a channel install;
    - #230's scenario is one CPU at three fixed points, and its test-only `RACE_CLOSE` is taken at mixed IF states.
  - All four merge on a green gate. Post-merge re-checks: Chad #183 (#227, #229, #230) and Jarvis #184 (#228), in
    `Plans/DISPATCH-2026-10-29-b24-rechecks.md`.
- **J.c. (10/30): #224's sign-in skip stays (option A).** A volume without `.system/`, or one whose record file can't
  be opened for writing, boots unlocked. It is a recorded product decision, not a defect.
- **Muse is unavailable, so Dexter (DeepSeek) stands in for the Claude-code security reviews:** #185 (#210's ACTIVATE
  vs revoke; Muse #167's open FAIL, text never captured) and #186 (#224's product login). See
  `Plans/DISPATCH-2026-10-30-security-review-dexter.md`.
- **#204 + #219's fixed heads** (`22c4a830`, `12bfae54`): Janus #179/#180 now point at them.
- **#204 `22c4a830` + #219 `12bfae54` re-reviewed (Claude AAG): all three bounce items closed.** The PM checked item
  1 in the code.
  - Item 1: the kill now completes on `Process::live_threads`, read under `DYING`; every reap calls `thread_reaped`
    after `Thread::free`.
  - Item 2: the CP-98 debug assertion is replaced by `note_kill_unlocated`.
  - Item 3: `close_peer` wakes under the endpoint guard.
  - Legs E, F and G kill from CPU 1 via the real `SYS_KILL`.
  - Notes to seat A (#219's PR comment): every kill now waits for every reap (an OT-99 residual, to be stated); the
    spawn-failure `thread_reaped` gap is unreachable from userland; the xtask comment is stale.
  - They gate in the next free slot. Merge waits for Janus #179/#180.
- **Dexter's security reviews (DeepSeek, standing in for Muse), banked:** #185 `815ec391`, #186 `2363e889`.
  - **#185 (#210, ACTIVATE vs revoke):** items 1, 2 and 4 PASS. **Item 3 FAILs, confirming Muse #167.**
    - `sys_input_focus_mint_activator_impl` installs the ACTIVATE sibling with parent `None` (`syscall.rs:8476`), so
      revoking the RECEIVE handle never reaches it.
    - Impact is bounded: activation also needs input-system, so there is no key disclosure and no UAF; what's left is
      a stale handle and a pinned object.
    - Fix: pass the RECEIVE cap's id as the parent. → seat A.
  - **#186 (#224, product login): item 4 FAILs, and #224 is held.**
    - The product image doesn't enable `cap-rooted-open` (PM-verified against `PRODUCT_IMAGE_KERNEL_FEATURES`), so
      the root form of SYS_OPEN is open to every process.
    - So any product process can read `.system/account.cred.*` (the Argon2id verifier and salt, for an offline attack)
      or overwrite it, sending the next boot to first-boot and wiping the account.
    - Holding it: shipping a credential store any process can read is a new exposure, not an improvement, which is
      the merge-bar exception. → seat B: make the record files reachable only by the WM, either `cap-rooted-open` in
      the product or a narrower kernel rule for `.system/`, with a scenario showing a non-WM open is EPERM.
  - **#186 item 2 (open to J.c.):** the wrong-password backoff counter is per boot, so a reboot gives three more
    guesses.
  - Items 1, 3 and 5 PASS.
  - Dispatch correction: the SAK code is in `kernel/src/atrium/{sak_logic,focus}.rs`, not `kernel/src/input/`.
- **Chad bundle b20/b24 banked** (`codex/review-bundle-b20-b24-chad`):
  - **#176 PASS** (`4be0766d`): grow-write.
  - **#175 FAIL** (`a3518024`, on #215, merged): fix forward. Net-tick isn't in the product image, and no CI kernel
    combines the harness takers with net-tick.
    - Item 1: test harness IF-on takers (`phase4_tcp_connect_stream_close`, `phase4_dhcpv4_ipv4_udp`, real-DHCP) are
      CP-37 hazards only if combined with net-tick;
    - item 2: the registries allocate under the IrqSafeMutex;
    - item 4: no per-tick work bound (TCP full-table snapshots, an unbounded `flush_wire_egress`, ARP/NDP maps);
    - items 5 and 7: wording, since the registry change is unconditional and CP-37's claim is too broad.

    → seat C, folded into #218 (whose F2 already bounds the flush).
  - **#177 FAIL** (`3871f06b`, on #221, merged): fix forward.
    - Item 4: desktop Home → Arthur → Home maps a fresh Arthur buffer into the long-lived WM each launch and never
      unmaps it (`desktop_session.rs:1238–1261`).
    - About 1024 launches in one session hit `MAX_BORROWED_PINS`; `arthur_session` returns 15 and the WM loop exits.

    → seat A: a release-on-unmap or reuse policy, plus a repeated-launch leg.
  - **#183 pending** (`d090d239`): #227, #229 and #230 aren't merged yet. It re-runs after merge.
- **J.c. (10/30): the wrong-password failure count must survive reboots** (Dexter #186 item 2). It is now a #224
  requirement for seat B: the count and backoff state are persisted with the credential record, so a reboot gives no
  fresh guesses, and a scenario must show the backoff still applies after a reboot.
- **Jarvis bundle b23/b24 banked** (`codex/review-bundle-b23-b24-jarvis`):
  - **#178 CLOSED** (`9450e5d6`): #223's Condvar check.
  - **#181 CLOSED** (`c6255835`): #216's lock route (#160 items 1, 6, 7 and 10). **#216's cross-family pre-merge
    requirement is met.**
  - **#184** (`ea383966`): the cleanup holds at #228's head. The post-merge check waits for #228 to merge.
  - Jarvis ran both QEMU baselines at 4 CPUs, and every negative control failed as intended.
- **Janus bundle banked** (`codex/review-bundle-kill-k1a0-janus`): #179 `001e21de`, #180 `1004e55d`, #182 `f11dc7b2`.
  Every focused scenario passed, and all 12 controls failed as intended.
  - **#179 (#204):** F2 and PM item 1 closed; F1 is tracked as OT-99. PM items 2 and 3 are handled by the stacked #219.
  - **#180 (#219):** F1, both F2 schedules, F3, leg C and all three PM bounce mechanisms are closed. **Item 7 is
    open, and the PM verified it:**
    - `release_scan_pin` pushes the last Arc onto `DEFERRED_PROCESS_DROPS` with no recurring consumer
      (`process.rs:2659`);
    - so a lost kill race against a natural exit keeps one Process and Pmap until a later teardown drains the list.
  - **#182 (#226):** the pinned lookups and the flag protocol are closed. Still open:
    - item 2: the BSP's recurring drain runs from `sched::reap` inside the NeoFS retry paths, while `entries` and the
      cache `drain` guard are held;
    - item 4: combined with #219, `release_scan_pin` doesn't set `DEFERRED_PENDING`.
  - **PM decision:**
    - **#204 and #219 merge on their gate.** They fix product-reachable kernel panics, and item 7 is a bounded
      retention (one Process per lost race), not a crash.
    - **#226 is bounced to seat B.** (a) Drain only from a context that holds no other lock: the reaper's top level,
      not reaps nested inside NeoFS. (b) Add one enqueue function that every producer uses (CapGraph's, and
      `release_scan_pin` once #219 is on main) and that always publishes `DEFERRED_PENDING`. That closes #180 item 7
      and #182 item 4 together. Janus re-checks.
- **Batch 23 green** (run `40968561356` at `bfb59bab`). **#216 merged as `60108a43`** (the L2b lock route; Jarvis #181
  CLOSED) and **#225 as `bfe63d54`** (tensor EPERM first).
  - #224 is held (the credential files, Dexter #186) and #226 is bounced (Janus #182), so main took a subset of the
    gated tree.
  - The PM built main+#216 and main+#216+#225 and checked each: fmt, xtask, and kernel check. The subset is gated
    exactly by batch 22's re-fire, which runs on that main.
- **Batch 22 re-fired at `b55ba447`** (`claude/pm-gate-2`): main + #204 `22c4a830` + #219 `12bfae54`. The only
  conflict was docs: the CP-97 and CP-98 rows, both kept. Both PRs merge on green, with #219 item 7 fixed forward in
  #226.
- **Batch 24 merged** (run `40969203654` green at `477a68f3`):
  - #227 as `df55d40b` (the installer hit boxes);
  - #228 as `0769274c` (mem-quota leaf frame);
  - #229 as `6d01cb1d` (obj-quota table slots);
  - #230 as `af164506` (TCP listener retirement).

  The gate was built on main `69588438`, before #216 and #225. The PM rebuilt the four in order on current main
  (T1..T4). Each passed fmt and a kernel check of default, obj-quota, mem-quota and their scenarios, and T4's
  difference from the gate is exactly main's new code. Main equals T4. Post-merge re-checks: Chad #183, Jarvis #184.
- **Disk note:** the cloud PM's disk filled mid-check (`wt-b20/target`, about 26 GB). The PM freed it and redid the
  checks in throwaway worktrees. Delete `target/neo-test`, `iso-root` and the incremental dirs between batches.
- **Batch 25 gating at `c61f3594`:** main + #224 `f82d2b3d` (login, now with the `system-store` rule and a persisted
  NEOFAIL1 counter) + #233 `2fec3024` (OT-90 power refusals) + #234 `236a8f90` (ACTIVATE is a child of RECEIVE) +
  #235 `a89fa657` (the WM unmaps each Arthur buffer). The Claude AAG is running. Dexter's #187 re-check of #224 is
  queued in the morning file.
- **#226 is sequenced after #219.** Seat B's `defer_process_drop` (the one enqueue) is in; #219's `release_scan_pin`
  must call it once #219 is on main.
- **Batch 25's AAG (Claude): no blocking finding on #224, #233, #234 or #235.** Notes:
  - **#224:**
    - Without `cap-rooted-open`, any product process can unmount `neofs` (when no handles are open) and mount
      another writable NeoFS volume carrying its own `.system/` record. This is outside the path rule. Dexter #187
      gets it, and the likely fix is K1b, or `cap-rooted-open` in the product.
    - The lock's `show()` zeroes `wait_until`.
    - Files opened under SystemStore aren't its children.
    - `login-attempts-per-boot-control` is built but no scenario runs it.
  - **#233:** the reset register's width and offset aren't checked (ACPICA also forces 8 bits), and the judge checks
    the printed constant, not the time actually waited.
  - **#234:** the parent-gone install path drops the handle under CAP_GRAPH (a new CAP_GRAPH→DELIVERY nesting,
    not a deadlock). Return it before dropping. → seat A follow-up.
  - **#235:** the obj-quota release isn't asserted, and the lock-watch table lacks site 7.
  - #233, #234 and #235 merge on green. #224 waits for Dexter #187.
- **Batch 22 merged** (run `40989309159` green at `f0d72417`; main's code equals the gate):
  - **#204 as `f2e556d9`** (OT-93);
  - **#219 as `ad1150f3`** (OT-98, kill vs natural exit). The product-reachable kill panics are closed.

  The cross-family lenses were Janus #179 and #180. #180 item 7, the deferred-drop retention, is fixed forward by
  #226's `defer_process_drop`. Seat B now converts `release_scan_pin`, and Janus #188 re-checks (morning file).
  #236 (seat A's follow-ups, stacked on #219) needs a rebase onto main.
- **Batch 25 merged:** #233 as `4a0be570` (OT-90 power), #234 as `78303c73` (ACTIVATE is a child of RECEIVE), and
  #235 as `8ff1aa14` (the WM unmaps Arthur buffers). They were built in order on main after #204 and #219 merged; each
  step builds. #224 is held for Dexter #187.
- **Batch 26 gating at `0589c0ba`:** #226 `ac7dedd4` (K1a-0 with one enqueue, `release_scan_pin` converted, and a
  top-level-only drain), #236 `894ce31e` (OT-93/98 follow-ups), #237 `aa6ee599` (install_cap drops outside
  CAP_GRAPH), and #238 `ba880cf3` (#171 exit claims). Cargo.toml was resolved by hand. Claude AAG running; Janus
  #188/#190/#191 are pre-merge (morning file).
- **Batch 27 gating at `a2ef4197`:** #218 `b7e3d53e` (OT-96 wire RX round 2 plus #175's items; test_harness took
  #218's re-indented copy of #230's leg 5) and #222 `e48f9a53` (settings WM). Claude AAG running; Chad #189/#192 are
  pre-merge (morning file).
- **#231 merged as `e3167841`** (docs: the OT-88 and OT-67 THINK corrections).
- **Batch 26's AAG (Claude): two bounces, both PM-verified.**
  - **#238:** `sys_wait_impl` holds `_claim` across the park in `wait_for_exit_abi` (`syscall.rs:7708`). A waiter
    killed while parked is zombified raw, so the claim's `Drop` never runs, and each killed waiter leaks one
    exit-code entry (CP-30; #219's rule is that the waiter parks owning nothing). → seat A.
  - **#226:** `reap` still drains, through `finish_kill`'s early return (`process.rs:2364`) and `on_thread_reap`
    (`:4382`). So a NeoFS-nested reap that pops a user zombie drains under `entries` and `drain`. → seat B.
  - **#236 and #237 pass.** Both are SMP-core (kill-path follow-ups, and CAP_GRAPH lock order), so they wait for
    Janus (#190, #193) before merge.
  - Nothing from batch 26 merges overnight. Its gate result still counts as evidence for #236 and #237.
- **Batch 27's AAG (Claude): two bounces, both PM-verified. Batch 27 cancelled.**
  - **#218:** `identity::snapshot_links` (`identity.rs:240`, f180f3ef, the #175 item 2 fix) can spin forever with
    IF off in the product build. `reserve_exact` reserves relative to `len()`, which is 0, so a registration racing
    the snapshot never gets reserved for. → seat C, with a host KAT.
    - Notes: the kill-switch wording, `OUTBOUND` uncapped, and `assert_if_off` covering only virtio and wlan0.
  - **#222 (security):** any client can send OP_SETTING with the idle-lock key and value 0 and turn off auto-lock,
    persistently. The WM checks only the client's own surface. → seat B: WM-only keys, a settings-app-only
    OP_SETTING, and a rate limit.
- **Batch 26 green** (run `41007700229` at `0589c0ba`), but the old heads of #226 and #238 were bounced. The fixed
  heads posted overnight (#226 `6cb63ca2`, #238 `fa152231`, #218 `fe02d28f`, #222 `e25734bd`).
- **Batch 28 gating at `805fd9f1`:** main + #226 `6cb63ca2` + #236 `894ce31e` + #237 `aa6ee599` + #238 `fa152231`.
  Cargo.toml was resolved by hand, keeping all three controls.
- **Batch 29 gating at `b68a95e8`:** main + #218 `fe02d28f` + #222 `e25734bd`.
- A Claude re-check of the four fixes is running. The Janus and Chad pre-merge bundles in the morning file now carry
  the fixed heads.
- **Re-check of the four overnight fixes (Claude):**
  - **#238 CLOSED:** the claim is dropped before any park, and the code is delivered through `waiters.completion`.
  - **#218 CLOSED:** `snapshot_sized` uses a fresh `with_capacity` each pass; the host KAT fails on the old body; no
    duplicate of #230.
  - **#222 CLOSED:** keys are classified exhaustively, the idle lock is WM-only, authority comes from the module the WM
    itself spawned, OP_SETTING is refused while locked, and there is a 4-token rate limit.
  - **#226: one path still open, PM-verified, browser-runtime only.** `on_thread_reap_threaded`'s plain
    `drop(proc_arc)` (`process.rs:4454`) can be the last reference inside a nested reap under a cross-CPU kill. →
    seat B: `release_last_process`. Batch 28's gate covers the rest of #226; the one-line follow-up gets a re-gate.
- **Batch 28 green** (run `41016969570` at `805fd9f1`): #226 `6cb63ca2`, #236, #237 and #238 `fa152231`.
  - #226 has since moved to `3e8e3239`: the one-line `release_last_process` follow-up plus a merge of main. That
    code change needs its own gate, which it gets in the next batch.
  - #236, #237 and #238 are gate-green on their current heads. All four wait for Janus (#188, #190, #191, #193)
    before merge.
- **#222 rebased to `ac23143a`.** Its code tree equals the gated `e25734bd` merged onto main (merge-tree checked, with
  main docs-only since), so batch 29's result will carry over.
- **Batch 29 green** (run `41017454294` at `b68a95e8`): #218 `fe02d28f` and #222 (`e25734bd`, code-identical to its
  rebased `ac23143a`). Both wait for Chad #189/#192 before merge.
- **Batch 30 gating at `94be7fb1`** (`claude/pm-gate-2`): batch 28 plus #226's one-line follow-up (`3e8e3239`).
- **Batch 30 green** (run `41032724024` at `94be7fb1`): #226 `3e8e3239`, #236 `894ce31e`, #237 `aa6ee599`, #238
  `fa152231`. All four are gate-green on their current heads and wait only for Janus's pre-merge bundle (#188, #190,
  #191, #193).
- **State for J.c.'s morning:** every open lane is gate-green and waiting on a cross-family review.
  - Janus: #226, #236, #237, #238.
  - Chad: #218, #222.
  - Dexter: #224.
  - The post-merge rechecks are Chad #183 and Jarvis #184.
  - All of it is in `Plans/DISPATCH-2026-10-30-morning-relays.md`.
- **Chad #183 banked** (`9e58a944`; post-merge re-check of batch 24): #227 CLOSED. Still open, fix forward → seat C:
  - #229.1a (P2): the first table allocation is undercharged. SLOT counts 3 entries, `charge_for` charges 1.
  - #229.3: the scenario doesn't cover the minted or excluding installs, padding, or the backlog charge.
  - #230.3: test-only `RACE_CLOSE` is a plain spin lock taken at mixed IF states in a build that also has net-tick
    (CP-37).
- **Dexter #187 banked** (`2b6cab70`; the #224 re-check):
  - **Item 4 (the `system-store` rule) CLOSED. Item 2 (persisted failure count) CLOSED.**
  - **3a, mount (STILL OPEN, pre-existing on main, not introduced by #224):**
    - Any product process can `SYS_OPEN("/", MOUNT)` then `SYS_UMOUNT(neofs)` whenever no handles are open, which
      locks the user out (fail closed: a DoS, not a bypass).
    - Mounting a foreign volume carrying its own record is latent: there is no USB mass-storage driver yet.
    - Dexter's fix: a non-delegable mount authority (shaped like `system-store`), or masking `MOUNT_RESERVED` out of a
      plain root open. Interim: the WM holds a session-long `/neofs` handle, so the unmount is EBUSY.
    - **Open to J.c.**, since it changes what clients may do.
  - **3b:** the lock's `show()` zeroes `wait_until` (`lock.rs:301`); a one-line fix to seat B.
  - **G1 (fail-safe):** a failed counter reset leaves the higher count.
  - **Qualifiers:**
    - (A) the `/.system` of the root mount itself isn't matched;
    - (B) `login_record_guard` doesn't probe the path tricks or `SYS_MODEL_LOAD`/`SYS_FILE_REPLACE`;
    - `sak-lock-route` first ships in the product with #224, which the merge report names.
  - **#224 merges after a re-gate on current main**: its last gate was batch 25, and main has moved since.
- **Jarvis #184 banked** (`be29cb5c`): every #143 item is CLOSED on the merged #228. The four-CPU baseline passes,
  and the negative control catches 40 leaked frames.
- **Batch 31 gating at `4856ee64`:** main + #224 `f82d2b3d`, the re-gate on current main. #224 merges on green.
- **J.c. (10/30): Dexter #187 item 3a gets option A**, a non-delegable mount authority shaped like `system-store`.
  Dispatched to seat A (`Plans/DISPATCH-2026-10-30-mount-authority.md`); Dexter does the security re-check.
- **Chad bundle b27 banked** (`codex/b27-recheck-chad`):
  - **#189 (#218, `0242855c`):** #173 F1 and F3 are closed, and virtio's used-index loop is bounded. **Still open
    (P1):** the whole-call work bound. `flush_wire_egress_some` and `pump_loopback_egress_some` send at most 16/128
    segments, but re-append the unprocessed snapshot and the concurrent arrivals (`kept.extend(taken)`,
    `queue.extend(newer)`), which is work proportional to the queue, under OUTBOUND with IF off. `OUTBOUND` has no
    cap. Also: registration still allocates under the registries, and the design/periodic docs claim bounded work.
    - **PM decision: #218 merges.** The gap already exists on main (#215's flush drains everything), wire-rx and
      net-tick are both off in the product image, and #218 bounds the send count. → seat C fixes it forward: a
      whole-wrapper bound (no full re-append; e.g. a VecDeque split, or a cursor), a cap on OUTBOUND, corrected docs.
      Chad re-checks.
  - **#192 (#222, `5a710a51`):** functional checks pass. Notes: (5) "off is bit-identical" holds for the functional
    and `.text` scope only. #222 relies on #224's `.system/` guard: the settings record is
    `/neofs/.system/settings.{a,b}`. **#222 merges after #224, gated with it.**
- **Janus bundle b26 banked** (`codex/b26-recheck-janus`):
  - #190 (#236) CLOSED (`f7fee2fa`); #191 (#238) CLOSED (`c10cf409`); #193 (#237) CLOSED (`0cbdbd5c`).
  - **#188 (#226, `730ce4e5`): the separate drain and the single enqueue are CLOSED.** Two pre-existing paths still
    let a last Process Arc drop in a nested reap with IF off:
    - (2) `on_thread_reap`'s forced/finished-exit early return (`process.rs:4353`) drops its clone implicitly;
    - (3) `ProcessAnchor`'s Arc field drops after `unanchor_thread` has published count 0 (`thread.rs:625/644`).
  - Also: the stock pending control fails at F1c, not F1d (an attribution note), and F1f's `left != 0` check is weak.
- **PM decision: #226 merges.** Both paths are on main already (the anchor came with #204), and #226 strictly
  improves it. Fix forward → seat B, then Janus re-checks.
- **Batch 30 merged** (main's code equals gate `94be7fb1`), each step built in order:
  - #226 as `3d61a1f5`;
  - #236 as `09d4e1b2`;
  - #237 as `657f1354`;
  - #238 as `648ae9c8`.

  #237's Cargo.toml was resolved so its own squash carries only its own feature lines.
- **#224 merged as `8bd7fc14`** (batch 31 green at `4856ee64`; rebuilt on main after batch 30 merged, and it builds).
  This is the first product-image ship of `kernel-rng`, `sak-lock`, the WM's login, `system-store` and `sak-lock-route`.
- **Batch 32 gating at `6ccde4de`:** main + #218 `fe02d28f` + #222 `ac23143a`. The xtask scenario lists were resolved
  by hand to one timeout arm for the six sign-in boots. This run also gates main's batch 30 + #224 combination.
  Both merge on green; #218's whole-call work bound is fixed forward (seat C).
- **J.c. asked for a playbook gap map.** Written as `docs/THINK-2026-10-30-playbook-gap-map.md`: seven principles
  against the inventory at `46fa59fd`, seven lanes (L1 turn on what's built … L7 subtraction as a gate), and four
  decisions open to J.c. The inventory also found `docs/OPEN-THREADS.md` stale: OT-65/66/67 marked "zero code", no
  OT-87/88/90 rows. That goes in a consolidation pass.
- **J.c. (10/30): lane order L1 → L2 → L7, then L3 and L4.** Seats A, B and C take them after their current queues
  (`Plans/DISPATCH-2026-10-30-lanes-L1-L2-L7.md`).
- **J.c.: one coder on the browser, nonstop, until it ships.** A fourth session, **seat D**, starts on ADR-078's ladder
  from C2.3b (`Plans/DISPATCH-2026-10-30-seat-d-browser.md`). "Done" means a real site renders on the Zenbook from
  image A, as a removable, permissioned app, with A2 enforced.
- **Hands-off Codex relays (10/30).**
  - J.c. set up Remote Control on his PC (WSL plus Windows Codex 0.165.0) and started a relay-runner session there.
  - This PM session can't start or schedule sessions on his PC: the trust approval doesn't register in this session,
    because its log is too long.
  - The fix doesn't need cross-session messages: `scripts/relay-runner.sh` polls `Plans/RELAY-QUEUE.md` on main every
    20 minutes and runs each new relay with `codex exec` in the coder's folder. The PM reads the RETURN branches from
    GitHub.
  - A fresh PM session can still make the trust request, if direct messaging is ever wanted.
- **The relay runner is live on J.c.'s PC** (pid 16912; polls every 20 minutes; logs in
  `E:\AI Automation\workers\.relay-runner\`).
  - Model slugs were checked against the PC's Codex: Daybreak Blue is `gpt-daybreak-blue-latest`, and unlisted names
    are lowercased with spaces turned into dashes. Both are fixed in the repo copy too.
  - **Trust boundary:** any push to main can make Codex run on that PC, with write access to the worker folder and
    network on. Main is J.c.'s repo, pushed only by his account's sessions. Keep it that way, and never queue a relay
    whose text comes from outside the project.
- **J.c. (10/30): DeepSeek may serve as an extra coding seat.**
  - Dexter gets two bounded, non-time-critical tasks (`Plans/DISPATCH-2026-10-30-dexter-sdk-abi-and-size-budgets.md`):
    L4.4, the SDK-ABI refresh (49 of 111 syscalls documented, plus a drift guard); and L7.2, CI size budgets, moved
    from seat C.
  - Queued as relay 194 for the PC runner (`dsh headless`).
  - The relay runner also gained DeepSeek support (`a57a6a07`); J.c. restarts the PC copy once.
- **Batch 32 merged** (run `41120593830` green at `6ccde4de`): **#218 as `0b4cca23`** (OT-96 wire receive) and **#222
  as `b24f81eb`** (settings in the WM). Main's kernel, userland and tools code equals the gate; the only other change
  is the relay-runner script.
- **Batch 33 gating at `e4c8426e`:** #240 mount authority, #242 the K1a-0 last Arcs, #243 the login follow-ups, #239
  the obj-quota follow-ups. **Batch 34 at `f405fa3c`:** #246 clean shutdown.
  - #247 (K1b plus cap-rooted-open) conflicts with main in neo-init after #222, so it goes back to seat A for a rebase.
  - #241 (the egress bound) was built on #218's pre-merge branch, so it goes back to seat C for a rebase onto main.
  - A PM batching-loop error briefly union-resolved #241's code conflicts. That tree was discarded and never fired.
- **Cross-family reviews queued on the PC runner** (relays 195–199, `Plans/DISPATCH-2026-10-30-b33-b34-reviews.md`):
  Dexter on #240 and #243, Janus on #242, Chad on #239 and #246.
- **Batch 33/34 AAG (Claude): no blocking finding on any of #240, #242, #243, #239 or #246.** Notes:
  - **#240:** the scenario doesn't try deriving, granting or inheriting the authority, or check the plain-open MOUNT
    mask.
  - **#242:** F1g and F1h simulate on one CPU.
  - **#243:** a failed `sys_clock_gettime` would make the lock wait forever (hypothetical); after a show-time wait,
    tick shows WRONG; `.system` matches as the second component of any root path (same breadth as before).
  - **#239:** no net-tick + tcp_listen_bounds composition scenario.
  - **#246:** it's not in image A (`power-clean` is a test entry only). Its bounds rely on per-command driver timeouts
    inside one drain pass, `SINK.lock()` and the worker join are unbounded, and `fsync_regular` isn't gated by the
    freeze. These go to seat C as follow-ups before `power-clean` joins image A with L1.4's UI.
  - All five merge on gate plus cross-family (relays 195–199).
- **Relay runner, first real cycle (10/30 evening):**
  - **DeepSeek is out of balance:** `dsh` returns 402 `ACCOUNT_QUOTA`, so relays 194–196 fail until J.c. tops up.
  - **Two script bugs on the Codex side, now fixed in the repo:**
    - `codex exec` read the queue loop's stdin, so one run swallowed relays 198–199. Codex now gets `</dev/null`.
    - `--approve-for-me` needs Daybreak, which `gpt-6.1-sol` can't use. It's replaced by
      `-s workspace-write -c approval_policy="never"` with network on.
  - Failed relays retry each cycle. The PC copy needs a recopy and restart (J.c.).
- **J.c. (10/30): the local Qwen (Prometheus) helps the browser, within CLAUDE.md's scope** (research, design,
  host-only).
  - The relay runner gained an OpenCode route (`opencode run -m strata/qwen3.8-flash-next-iq3_s`).
  - Relays 200–202 are queued (`Plans/DISPATCH-2026-10-30-prometheus-browser-prep.md`): the Stylo/layout closure, the
    SpiderMonkey port study, and HTML/CSS fixtures.
  - Qwen is also available as a fourth-family reviewer.
- **Relay runner: three engines in parallel** on J.c.'s PC (`ONLY=codex` pid 37466, `ONLY=deepseek` 37467,
  `ONLY=qwen` 37469), with a per-coder folder lock (`0e1ce91d`).
- **J.c. merged the PC copy's three fixes himself (option 1), as `e6cc61cc`.** The PM's merge was refused by the
  safety check as "creating an unsafe agent", so J.c. made the call explicitly. The fixes:
  - `-c daybreak=false` for non-Daybreak models;
  - **`-s danger-full-access` for Codex relays**, because the workspace sandbox made `.git` read-only and hid the
    GitHub login. This is J.c.'s explicit decision;
  - a relay counts as done only when its reply names a commit SHA.

  **Trust boundary:** only what's pushed to main's `Plans/RELAY-QUEUE.md` starts a job. Never queue relay text that
  comes from outside the project. A narrower sandbox (option 2: let `.git` and the Git login through) stays open as a
  later hardening.
- **J.c. (10/30): Sage is folded into Dexter.** Same DeepSeek model and profile, so there's no separate seat. Sage-shaped
  diagnosis jobs (one hard question; a hypothesis plus a falsification test) go to Dexter. CLAUDE.md is updated.
- **logansos.com is live (J.c., 10/30).**
  - J.c. bought the domain on Cloudflare, and the landing page (`site/`, `8d0b366b`) is deployed on Cloudflare Pages
    (project `logansos`).
  - It uses the Atrium design and the real emblem: the ring turns round the still star as in Arthur's thinking, built
    from `tools/branding/generate.py`'s layer split.
  - Future site changes: edit `site/` on main. With the Pages repo connection (watch path `site/*`), they deploy
    automatically.
- **Codex reviews from the PC runner, banked:**
  - Janus #197 (`k1a0-recheck3`): PASS.
  - Chad #198 (`b24-recheck2`): PASS. One coverage gap: the refusal leg doesn't observe pending state.
  - Chad #199 (`ot90-p3a-review`): items 1, 3 and 4 PASS. **Item 2 FAILS: a stuck block request can hang shutdown
    forever** (the K2 deadline is checked only between drain passes).
- **Merged:**
  - #242 as `eb9991a8` (batch 33 green);
  - #239 as `5f41d146`;
  - #246 as `4d714c1a` (batch 34 green). `power-clean` is off in image A.

  Each was built in order on current main.
- **Fix forward, to seat C:**
  - #246 item 2, plus the AAG's unbounded `SINK.lock`, worker join and `fsync_regular`. These must land **before #258
    (the power menu) ships shutdown in image A**.
  - #239's coverage gap.
- **Still waiting:** #240 and #243 on Dexter #195/#196 (DeepSeek).
- **A PM merge-tool error, now fixed.**
  - The PM's per-PR merge step built each tree from main at one moment, then committed it on a newer main. So
    whatever main gained in between was reverted.
  - #242's squash (`eb9991a8`) reverted J.c.'s site edits (`a4773ea9`, the footer inquiry link). J.c.'s session
    restored them (`3768acec`).
  - #239 and #246 were checked and carry only their own files.
  - The tool now re-merges each tree onto the freshest `origin/main` (`git merge-tree`) and aborts on any conflict.
  - J.c.'s rule: a branch that touches `site/` is rebased on main before merging, or leaves `site/` alone.
- **Batch 35 gating at `924281ea`:** #245, OT-61 C2.3b, the browser's vendored HTML/CSS closure. **Batch 36 at
  `c2e379bc`:** #248 Arthur's permission prompt, #253 the action log, #259 SDK-ABI, #260 size budgets.
  - The Claude AAG is running.
  - Cross-family relays 203–206 are queued: Dexter on #248 and #253, Chad on #245 and on #259/#260.

### 2026-10-31 ~00:00 EDT: batch 35/36 Claude AAG results, and bounces

- **#245 (OT-61 C2.3b vendored html5ever):** no FAIL. Notes: no CI check of the vendored checksums; the rustfmt use
  in `string_cache_codegen` is undocumented; CI caching. Waiting on Chad 205.
- **#248 (Arthur behind the WM prompt):** no FAIL. Three follow-ups to seat B: Enter typeahead answering Allow, an
  Always grant lent while locked, early-error returns skipping `permission::end`. Waiting on Dexter 203.
- **#253 (NAL1 action log): BOUNCED to seat B, two FAILs re-checked in code.** Chain repair does one `sys_write` of
  the whole prefix, over `MAX_FILE_IO` 4096 for any log past 4 KiB, so a torn tail sticks the log for good. An
  Always grant is recorded before `derive()` but noted only after it succeeds.
- **#259 (SDK-ABI doc):** drift guard PASS; three doc errors (THREAD_JOIN return, GETRANDOM and FUTEX_WAIT errors)
  queued to Dexter as relay 207.
- **#260 (CI size budgets):** no FAIL. Relay 208: rebase and re-measure (about 990 code lines landed), fail loudly
  on a missing download.
- Dispatch: `Plans/DISPATCH-2026-10-31-b36-bounces.md`. Every rebased branch is checked for no `site/` path (J.c.'s
  rule after the #242 revert).

### 2026-10-31 ~00:00 EDT: Dexter 195/196 PASS; batch 36 replaced by batch 37

- **Dexter 195 (#240 mount authority, `6f2ef641`):** PASS on all four items. Notes: the MOUNT mask is not
  independently witnessed (add a `SYS_HANDLE_INFO` probe); a MOUNT-only open is now EINVAL.
- **Dexter 196 (#243 login follow-ups, `7947a189`):** 3b, G1 and qualifiers A and B all CLOSED. Follow-ups, not
  blockers: no scenario reaches the lock's carried delay (3b unwitnessed); the expiry note says "wrong password" for
  a carried delay; `.system` is still a legal mount name.
- **Batch 36 cancelled**, since #253 is bounced and #259/#260 are being rebased. **Batch 37** (`claude/pm-gate-2` at
  `513d9c84`) = main + #240 + #243 + #248. #240/#243 are re-gated because #246 changed `process.rs` and
  `fs/mod.rs` after batch 33. All clean merges, fmt and the kernel check pass, and there is no `site/` change.
- Cancelled seat D's own #245 run 41156832843: batch 35 gates the same head on current main.

### 2026-10-31 ~00:45 EDT: Dexter 203/204/207 and Chad 206

- **Dexter 203 (#248): FAIL, bounced to seat B.** F1, PM-verified at `permission.rs:201` (`8b7511c6`): the path is
  a second, blocking `sys_recv` after the polled header, so a client that withholds it parks the WM and keeps the
  lock screen from engaging or being served. Also F2 (the route restore ignores Super+Escape's escaped state) and a
  missing CP-57/CP-97 row. The grant machinery itself passed: no forged answer, no client write to the record, and
  no write without a prompt.
- **Dexter 204 (#253, reviewed at `75eae4f8`):** items 1-4 PASS (Dexter reproduced the byte oracle with `b3sum`), but
  F1 is inherited and extended (#253 adds a second blocking read for the intent). It stays with seat B with #248.
- **Dexter 207 (#259):** THREAD_JOIN, GETRANDOM and FUTEX_WAIT fixed, every error list re-checked, and rebased on
  main at `6ad6477f`. **Chad 206:** #259 and #260 PASS. #259 is docs plus an xtask test, so it merges on
  fmt and that test.
- **Batch 37 is unchanged:** #248's old head is in it, so it gates #240/#243 only. #248 won't merge from it.

### 2026-10-31 ~01:15 EDT: #259 and #252 merged; batches 38/39 lined up

- **#259 merged as `c5032e55`** (SDK-ABI doc + drift test; Chad 206 PASS, Dexter 207 fixes). Docs plus an xtask
  test, so its gate is fmt, clippy -p xtask and the drift test, all run by the PM on the merge with main (push-CI was
  stuck in the runner queue). **#252 merged as `9be29c37`** (S0ix THINK, one docs file).
- **Qwen 200 banked:** `docs/RESEARCH-2026-10-30-ot61-stylo-layout-closure.md` (stylo/layout dependency closure).
- **#260** is rebased (`89e74721`) with loud download failures; no re-measured numbers reported, so its CI
  product-build job decides.
- **Batch 38 (ready, waits for a gate slot):** main + #241 + #247 + #250 + #265 + #267. All clean together;
  fmt, the default kernel and the image-A kernel build pass, with no new warnings vs main. **Batch 39:** #254
  (conflicts with batch 38 only in the scenario registries). #220 conflicts with main and needs a rebase.
- Cross-family relays 209-212 queued (`Plans/DISPATCH-2026-10-31-b38-b39-reviews.md`); the Claude AAG is running.

### 2026-10-31 ~01:20 EDT: Claude AAG on batches 38/39, no surviving FAIL

Six PRs (#241, #247, #250, #254, #265, #267), each checked on lenses 1-5, the kill-switch and the prior-art section.
None bounces. Notes kept for the merge reports and follow-ups:
- **#250:** the `write_bytes_at` retry has no bound (a sustained reader at capacity can starve a partial writer;
  a per-block epoch or retry cap would fix it). On a stuck device every mutator can now wait in `drain_all`, and
  under #246's K2 a throttled mutator holding `self.drain` blocks K2's own drain. #267's deadline bounds it, so
  **#250 merges with or after #267** (batch 38 has both).
- **#247:** turns `cap-rooted-open` on for the whole product image, not just Arthur's scope, and no full
  product-image boot is in the gate. `/model 1.5b` becomes EPERM in image A (stated in the PR). A scope is a path
  string, so a remount at `/neofs` re-targets it until #240 lands.
- **#267:** when the deadline fires, un-fsynced data is lost (recovery to the last CoW generation). The 20 s cap is
  total, so a slow drain that keeps progressing now ends unclean. The product's 2000-tick deadline is not exercised.
- **#241:** put-back is uncapped, so two concurrent passes can reach 512+128+16 (one bounded realloc). The PR body's
  claim that the cap bounds each `pump_loopback_egress` round is wrong (it is the syscall path, listed as unbounded).
- **#254:** `validate_user_buf` checks range only (inherited). Userland `rdtsc` assumes synchronized TSCs.
- The panel is all Claude. Cross-family relays 209-212 cover #250 (Janus), #247 (Dexter) and the rest.
- Batch 37 (run 41165605125) is running: shard 3 green; shards 1, 2 and 4 in progress.

### 2026-10-31 ~01:55 EDT: cross-family returns 208-210, 212; batch 38 split; user-copy fault found

- **Chad 209:** #265 PASS. **#267 FAIL** (3b, PM-verified): after `Done`, `power_off` / `restart` print and call
  `log::flush()` with no deadline, so a sink device that never completes a write blocks S5 and restart. **#241
  FAIL on its claims:** the 512 cap is an admission threshold (put-back reaches 528), and the CP-54 "whole call
  bounded" claim is not true as written. The PM treats 1b (an identity snapshot that retries while interfaces keep
  registering) as finite in practice, so it is a claim fix, not a hang.
- **Janus 210 (#250): FAIL** on the kill-switch (`forget_range` bumps the epoch in off mode, PM-verified at
  `cache.rs:358-366`) and on the CP-64 bound (one transaction can dirty more than capacity). Janus 3 (a mutation
  waits on a device that never completes) is inherited driver behaviour, so it is a note.
- **Jarvis 212 (#254): FAIL.** "From power-on" is not established (the fixture prints model and desktop before
  firmware exit), and an unrecorded firmware time prints as 0.00 s.
- **Jarvis 2 points at a kernel-wide defect (PM-verified on main):** `validate_user_buf` checks range only, and a
  kernel-mode page fault on a user address goes to `halt_forever()`. **Any process can stop the machine by passing
  an unmapped buffer to a syscall.** New lane for seat A after #247: a fault-tolerant user copy (exception-table
  style) for every syscall copy, with an EFAULT scenario and its control.
- **Dexter 208 (#260):** rebased, re-measured, loud downloads; banked.
- **Batch 38 is now #247 + #265** (fires when a gate slot frees; #247 still waits on Dexter 211). #241, #250, #254
  and #267 go back to seat C (`Plans/DISPATCH-2026-10-31-b38-bounces-and-user-copy-fault.md`). #258's power menu
  still waits on #267.
- **#248** has seat B's fixes (`6b27864a`: F1 polled path read, F2 route restore, the notes); it needs a re-gate
  and Dexter's re-check of F1.

### 2026-10-31 ~02:30 EDT: Dexter 211 (#247) bounces on claims; batch 38 = #265 + #248

- **Dexter 211 (#247):** no escape from a scoped cap and no widening (items 1, 2 and 4 PASS). **F1:** the Lens client
  holds a `/` browse cap (LOOKUP|READDIR), contrary to THINK line 90. **Item 5 FAIL:** the survey is login prior
  art, not path scoping. Back to seat A, ahead of the user-copy lane.
- Batch 37's four QEMU shards are green. Three light feature-matrix jobs are still queued behind batch 35, so
  #240/#243 merge when the run completes.
- **Batch 38** fires on a third gate branch (`claude/pm-gate-3`) so it does not cancel batch 37's last jobs: main +
  #265 + #248 at `6b27864a`. Only two QEMU gates are in flight (batch 37's QEMU portion is done).

### 2026-10-31 ~03:00 EDT: Dexter 213 (#248) PASS on code; Chad 205 re-queued to Janus

- **Dexter 213:** #248's F1 (a polled, bounded path read with the lock served; both stall legs fail the old code)
  and F2 (the route is restored through `swap_to`) are fixed, and the identity and unsaved-Always notes are in code.
  One documentation FAIL: the PR body's CP-57/CP-97 rows overstate the guard for the synchronous-send half. Seat B
  corrects the PR text; no code bounce. Test gaps to carry on #253: the intent-stall leg, record-full Always,
  identity mismatch, and an escape pressed during the path wait. #248 merges on a green batch 38.
- **Chad 205 (#245) never came back.** Re-queued as relay 214 to Janus (same section).
- Batch 37: everything green except one light job (preemptible-liveness), still queued behind batch 35.

### 2026-10-31 ~03:30 EDT: #240 and #243 merged (batch 37 green)

- **Batch 37** (run 41165605125, `513d9c84`): 22/22 jobs green, all four QEMU shards.
- **#240 merged as `08b0b4a6`** (mount authority; Dexter 195 PASS). **#243 merged as `cd421ebb`** (login follow-ups;
  Dexter 196: all items closed). Each squash carries only its own files (13 and 5), with no `site/` path.
- Main's code vs the gate: the only differences are #248's files (gated, not merged; it re-gates in batch 38) and
  #259's xtask drift test. Checked on main: fmt, and `cargo test -p xtask` 159 passed.
- Still running: batch 35 (#245; relay 214 review pending) and batch 38 (`claude/pm-gate-3`, #265 + #248).

### 2026-10-31 ~04:40 EDT: batch 35 green; #245 re-gates as batch 39; re-checks 215-218

- **Batch 35** (run 41164840278, `924281ea`): 22/22 green. #245's closure build is green on `e3fe798a`.
- **Janus 214 (#245):** vendored metadata, build-host code, the futex parker and memchr's AVX check all PASS. Two
  FAILs, judged by the PM:
  - The CR4.OSXSAVE clear is in the shared start-up path. This is a disclosed hardening (PR body, "Kill-switch"):
    image A re-sets it through `product` → `inference-preemptible` → `xsave-fpu`, and the kernel's AVX2 matmul is
    gated on OSXSAVE at run time, so other builds fall back to scalar instead of losing YMM state. Not a defect.
  - The closure job's disk is unbounded. Checkout cleans the workspace each run, so it does not accumulate; a disk
    guard is a follow-up for seat D.
- Main changed #245's registry files since batch 35's base, so **batch 39** (`claude/pm-handoff-document-ypgky3`
  at `0ac09e3c`) re-gates main + #245. #245 merges on green (seat D's draft criteria are met).
- Seat C re-pushed #241 `8c7bdf9e`, #250 `572fba15`, #254 `040d0e62` and #267 `cd3385ff`; seat A #247 `cf96e17f`;
  seat B #253 `93cbc705`. Re-checks queued: 215 Chad (#267, #241), 216 Janus (#250), 217 Jarvis (#254), 218 Dexter
  (#247, #253).

### 2026-10-31 ~06:35 EDT: #265 and #248 merged (batch 38 green); batch 40 fired; relay runner quiet

- **Batch 38** (run 41180591532, `5ac6a5e0`): all jobs green. Shard 4 took about 100 minutes on `m910q-1`, which
  is slow but passed.
- **#265 merged as `6ae9acff`**, then **#248 merged as `b8992ff5`**. Main's `kernel`, `userland`, `tools`,
  `Cargo.*` and `.github` trees are now identical to the gate's.
- **Batch 40** (`claude/pm-gate-3` at `493897cd`): main + #267 + #241 + #250 + #254 (seat C's fixed heads). It is
  the batch built earlier, re-applied cleanly onto the new main. fmt, xtask (159) and each new scenario's kernel
  build pass. It merges on green plus re-checks 215-217.
- **Batch 39** (#245) is still running, on main `d9437348` from before #265/#248. Those two touched the same
  registry files, so check the merge at that point.
- **Relay runner:** nothing has come back since Janus 214 at 03:11 EDT. Relays 215-218 were queued about 04:40 EDT.
  It may be stalled on the PC (for J.c. to check: the runner processes and `E:\AI Automation\workers\.relay-runner\`
  logs).

### 2026-10-31 ~07:25 EDT: #245 merged (batch 39 green)

- **Batch 39** (run 41184849656, `0ac09e3c`): 22/22 green, including `servo_closure_smoke` and `_avx`.
- **#245 merged as `f5ceeb4d`** (OT-61 C2.3b, the vendored html5ever + selectors closure). Taken out of draft
  first (seat D's criteria: closure build green, QEMU green). Merged onto main after #265/#248 without a re-gate:
  the overlap is registry-only, and the merge's delta from the gated tree has the same patch-id as main's own
  batch-38-gated delta. The squash is 1288 files (almost all `third_party/ot61-browser/`), with no `site/`.
- Seat D is re-stacking #249/#261/#262/#263/#264 onto main and adding the closure job's disk guard. Seat B is
  rebasing #253 on main with #248 in.
- Batch 40 (#267 #241 #250 #254) is running. The relay runner is still silent since 03:11 EDT.

### 2026-10-31 ~08:20 EDT: batch 41 (#247 + #253) gating; Claude AAG has no surviving FAIL

- **Batch 41** (`claude/pm-gate-2` at `d282d115`): main + #247 `272bc85a` + #253 `23a8d98a`. fmt, the default and
  image-A kernel builds, and xtask (160) pass.
- **AAG #247:** mount authority survived the rebase (only the two `root_dir_cap_for` call sites changed in
  `syscall.rs`; no scope carries MOUNT). `..` is folded once for both check and resolve; the `..` leg is real. The
  path-scoping prior art is now present. **F1 is accepted, not fixed:** the Lens keeps a browse-only `/`
  (LOOKUP|READDIR, no read), and the THINK now says so. **This is J.c.'s call** ("every client" vs a browse-only root).
  Notes: the scope control is not a registered scenario; `neo-shell` still spawns Arthur with all of `/` (the
  desktop product doesn't use it).
- **AAG #253:** the >4 KiB torn-tail repair, the Always-after-failed-derive case, the polled intent read and all four
  test gaps are fixed and tested (host: action-log-core 8/8, atrium-logic-tests 146/146). Note 1, sent to seat B as
  a follow-up: an Always is saved even when the log append that should record it fails (log over 1 MiB + 4 KiB, or
  ENOSPC); undo the record when the note fails. Other notes: a log over CAP+IO blocks every lend until repaired;
  rotations rewrite in place (loss is detected, not silent); a front deletion can pass as a rotation (needs an
  external anchor, deferred by the THINK); the kill-switch off leg is not gated.
- #247 and #253 still need Dexter 218 (relay runner silent since 03:11 EDT).

### 2026-10-31 ~08:55 EDT: batch 40 green (held for re-checks); batch 42 fired

- **Batch 40** (run 41193219896, `493897cd`: #267 #241 #250 #254): 22/22 green. **Held:** the merge needs Chad 215,
  Janus 216 and Jarvis 217, and the relay runner has returned nothing since 03:11 EDT.
- **Batch 41** (#247 + #253) is still running; Dexter 218 is also outstanding.
- **Batch 42** (`claude/pm-handoff-document-ypgky3` at `f77ce08e`): main + #249 (OT-61 C2.4 Stylo closure) + #261
  (C3a neo-crt), seat D's re-stacked heads. fmt, the default kernel build and xtask (160) pass. #266 is held out (it
  conflicts with these in `ot61-closure.yml`). Claude AAG running; Chad's review queued as relay 219.

### 2026-10-31 ~09:05 EDT: AAG on #249/#261; #261 bounced; batch 42 re-fired with #249 alone

- **#261 (OT-61 C3a neo-crt): bounced, merge-blocking (PM-verified).** Its `cxx_runtime_smoke` scenario is
  `ignored: false`, so it joins the 4-shard gate, but the CI container (`Dockerfile`) installs no clang, and the PR's
  own `crt-build` job is red ("clang is present"). This breaks gate hermeticity. Fix: a pinned clang in the
  Dockerfile, or `ignored: true` / a separate job. Also a latent defect: musl's stdout/stderr have `lock = -1` and
  nothing turns locking on, so two threads printing at once can overrun the static stdio buffer (`__fwritex`). libc++
  is built without threads (shared_ptr refcounts not atomic), which needs to be stated as a hard rule. musl v1.2.5 and
  libc++ 18.1.8 were verified byte-identical to upstream, with the licence files kept.
- **#249 (C2.4 Stylo closure): no blocking finding.** All 97 crates' files match their `.cargo-checksum.json`, and
  three were checked against crates.io. Before it leaves draft: VENDORED.md is not updated for the new crates,
  void-1.0.2 has no licence text, the survey count is off (93 vs 97), and stylo's vendored Python wheels are not
  listed. The `stylo-smoke` crate has never been compiled. **For J.c.:** Python in the browser build (stylo's
  `build.rs` runs the host `python3`; survey section 3.7) is still open.
- **Batch 42 re-fired** (`claude/pm-handoff-document-ypgky3` at `d0fe2fd0`) as main + #249 only. #249 changes
  servo-closure-smoke's patch table, so the gated `servo_closure_smoke` re-runs.

### 2026-10-31 ~09:50 EDT: the relay runner is back; 215-218 re-queued as 220-223; Chad 219 banked

- **Chad 219** came back at 09:05 EDT, so the runner is running again. Relays 215-218 (queued about 04:40 EDT) never
  returned and are re-queued as 220-223.
- **Chad 219 (#249, #261):** #261's vendored musl/libc++ licences and its syscall surface PASS, and the
  default/product builds are unchanged. FAILs:
  - #249 links Unicode-3.0 (ICU4X data crates), which is outside CLAUDE.md's listed licences. It is OSI-approved
    and permissive, and #245 already carries it host-side. **J.c.'s call:** add Unicode-3.0 to the allowed list.
  - #249: Stylo's `build.rs` runs the host's Python and writes docs outside `OUT_DIR`. Same open question for J.c.
  - #249 has no trap mining or CP rows for Stylo's Rayon/parking thread paths.
  - #261's `aligned_alloc` accepts zero alignment and a size that isn't a multiple of the alignment (C17 allows an
    implementation to accept it; contract wording).
  All go to seat D with the AAG's findings.
- Batch 41 (#247 + #253): 21/22 jobs done. Batch 42 (#249 alone): running.

### 2026-10-31 ~10:35 EDT: batch 41 green; re-checks 220/221 back

- **Batch 41** (run 41199447446, `d282d115`: #247 + #253): 22/22 green. Merges when Dexter 223 passes.
- **Chad 220:** #267 PASS (the sink is closed before both power paths, after Done and on timeout; the stuck-sink legs
  cover power-off and restart). #241's interface snapshot is now bounded (four passes). **#241 FAIL on PR text
  only:** the title and body still promise a hard 512 cap; the code and CP-54 correctly say admission threshold.
- **Janus 221:** #250's epoch counterexample is closed and the partial-write retry is bounded (16, then
  BlockDevice). **FAIL on claims:** the PR body's CP-64 row still says 257 entries; and off mode is not "bit for bit",
  because the bounded retry now succeeds after a racing `forget_range` where the old code returned an error (a fix,
  to be stated as one).
- Seat C is correcting the #241 and #250 PR bodies (no code push, so the gate stands). Batch 40 merges as a whole
  once Jarvis 222 (#254) is back.

### 2026-10-31 ~10:40 EDT: J.c.'s decisions on #249

- **Unicode-3.0 is an allowed vendored-code license** (the ICU4X data crates). CLAUDE.md's license list is updated.
- **Build-time Python is fine** (Stylo's `build.rs` runs the host `python3`; no network). Also recorded in CLAUDE.md.
- Both close Chad 219's first two #249 FAILs and the survey's section 3.7 question. #249 still needs its
  VENDORED.md, void-licence, survey-count and trap-mining/CP updates before it leaves draft.

### 2026-10-31 ~10:50 EDT: J.c. accepts #247's browse-only root for the Lens; pause for a computer switch

- **#247 F1 accepted as is (J.c.):** the Lens keeps a list-only `/` (LOOKUP|READDIR, no read; `.system` refused),
  like a Linux desktop file manager. Arthur stays scoped to his own subtree and `/boot`.
- **J.c. is switching computers and asked to pause everything once it's in a good position.** Plan: let relays 222
  and 223 finish on the PC so batches 40 and 41 can merge; every seat finishes its current step, commits and pushes,
  and starts nothing new; then J.c. stops the relay runner and runs the tree-state audit (CLAUDE.md: `Dirty = 0` and
  `Unpushed = 0` for every worker folder) before leaving the old PC. The resume list is
  `Plans/HANDOFF-2026-10-31-pause-for-computer-switch.md`.

### 2026-10-31 ~11:50 EDT: #247 and #253 merged (batch 41); batch 43 fired; #254 bounced again

- **The relay runner was not stalled.** Relays 215-218 and 222-223 all finished and pushed, but to branches named
  `RETURN-2026-10-31-…` without the `codex/` / `dexter/` prefix, which the PM's sweep missed. The PM now lists all
  remote branches when looking for returns.
- **Dexter 223:** #247 and #253 all items PASS. **#247 merged as `89af2e6b`** and **#253 as `992d42c9`** (batch 41,
  run 41199447446, 22/22 green; main was docs-only since its base, and main's code now equals the gate tree).
- **Jarvis 222: #254 FAILs item 1 again** (items 2 and 4 pass): the power-on origin is a one-sided check, later
  stamps are not ordered, and the comparison truncates to ms. Back to seat C (second round; the next review goes to
  GPT-6-Astra per the routing table).
- **Batch 43** (`claude/pm-gate-3` at `f84626cc`): main + #267 + #241 + #250, i.e. batch 40 minus #254, re-gated on
  current main. Reviews are complete (Chad 220, Janus 221, the PR-text fixes); it merges on green.
- **Public-repo check** (for J.c.'s move to GitHub-hosted runners): history scan done; summary given to J.c.
  in chat (not committed, since it lists what would need scrubbing).

### 2026-10-31 ~12:10 EDT: J.c.'s public-release decisions

- **Plan B:** publish a fresh public repo from a cleaned current main. The private repo keeps the full history.
- **Removed from the public copy:** the hardware inventories, the seven real-machine ACPI fixture sets (the QEMU
  sets stay; tests that need a real set skip when it's absent), the two evidence logs with real BSSIDs, and
  personal network details (Windows username paths, the runner hostname, LAN IPs, the owner's email in file text).
  In progress on `claude/public-scrub`.
- **Names:** the current repo is renamed `LogansOS-private`; the new public repo is `LogansOS`. Development
  moves to the public repo (GitHub-hosted runners are free there); the private repo becomes the archive, holding the
  real-machine fixtures.
- Before the public repo runs CI: switch every job to GitHub-hosted runners, and require approval for workflows from
  outside contributors, so no fork PR can run on J.c.'s machines.

### 2026-10-31 ~12:15 EDT: public snapshot prepared; J.c. approves the push after batch 43

- **The repo rename broke batch 43's first run:** its jobs, created under the old name, checked out from the new,
  empty public `LogansOS`. Infra only; re-fired. Worker remotes on the PC were checked: `relay-runner-repo` and
  `AltimitNeo` pointed at `LogansOS.git` and were repointed to `LogansOS-private`; all seats were warned to check.
- **Cleanup PR #273** (`claude/public-scrub`, `b3c77561`): inventories, the seven real-machine ACPI sets and the two
  BSSID logs removed; 18 MACs, 41 username paths, 136 emails, 3 full names, 26 hostnames and 70 LAN IPs replaced.
  Host tests 1654 pass; the ACPI crates pass with and without the real sets. Kernel changes are two comments.
  **#273 is not merged into the private repo**, which stays the full archive.
- **CI for the public copy** (`claude/public-snapshot`, `585caa81`): the QEMU job runs on `ubuntu-latest` (an opt-in
  `QEMU_RUNNER_LABELS` variable can name a self-hosted pool), a 14 GiB memory floor and a free-disk step on
  GitHub-hosted runners, and `qemu-trial.yml` removed.
- **J.c.:** keep the ACPI coverage reports (provenance and checksums, no raw tables or serials); push the public
  snapshot once batch 43 merges. It goes out as one root commit authored with his GitHub no-reply address.

### 2026-10-31 ~17:50 EDT: work moves to the public repo

- **The always-on relay box works:** a self-test sends one prompt to each engine; Codex and DeepSeek both answered.
  DeepSeek had been exiting silently because its CLI starts only under Node 24 (`import.meta.main`); the box moved
  to Node 24 LTS. Its relay runners now poll this repo's queue; relay 226 (Dexter, Lane 2a) is the first queued here.
- **Public CI:** shards 2-4 and every host job green on the hosted runners; shard 1 ran out of time on
  `desktop_home_cycle`, now on the 300 s desktop timeout (`76a1f03`), with a gate dispatched on main.
- **Open PRs carried over** (J.c. approved publishing): private #266, #261, #260, #249, #272, #255, #256, #257 are
  public #1-#8, each one squashed commit of the net change, scanned for personal details. Ten more need a rebase by
  their seats; the list and each seat's next steps are in `Plans/DISPATCH-2026-10-31-seats-resume-on-public.md`.
- **Seats A-D restart as fresh cloud sessions on this repo**; the old sessions (stuck on the rename) are left as they
  were, with their work already pushed to the archive.
- **J.c.: at least one seat on the browser at all times** (seat D, OT-61; it never idles or moves to other lanes).

### 2026-11-01 ~16:55 EDT: an escaped defect found by the preview image; merges since the move

- **Escaped defect, fixed:** the product image panicked at boot (CSPRNG init) because `rdrand_u64` treated RDRAND's
  carry flag backwards. Every gated kernel is `neo-test`, which seeds the DRBG with a fixed seed, so no gate reached
  `Csprng::init`. Found by Seat G's boot check of the preview VM image (#44), the first CI boot of a product kernel.
  Fixed in #43 (Chad's cross-family review PASS, with an independent carry harness); the `rng-seed-boot` job is the
  promoted scenario. CLAUDE.md's hermeticity note is corrected.
- **Merged since the move to the public repo (2026-10-31 evening):** #19, #1, #37 (Dexter's Lane 2), #34, #38, #39,
  #12, #9, #20, #31, #5, #32, #13 (user-copy-fixup, CP-99), #43, and #30 (USB mass storage, carrying #23).
- **Process:** every kernel PR gets a cross-family review on the relay box before merge; merge-ready PRs are gated
  together in a PM batch (`claude/pm-batch-N`) so one merge doesn't invalidate the others' gates.
