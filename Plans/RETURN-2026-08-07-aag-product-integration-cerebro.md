<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: AAG on the 0.1 integration build (`c8b9cb9e`) — Cerebro

**Date:** 2026-08-10 · **From:** Cerebro (Claude-family; cross-family lens on Chad/Codex work — not the author) · **To:** PM (Claude)
**Dispatch:** `Plans/DISPATCH-2026-08-07-aag-product-integration-cerebro.md` (`067ffe07`)
**Candidate:** `codex/product-integration-build-chad`, implementation `c8b9cb9e`, tip `f6a94bd9`
**Branch:** `cerebro/aag-product-integration-return` (doc-only, off `origin/main` @ `067ffe07`)
**RAG:** healthy (`status=healthy, ollama=true, qdrant=true`) at 2026-08-10T16:32Z; used to orient, then every
finding below was established by direct reads of the candidate tree (`git show c8b9cb9e:<path>`), not by RAG
snippets and not from the RETURN.

## Verdict

**MERGE.** Four of the five rungs carry a nameable breaking mutation and go red under it. The three degrade
scenarios reach the arms they name and are discriminated by runtime latches plus a probe counter — the gap I
found in the rung-5 AAG is genuinely closed, and closed in the strong form (a counter that distinguishes
`NoValidSuperblock` from `NoBlockDevice`), not the weak one. The kill-switch statement is truthful, including
its refusal of the blanket claim.

**Two confirmed findings, neither merge-blocking.** F1 is the one that matters: **rung 3's input-fidelity claim
is not fail-capable** — I can name a mutation that violates "the typed question reached Arthur" and leaves the
gate green, and two serial strings assert that property in prose anyway. F2 is a truthfulness defect in the
boot-1 PASS line. Neither introduces a defect to `main`; both are things that would be *cited* as proof of 0.1
and should not be. Recommended disposition: merge now, fix F1+F2 forward in the next product-touching build,
and do not cite either string as evidence in the meantime.

---

## Lens 1 — Do the five rung assertions have teeth? (primary)

Checked against the code: `kernel/src/test_harness.rs` (the product block, ~2236–2320), `userland/neo-init/src/main.rs`
(`run_selected_scenario` + `product_integration_tests`), `userland/neo-init/src/dw1_desktop_wm.rs` (95–175),
`userland/neo-init/src/arthur_compositor.rs`, `userland/neo-arthur/src/main.rs` (609–735), and the host-side
`require_observed_pass` / `run_reboot_persistence_scenario` in `tools/xtask/src/main.rs`.

**First, a structural check the lens depends on.** The two-boot driver writes **two separate serial logs**
(`…boot1.serial.log`, `…boot2.serial.log`) and calls `require_observed_pass` on each. So
`require_observed_pass`'s `if serial.contains("stage=readback")` branch selector is not self-fulfilling: boot 1
cannot contain that literal (`read_prior_boot` returns `None` on `ENOENT`, and the `stage=write` line's
`same_boot_readback=` does not contain the substring `stage=readback`), and boot 2 must contain it or the
generic `require_serial_contains(&boot2, "stage=readback")` fails. A disk that failed to persist across boots
therefore lands boot 2 in the boot-1 branch and is caught by that generic check. **The branch selector is sound
— this was my first suspicion and it does not hold.**

### Mutation table

