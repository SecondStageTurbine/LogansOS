# PM Disposition - Phase 3 BUILD-4C Close Consumption And Ratification

Date: 2026-05-20
From: PM
Status: ACCEPTED / BUILD-4C CLOSED / PUSH-AND-MERGE AUTHORIZED
Consumed artifact:

```text
Plans/PHASE-3-BUILD-4C-CLOSE-EVIDENCE-2026-05-20.md
```

Commits:

```text
a627d670e81a277b1c5c80b3af22624225767ff2  build-4C: add FAT32 read-only foreign filesystem driver
ad31f1269fb9c069b6116eb9887c090307fe9e94  (close evidence commit)
```

Diff: 8 files, 1304 insertions, 127 deletions.

## 1. Disposition

BUILD-4C is closed. All dispatch acceptance gates from
`Plans/DISPATCH-2026-05-20-phase-3-build-4C-fat32-readonly-jarvis.md`
are met. PM consumes the close artifact and authorizes push and merge of
both commits to main.

## 2. Gate Verification

PM walked the close artifact against the dispatch contract; every gate
passes:

**Scope items (dispatch §2.1):**

- FAT32 on-disk parser with BPB/EBPB validation, FAT region read, cluster
  chain walk, short and LFN directory entries with checksum validation —
  ✓ §3 confirms; LFN scenario passes case-preserving display and case-
  insensitive lookup.
- `MountedFs` implementation with FAT32 root and lookup; inode kinds
  restricted to Directory and Regular — ✓ §3, §4 confirm.
- **`MountedFs` trait did not require modification** — ✓ explicitly stated
  in §3. This was the load-bearing abstraction-validation test from
  dispatch §3.1; the substrate is genuinely filesystem-agnostic.
- `fs_type = 3` for FAT32 device1 read-only — ✓ §3 and scenario PASS line
  confirm.
- Mount-flags ABI extension: bit 0 = `MOUNT_FLAG_EXEC_ALLOWED`, reserved
  nonzero bits return `EINVAL` — ✓ §3 confirms.
- Mount rights mask with `effective = inode.available_rights() &
  mount_entry.rights_mask` — ✓ §3 confirms the formula verbatim.
- FAT32 default `rights_mask` clears WRITE always, clears EXECUTE unless
  `MOUNT_FLAG_EXEC_ALLOWED` is set — ✓ §3 and scenarios confirm.
- xtask FAT32 image generation via mtools, no committed binary fixtures —
  ✓ §3 confirms generation path; §4 confirms no fixture landed.
- libneo additions for new fs_type and flag constants — ✓ implied by the
  libneo diff (+7 lines).
- All nine required scenarios PASS with literal `NEO_TEST/1 event=PASS`
  frames — ✓ §6 lists all nine with the dispatch-specified scenario names.

**Anti-scope (dispatch §2.2):**

- No FAT32 write code; no ext4 code; no exFAT/FAT12/FAT16 code — ✓ §4
  explicit confirmation.
- All FAT32 entries imported as `Regular` or `Directory`; no `.neom` file
  imported as `Model` — ✓ `phase3_fat32_type_import_is_regular` scenario
  shows `neom_kind=Regular, model_imports=0, symlink_imports=0`. The
  Bible Article 10.3 + ADR-021 Q9 invariant is upheld.
- No multi-device FAT32 mount; no generic device-string parser — ✓ §4.
- No `MountedFs` or `Inode` trait change — ✓ §4.

**Verification gates (dispatch §4):**

- `cargo k` PASS, 52 warnings (one below BUILD-4B's 53 baseline; clean
  movement, no new warnings introduced).
- `cargo k --no-default-features` PASS, 52 warnings.
- All nine FAT32 scenarios PASS.
- All ten dispatch-required anti-regression scenarios PASS, including
  `phase3_neofs_filecap_generation_stale`,
  `phase3_neofs_fsync_filecap_stale_regression`, and
  `phase3_sched_wake_token_abandon` — the previously-closed lanes remain
  green under the BUILD-4C delta.
- Final `NEO_SMP=4 cargo neo-test --no-meta` exits 0 with full SMP run
  log archived.

**Required close artifact content (dispatch §6):**

Every listed requirement present; PM finds the artifact complete and
well-disclosed.

## 3. Substantive Disclosures Worth Flagging

### 3.1 virtio-blk completion polling fix

Jarvis discovered during BUILD-4C bring-up that the virtio-blk completion
worker was polling only the primary device. He extended it to register
all virtio-blk devices and poll all completion queues. This is the kind
of "discovered while implementing" infrastructure assumption that
naturally surfaces when a new device class (FAT32 on `device1`) starts
exercising the second virtio-blk device.

PM accepts this as part of BUILD-4C scope. It is a prerequisite for the
BUILD-4C scenarios to run at all (device1 must be functional), Jarvis
disclosed it clearly in §3 with the technical context, and the fix is
minimal in scope (18 lines in `virtio_blk.rs`). Treating it as a
separate commit would have introduced an artificial ordering dependency
that adds no value.

For future awareness: this is the kind of fix that should be called out
prominently in close evidence so PM and PL can adjudicate without having
to read the diff. Jarvis did this correctly.