| Rung | Breaking mutation | Gate result | Teeth |
|---|---|---|---|
| **1 — boot / framebuffer** | Prevent the framebuffer from being mapped so `fb::read_pixel(0,0)` returns `None`. | **RED** — `fail("reason=product_framebuffer_not_initialized")`. A panic before the harness produces no PASS line at all. | **Yes.** Bounded: it proves the fb is mapped and readable, not that anything legible is on it. |
| **2 — desktop** | Deliver a focus event to the *unfocused* client; drop a delivery; make direct present succeed instead of returning `EPERM`. | **RED** — `recv_control(D/N)` mismatch → neo-init 51/54; `sys_input_focus_recv` on a client activator not returning `EPERM` → 56; child nonzero → `100 + code` → kernel FAIL. | **Yes.** See caveat below. |
| **3 — Arthur (answer half)** | Return an empty/short reply; echo the prompt back; return non-printable bytes; break the model-path breadcrumb. | **RED** — `product_reply_is_coherent` → 60 → `200 + 60` → FAIL. The forward is a live tensor forward (`forward=real`). | **Yes.** |
| **3 — Arthur (input half)** | **Drop any subset of the injected keystrokes in the focus-delivery path**, leaving ≥1 printable byte and the newline. | **GREEN.** See F1. | **No.** |
| **4 — eth0** | Publish any `BootEth0Status` other than `NoDevice`. | **RED** — `fail("reason=product_eth0_expected_clean_no_dongle_degrade")`. | **Yes**, and strongly — see Lens 5, this is an *asserted* latch, so a real dongle makes this scenario red. |
| **5 — persistence** | Corrupt one of the 39 bytes; short write; short read; fail `fsync`; seed a fresh disk for boot 2. | **RED** — `verify_file` → 245, `sys_write`/`sys_fsync` → 242/243, and a fresh disk is caught by the generic `stage=readback` requirement. | **Yes** for byte-equality. **No** for fsync *necessity* — see caveat. |

### What I could not break, stated plainly

1. **Rung 3's input fidelity — F1, below.** This is the one real hole.
2. **Rung 5's fsync necessity.** The `sys_fsync` return value is checked (`!= 0 → 243`), so a *failing* fsync is
   caught. But the scenario does not differentially prove that the record would be lost *without* it: QEMU
   flushes the backing file on clean shutdown, so a build with the `fsync` call deleted would very likely still
   pass boot 2. The durability claim rests on the call succeeding, not on an observed difference. This is a
   pre-existing property of every reboot-persistence scenario, not something this build introduced, and I am
   recording it as a limit rather than a finding.