### 3.2 `VIRTIO_DEVICES` new lock site

A new lock was introduced for the virtio-blk device registry. §3 states
"The new `VIRTIO_DEVICES` lock has IF discipline documented at its
declaration site."

The dispatch's stop condition was specifically about new `SwitchMutex`
sites lacking IF discipline. The close artifact does not explicitly
state whether `VIRTIO_DEVICES` is a `SwitchMutex` or another lock type.
However, the IF discipline is documented at the declaration site
regardless, which satisfies the underlying intent of the stop condition.

For documentation completeness, PM directs that the type of any
newly-introduced lock be named in future close artifacts (e.g.
"`VIRTIO_DEVICES` is a `SwitchMutex<...>` with IF discipline documented
at its declaration"). This is documentation hygiene for close artifacts,
not a blocker for BUILD-4C closure.

### 3.3 Test-harness diff size

The `kernel/src/test_harness.rs` diff is +464 lines, the largest single
diff outside `fat32.rs` itself. This is expected — BUILD-4C added nine
new scenarios that needed test-harness infrastructure for FAT32 mount
helpers, fixture access, and the assert-policy patterns specific to
foreign-FS testing. PM does not flag this as scope creep; it is the
natural shape of "nine new test scenarios for a new filesystem."

## 4. Phase 3 Status After BUILD-4C

- **BUILD-1, 2, 3, 4A, 4B, 4C:** closed.
- **BUILD-4D (ext4 read-only):** next authorized Phase 3 lane per ADR-021
  and the BUILD-4 prereq audit. Requires either `e2fsprogs` in the
  container or a controlled Rust image generator before implementation
  (per audit D9). Has not been dispatched.
- **BUILD-4E (Phase 3 integration close):** ahead.
- **BUILD-5 (AHCI/NVMe + bare-metal validation):** ahead. Bare-metal
  target hardware acquisition is in progress on the PL side (T480s 20L8
  acquisition path).

Phase 3 is now five-sixths closed by BUILD count. The remaining work is
ext4 read-only, integration close, and the bare-metal validation lane.

## 5. Authorized Forward Lanes

### 5.1 Push and merge

Per dispatch §6 implicit (BUILD-4B established the close-consumption →
push-and-merge pattern), push and merge of commits `a627d670` and
`ad31f1269` to main is now authorized. Standard git posture from current
handoff applies: local main is ahead of `origin/main` and behind by
README-only cherry-picks; do not rebase or rewrite without explicit PL
guidance.

### 5.2 BUILD-4D (ext4 read-only)

BUILD-4D is the next authorized Phase 3 lane per ADR-021. PM does not
authorize BUILD-4D in this disposition. Authorization will be a separate
dispatch after PL acknowledges BUILD-4C closure and signals readiness.

The BUILD-4 prereq audit's D9 finding stands: ext4 image generation
requires either `e2fsprogs` (which is also in the Dockerfile per a quick
check) or a Rust image generator. Confirmation that `e2fsprogs` is
present inside the dev container should be the canonical first step of
the BUILD-4D dispatch, using the same `docker compose run --rm neo
bash -c 'which ...'` idiom now established by the BUILD-4C mtools
clarification.

Agent assignment: heavy coding lane goes to Jarvis per current roster.
BUILD-4D will validate that BUILD-4C's `MountedFs` substrate accepts a
second foreign filesystem without modification, mirroring BUILD-4C's
validation of BUILD-4B's substrate.

### 5.3 ADR-022 ratification still pending

ADR-022 from the Prometheus three-item bundle is still awaiting PL
disposition per
`Plans/PM-DISPOSITION-2026-05-20-prometheus-three-item-followup-consumption.md`
§4.1. PM recommendation remains option (a) — ratify as-is.

### 5.4 Small follow-up tasks (opportunistic)

- Documentation hygiene: BUILD-4C close artifact does not explicitly name
  the lock type of `VIRTIO_DEVICES`. Could be added in a tiny edit if PL
  prefers the close evidence to be fully self-describing, or left as-is
  since the IF-discipline requirement is satisfied either way.
- Forward-marker note in close evidence about virtio-blk polling
  generalization: now that the worker polls all virtio-blk devices,
  future BUILD-4D ext4-on-device2 will reuse the same machinery. Worth
  mentioning if a small follow-up edit is opened.

Neither is urgent.

## 6. PM Posture

Lanes after this disposition:

- **Cerebro:** idle.
- **Jarvis:** BUILD-4C closed, idle. Available for BUILD-4D when
  authorized.
- **Prometheus:** idle.

The day's progress: four artifacts closed cleanly (BUILD-4B, Jarvis
audit, ANSPEC-002 v0.1.1 amendment, three-item bundle), plus BUILD-4C
closed today. Two stop-condition surfaces handled cleanly (mtools-in-
container clarification, Prometheus wrong-file). No substantive process
problems.

PM stands ready for:

- PL acknowledgment of BUILD-4C closure.
- PL signal on BUILD-4D authorization.
- PL signal on ADR-022 ratification disposition.
- PL signal on BUILD-5 bare-metal target hardware commitment (T480s
  20L8 path or alternative).