3. **Two of rung 2's sub-claims.** `focus_atomicity=count_conserved injected=2 delivered=2 lost=0
   duplicated=0` and `wm_event_bytes=0` are literal strings. The *conservation* property is enforced
   structurally — each of the two rounds is individually guarded by `focus_round` + a `recv_control(b'D')` on
   the focused client and `recv_control(b'N')` on the unfocused one, so a lost or misrouted delivery returns
   51/54. A **duplicated** delivery would most likely be caught on the next round by `recv_control` reading a
   stale byte, but that is my inference about channel behaviour, not an assertion in the code, and
   `wm_event_bytes=0` is not checked anywhere I can find. **This is materially better than the DW-1 state I
   flagged previously** — the numbers are labels on real guards rather than a printed string standing alone —
   but `duplicated=0` and `wm_event_bytes=0` are not independently proven.

### F1 — CONFIRMED: rung 3 asserts the typed prompt at the sender, never at the receiver

**Claim under audit.** `SHOWCASE product typed question=hello_world bytes=12 source=input_focus_test_router`
and `stage=arthur typed_prompt=hello_world coherent_reply=validated provenance=neofs_model_path`, both in the
harness's required boot-1 sequence.

**What the code establishes.** `arthur_compositor.rs` injects 12 HID usage IDs and returns 91 if any
`sys_input_focus_test_inject` returns nonzero. The success of *twelve send calls* is what the `bytes=12` string
records. On the receiving side (`neo-arthur/src/main.rs`, 653–730) Arthur polls, accumulates printable bytes
into `input`, and fires inference on `b'\n' if input_len != 0`. **There is no check on `input_len`, and no
comparison of `input[..input_len]` against the expected prompt.**

**Why the reply cannot substitute for that check.** The fixture graph "deterministically selects token 3 after
a live forward" (xtask comment, `prepare_product_integration_brain_model`), and the product-only vocab sidecar
maps token 3 → `Arthur is ready.`. **The reply is by construction independent of the prompt.** So it carries
zero information about what was typed. `product_reply_is_coherent` checks `len >= 4`, `reply != input`,
contains an alphabetic byte, all ASCII graphic/whitespace — every one of which holds for any truncated prompt.

**The mutation.** Drop every other event in the input-focus delivery path. Arthur receives `hlowrd\n`,
`input_len = 6 ≠ 0`, the forward runs, the reply is still `Arthur is ready.`, coherence passes, exit 0, and the
scenario is **green** while the property "the typed question reached Arthur" is violated. `reply != input` does
not help; a truncated input is still `!=` the reply.

**Severity.** Not merge-blocking: no defect reaches `main`, and rung 3's *answer* half (real forward, real
model provenance, non-echo, printable) is genuinely proven. But this is the rung that would be cited as "you
type, Arthur answers," and the input half of that sentence is unasserted while two serial strings state it.

**The evidence already exists and is merely unasserted.** Arthur logs every received keystroke
(`SHOWCASE keystroke ascii=<n>`). Cheapest fix, in order of preference:
1. In Arthur, under `arthur-compositor-product-integration`, gate inference on
   `input_len == 11 && &input[..11] == b"hello world"`, returning a distinct code (e.g. 61) otherwise.
2. Or pin the 12 `SHOWCASE keystroke ascii=` lines with their decoded values into the required boot-1 sequence
   in `require_observed_pass`.
3. Failing either, **weaken the strings** — `typed question=hello_world` → `injected_keycodes=12`,
   `typed_prompt=hello_world` → `prompt_injected=12_keycodes` — so nothing claims delivery that was not checked.

### F2 — CONFIRMED: boot 1's terminal PASS advertises a two-boot readback it did not perform

On boot 1 `write_first_boot` returns 0, the `exit_code == 78` block is skipped, and control reaches the final
`pass("product_integration=ok product_feature=compiled rung1=… rung5=two_boot_byte_readback boot_neofs=mounted
probe_attempts=1")`. **Boot 1 performed a same-boot reopen-and-compare, not a two-boot readback.** Boot 2 emits
the correctly distinct `second_boot=prior_disk_byte_readback`.

This is not vacuity — boot 2 does prove rung 5 — it is a claim string that is false on the path that emits it.
It matters because the boot-1 log is the one with all five rung tokens in a single line, so it is the line a
reader would cite. Fix: on boot 1 emit `rung5=same_boot_fsync_readback` and leave
`rung5=two_boot_byte_readback` to the boot-2 PASS, which already earns it.

### N1 — note: `78` as the boot-2 success sentinel is one edit away from reading failure as success

`run_selected_scenario` returns 78 for "prior-boot record verified"; the kernel treats `exit_code == 78` as the
boot-2 PASS trigger. But `arthur_compositor::run()` independently returns **78** as a *failure* code (focus
activate or session send failed). Today there is no collision: the product driver wraps Arthur's code as
`200 + arthur`, so a bare 78 can only come from the readback path. The safety therefore rests entirely on that
`200 +` offset, in a different function from the one that defines the sentinel, with nothing asserting the
relationship. A future edit that returns `arthur` unwrapped converts a *failure to activate focus* into a
*declared second-boot success*. Cheap hardening: move the sentinel out of the error range (e.g. 0x7B00) or
add a debug-assert that the wrapped ranges cannot produce it. Low severity, zero cost to fix.

---

## Lens 2 — Do the three degrade scenarios reach the arms they name?

**No findings.** All three reach their arms, and the discriminator the dispatch asked for is present and is the
strong form. Checked in `kernel/src/fs/mod.rs` (753–812), `kernel/src/test_harness.rs` (2086–2126),
`kernel/src/main.rs` (688–696), and the disk preparation in `tools/xtask/src/main.rs`.

The mechanism: `mount_boot_neofs_from_device0_readwrite` latches an outcome code per match arm
(`Mounted=1 / NoBlockDevice=2 / NoValidSuperblock=3 / UnexpectedError=4`), and `find_neofs_block_device`
zeroes `NEOFS_PROBE_ATTEMPTS` on entry and increments once per device index examined. Each scenario asserts
**both** the latch and the counter. That pairing is what makes the assertions non-vacuous: the latch alone
could be `NotAttempted`-vs-real ambiguous, and the counter alone could not distinguish arms.

| Scenario | Reaches the named arm? | The dispatch's specific question |
|---|---|---|
| `boot_neofs_no_block_device` | Yes. `scenario_uses_storage_disk` is false → no disk attached → `device_count() == 0` → early `Err(NoBlockDevice)`. Asserts latch `NoBlockDevice` **and** `probe_attempts == 0`. | — |
| `boot_neofs_no_valid_superblock` | **Yes — and this is the answer to the dispatch's question.** `neofs_image_variant_for_scenario` returns `None` for it, so `prepare_storage_disk` falls past the NeoFS-image branch to the raw-disk path: a **present block device that is not NeoFS**. The harness asserts latch `NoValidSuperblock` **and `probe_attempts == 1`**. | **The `NoBlockDevice` mis-landing is excluded by construction.** `probe_attempts == 1` can only hold if a device was enumerated *and* probed; the `NoBlockDevice` arm returns before any increment, so it is arithmetically impossible for that arm to satisfy this assertion. The gap I raised is closed. |
| `boot_neofs_unexpected_error` | Yes. Two disks (raw at index 0, valid NeoFS at index 1) plus a test-only pre-installed `/neofs` `TestFs` with a live handle held open, so the real boot mount selects device 1 and then fails through the ordinary mount-table error path. Asserts latch `UnexpectedError` **and `probe_attempts == 2`**. | Reachable, not theoretical. The counter additionally proves index 0 was skipped before index 1 was selected — the selection path is exercised, not just the error arm. |

**Does each assert that boot proceeded, not merely that the mount failed?** Yes, structurally: all three
assertions live in `run_selected_scenario` in the kernel test harness, which executes *after* boot completes;
reaching the `pass()` call is itself the proof that boot continued past the failed mount. A boot that halted or
panicked at the mount produces no PASS line and the runner fails.

**One scope note, not a finding.** All three scenarios are `user_feature: None`, so what is proven is that the
**kernel** continues to scenario execution. They do not prove that **userland spawn** survives a failed
`/neofs` mount — which is the case a user would actually meet on a machine with a corrupt disk. I-14(b)'s claim
is about governance liveness and the kernel-level proof is a fair reading of it, but "the desktop still comes
up with no filesystem" remains unproven by these three. Worth a future scenario; not something to hold this
merge for.

**On the synthetic trigger for the catch-all.** The `UnexpectedError` arm is reached via a mount-table `Busy`,
not via an I/O or journal error of the kind that would produce it in the field. The arm's *code* genuinely
executes and boot genuinely continues, which is what the arm claims, so I am not raising this as a finding —
but the scenario proves the arm is live, not that every real error routes into it.

---

## Lens 3 — `product-build` is blocking, and `host-checks needs:` it

**No blocking finding. One concrete improvement, N3.**

Verified job graph in `.github/workflows/ci.yml` at `c8b9cb9e`: `product-build` → `host-checks` → `neo-test`
(the four QEMU shards). So `product-build` now gates the entire pipeline. The job runs
`cargo build -Zjson-target-spec --bin kernel --features product` on `ubuntu-latest`, 30-minute timeout, with
its own cache namespace, and carries **no** `continue-on-error` — which the xtask host test
`ci_workflow_shape`-style assertion pins (`assert!(!product.contains("continue-on-error: true"))`), with the
job's extent bounded by searching for the `# OT-69:` comment. If that comment is ever moved above the product
job the `.expect(...)` panics rather than silently passing — fail-closed, correct.

**Can it block `main` on a non-defect?** The genuine exposures:
- **Infra classes** — toolchain install, crates.io reachability, cache restore, runner loss. These are
  runbook-class reds and are already exempt from the escaped-defect contract (re-fire, do not revert). The
  change does not create a *new* class; it widens the blast radius of an existing one, because a
  `product-build` infra flake now also suppresses host-checks and all four shards rather than reddening one
  job. That is a throughput cost, not a correctness hazard.
- **The OT-69 hazard, specifically.** OT-69 landed non-blocking to avoid reddening `main` on *known,
  pre-existing subsystem debt* across a broad feature matrix. `product-build` is a different shape: one
  fixed feature set, the one that ships, which was green on this SHA. It does not sweep unknown debt into the
  blocking tier. **The hazard is not reintroduced by this door.** The OT-69 matrix below it remains
  `continue-on-error` and the comment explicitly separates them.

**N3 — the chain is stricter than the stated goal requires.** The job's own comment gives the goal: "a
product-only compile regression must make the workflow red and prevent the expensive QEMU gate from starting."
Both halves are satisfied by making the *gate* depend on it, not by putting it in front of `host-checks`.
Chaining `host-checks needs: product-build` puts a full kernel build in front of the fastest feedback in the
pipeline — `fmt`, workspace tests, clippy — so every push, including doc-adjacent and host-only ones, now waits
on a `--features product` compile before it learns its formatting is wrong. Suggested:

```yaml
  host-checks:            # (no needs: — back to a root job)
  neo-test:
    needs: [host-checks, product-build]
```

Same blocking strength for `product`, same protection of the expensive gate, fast feedback restored, and a
`product-build` infra flake no longer hides otherwise-valid host-check results. This is an improvement, not a
defect — it does not hold the merge.

---

## Lens 4 — Kill-switch, stated truthfully

**No findings. The RETURN's statement is accurate, and it is accurate in the form I would have demanded.**

Chad's §"Kill-switch boundary, stated literally" explicitly refuses the blanket historical claim and names
rung-5's F1/F2 as unconditional ride-alongs that this build neither hides nor changes. That is the true form,
not the strong-sounding one. Verified against the code rather than taking it:

- **`product` OFF, `neo-test` OFF (the production build):** every addition in this commit is behind
  `#[cfg(feature = "neo-test…")]` or `#[cfg(feature = "…product-integration")]`. `find_neofs_block_device`,
  `mount_boot_neofs_from_device0_readwrite`, and the `usb::init` path compile to their prior form. In
  particular `usb/mod.rs`'s new gate is
  `any(not(feature = "neo-test"), feature = "neo-test-product-integration")`, whose first disjunct is exactly
  the previous condition — so a production build's `boot_eth0::bring_up()` call site is unchanged.
- **Other `neo-test` scenarios:** unaffected. `BYPASS_DST` stays `(0,0)` under
  `not(neo-test-product-integration)`; the `arthur_compositor` `SCENARIO` const and its slice-A `pass()` are
  newly excluded only for the product scenario; the `SYS_INPUT_FOCUS_TEST_INJECT` change appends a feature to
  an existing `any(...)` list.

**N2 — one precision the RETURN could add.** `NEOFS_PROBE_ATTEMPTS` is gated on `#[cfg(feature = "neo-test")]`
alone, not on the three degrade features, so the `store`/`fetch_add` **rides in every `neo-test` build**, not
only in the scenarios that read it. It is a relaxed-ordering atomic on a cold path and I am not raising it as a
defect — production is genuinely untouched, which is what the kill-switch claim covers. But since this lens
exists precisely because a blanket "≡ prior path" was previously false, the honest wording is "*production*
builds are byte-identical; the `neo-test` profile gains a probe counter." **No new unconditional ride-along in
production builds was added by this commit** — that is the claim I checked and it holds.

---

## Lens 5 — What this scenario cannot cover (and therefore what the hardware boot meets first)

**The dispatch's specific question, answered first: rung 4 under QEMU asserts the no-dongle degrade, not a
lease, and the distinction is ASSERTED, not assumed.** The harness reads the runtime latch published by the
real bring-up and fails on anything other than `BootEth0Status::NoDevice`:

```rust
if crate::drivers::usb::boot_eth0::debug_status()
    != crate::drivers::usb::boot_eth0_logic::BootEth0Status::NoDevice
{ fail("reason=product_eth0_expected_clean_no_dongle_degrade"); }
```

Two consequences worth stating explicitly. (a) A DHCP **lease** is nowhere proven by this scenario — rung 4
proves the *degrade* arm only. (b) Because the latch is asserted for equality, **this scenario would go RED on
bare metal with a working AX88179 attached.** It is QEMU-only by construction and cannot be reused unmodified
as the hardware rung-4 proof; the hardware variant needs its own assertion (`Leased` + a real IP), and that is
new work, not a re-run.

**What remains unproven after this scenario is green — the list the bare-metal boot will meet:**

1. **The artifact under test is not the shipping artifact.** `neo-test-product-integration = ["neo-test-arthur-compositor", "product"]`
   — the booted binary carries the `neo-test` harness, the `SYS_INPUT_FOCUS_TEST_INJECT` syscall, and the
   test-only fb/eth0/NeoFS observability hooks. `product-build` compiles the shipping composition but **nothing
   boots it.** "The 0.1 showcase ISO boots" is proven for a near neighbour, not for the ISO. This is the single
   most important gap on the list.
2. **The real USB HID path.** Input arrives via the test-only injection router. HID enumeration, report
   decoding, and the physical-keyboard → InputFocus seam are exercised by *no* rung here (the bare-metal
   compositor variant marks them `qemu=unsupported`). On hardware this is the first thing between the user and
   rung 3, and F1 means we do not even have the QEMU-side delivery-fidelity assertion to fall back on.
3. **Legibility.** Rung 1 proves the framebuffer is mapped and readable at `(0,0)`; the bypass oracle proves an
   unauthorized client cannot mutate one sentinel pixel. Neither proves anything is *legible*. Per the
   bare-metal runbook the legibility proof is the PL's live eyes-on read plus the serial `REPLY:` line; that
   remains outstanding.
4. **Real GPU.** Composition runs `PresentMode::Software` raw scanout with `gpu_submit_enforce`. Mode-set,
   hardware flip, and the real iGPU path on the M710q/OptiPlex are untouched.
5. **The model answer.** The reply is a fixture that deterministically selects token 3, surfaced as
   `Arthur is ready.` via a product-only vocab sidecar. The forward is real; the *answer* is not the 0.5B
   model's. A real-artifact run (`STAGE_B_NEOM`/`GGUF`) takes a different path that this gate never runs.
6. **Storage.** One virtio-blk disk. The two-device catch-all is synthetic (a held `TestFs` mount, not an I/O
   error). Real NVMe/AHCI enumeration, real multi-device selection, and real power-loss durability are
   unproven — see also the fsync-necessity limit under Lens 1.
7. **Boot-continues-into-userland with no filesystem.** Lens 2's scope note: the degrade scenarios prove the
   kernel continues, not that the desktop comes up.

---

## Rules compliance

- Every finding above was re-verified against `c8b9cb9e` source before being written down; F1's mutation was
  traced end to end (injection site → focus delivery → `poll_ascii` accumulation → `input_len != 0` →
  `product_reply_is_coherent`) rather than asserted.
- Uncertain items are recorded as notes naming the missing fact (N1, N2, N3, the fsync-necessity limit, the
  `duplicated=0` / `wm_event_bytes=0` sub-claims, the userland-degrade scope note), not as blockers.
- No finding was manufactured. Lenses 2 and 4 are clean results and are reported as clean, with what each
  checked stated above.
- Doc-only. No build, no gate, no runner consumed.
