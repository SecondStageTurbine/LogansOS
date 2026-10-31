<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-87 installer THINK narrow re-check

**Relay:** #87. **Reviewer:** Chad, Codex family. **Reviewed SHA:**
`1f313ac651cd53fe654236fae29c854be67a8b93` (revision 2 over `f1cec9c2`).
**Scope:** relay #86 items 2, 3, 5, 6, 7 and 8, plus J.c.'s erase and read-only Try decisions.
**Worktree:** `E:\AI Automation\workers\chad`.

**Overall verdict: FAIL — revise the remaining write-identity and persistence contracts.**
The NeoFS deployment storage conflict, GPT geometry specification, RAM payload source, input selection,
minimum capacity and rung dependencies are resolved at design level. The remaining findings below are
design findings, not claims that unimplemented installer code has failed a runtime test.

RAG health returned `healthy`, `ollama=true`, `qdrant=true`. Read the relay #86 RETURN and relay #87
dispatch from `origin/main`, reviewed the revision diff and THINK directly, and spot-checked the cited
block, filesystem, logging and image-feature code at the reviewed SHA. Graft discovery returned unrelated
symbols; exact caller searches and direct reads supplied the code evidence. No build or tests run (review-only
dispatch; no build needed).

## 1. Item 7: A/B — FAIL on persistence ordering; storage, write bounds and budget PASS

- **PASS: storage and scope.** `docs/THINK-2026-10-27-ot87-installer.md:563` keeps deployments authoritative
  in NeoFS, matching `docs/THINK-2026-07-10-ot51-update-rollback-model.md:43`. The ESP is a derived boot copy.
  `docs/THINK-2026-10-27-ot87-installer.md:574` limits the slot writer to preallocated contiguous file extents,
  their directory-entry FileSize fields and the configuration commit sector, without FAT/chain edits.
  Rollback and inactive-slot copying are explicit at `:582`; OT-51 ratification is an R7 dependency at `:651`.
- **PASS: capacity arithmetic.** The five image A file sizes at
  `docs/THINK-2026-10-27-ot87-installer.md:604` sum to 133,744,032 bytes, and two sum to 255.0965 MiB.
  The stated 512 MiB ESP budget is consistent with those measurements and sub-1-MiB fixed overhead
  (`:614`). This checks the published numbers, not a fresh build measurement. The exact image A
  remeasurement remains an R4 gate (`:600`).
- **FAIL: power loss can select an incompletely persisted slot.**
  `docs/THINK-2026-10-27-ot87-installer.md:584` requires copying and read-back before the single-sector
  flip, but specifies no flush/durability boundary. Concrete input: a device completes writes and serves
  read-back from volatile cache; the commit sector reaches persistent media before some inactive-slot
  data or FileSize sectors; power is cut. The commit sector is entirely new (the sector-atomic assumption
  holds), yet it points to incomplete files. Single-sector atomicity does not establish persistence order.
  The block API exposes a separate flush (`kernel/src/drivers/block/mod.rs:77`); NVMe and AHCI implement
  distinct flush commands (`kernel/src/drivers/block/nvme.rs:414`, `kernel/src/drivers/block/ahci.rs:336`).

  **Required correction:** specify successful persistence of all slot data and directory metadata before
  the commit write, persist the commit before reporting activation successful, and refuse activation if the
  required durability/atomicity contract is unavailable. Include volatile-cache loss and reordered persistence
  in the activation oracle, in addition to cuts between submitted sectors.

## 2. Items 2 and 5: identity — FAIL

`docs/THINK-2026-10-27-ot87-installer.md:166` adds the registry generation, driver identity and content
fingerprint; `:188` refuses erase without stable identity. The table at `:198` explicitly covers reorder,
removal/reinsert, unavailable identity, same-tuple replacement, boot disks and protected disks. These are
useful corrections to relay #86.

However, `docs/THINK-2026-10-27-ot87-installer.md:183` checks identity and fingerprint only immediately
before the first write. `:185` requires only generation/liveness before later **batches**. This does not
meet the dispatch's before-every-write contract, nor explain how validation and submission stay bound
to one device during a batch.

**Concrete input:** move the table's same-tuple replacement hook (`:201`) from before the first write to
between later batches, retaining the slot generation and liveness used by that hook. Different contents
are no longer checked by the specified later-write validation. Also test replacement between validation
and command submission, rather than only before preflight.

**Required correction:** make each write submission use the validated kernel-held device object under
a defined replacement/submission synchronization rule; state which identity/content checks recur and
why a later replacement cannot evade them. The fingerprint must account for the installer's own changes
to the first/last MiB; rechecking the original enumeration digest unchanged would reject its own writes.
Extend the test table to replacement after the first write and during submission. Until that contract is
stated, the broad claim that every write reaches only the confirmed disk remains unproven.

## 3. Item 5: fresh GPT and oracle — PASS

`docs/THINK-2026-10-27-ot87-installer.md:313` selects the target's sector geometry and matching ESP
template. `:317` specifies fresh primary and backup headers, entry arrays and usable bounds; `:339`
lists entry fields; `:347` lists protective MBR fields. CRC-E, CRC-H1 and CRC-H2 are separate (`:351`).
Both 4-GiB fixtures (`:357`) have the stated LBAs and 916,992 NeoFS blocks; the partition ends before
the backup array. Hand-laid bytes with independent Python CRC computation and writer equality,
guard acceptance and `sgdisk -v` checks (`:363`) close the earlier oracle specification gap.

Minor wording correction: `:353` calls `0x04C11DB7` the reflected polynomial; that is the normal
representation. The reflected implementation uses `0xEDB88320`
(`kernel/src/drivers/block/write_guard_logic.rs:268`). The named CRC-32/ISO-HDLC algorithm and existing
guard reference identify the intended algorithm, so this wording does not fail this item.

## 4. Item 8: interruption and other installer requirements — FAIL on durability; other subitems PASS

- **PASS:** `NEOINST1` is a Limine module, with a compiled-in bundle hash and per-part verification before
  writing (`docs/THINK-2026-10-27-ot87-installer.md:284`). The executable-file/module responses supply
  the running deployment (`:299`), and Install skips consuming the model before copying (`:306`). USB
  removal has a concrete RAM-source explanation and zero-medium-read scenario (`:420`).
- **PASS:** keyboard, touchpad/mouse and console selection, refusal reasons and typed confirmation are
  specified at `docs/THINK-2026-10-27-ot87-installer.md:369`.
- **PASS:** the 2-GiB capacity check occurs before any write, with 1-GiB and one-sector-short refusal
  scenarios (`docs/THINK-2026-10-27-ot87-installer.md:394`).
- **FAIL:** W0-W6 and INCOMPLETE are now explicit, including backup-first GPT construction and restart
  from W3 (`docs/THINK-2026-10-27-ot87-installer.md:402`). But the only ordering guarantee is read-back
  between steps; no persistence barriers are specified. Concrete input: W3-W5 data remains in volatile
  device cache, W6's cleared-INCOMPLETE GPT reaches media, then power is cut. Both GPT copies can be
  valid and complete while ESP/NeoFS payload is missing. This violates `:418`'s bootable-and-mountable
  claim without violating the assumed atomic sector writes.

  **Required correction:** define durable ordering of the recoverable GPT marker before destructive
  payload changes, and persistence of all verified payload/NeoFS state before clearing INCOMPLETE;
  persist the final GPT update before success. Define flush-error handling. The torn-write oracle (`:428`)
  must model cached writes lost/reordered on power failure, not only submission boundaries. Also reconcile
  `:414`'s W6-only retry with `:417`'s unconditional restart from W3: INCOMPLETE alone records no W5
  completion, so it cannot justify a W6-only shortcut for an arbitrary incomplete disk.

## 5. Item 3: Try write-surface audit — FAIL on literal completeness; image A safety spot-check PASS

The image A conclusion is supported: `docs/THINK-2026-10-27-ot87-installer.md:514` identifies the
NeoFS and log-sink production callers. NeoFS already has `mount_readonly`
(`kernel/src/fs/neofs/mount.rs:49`), suppresses its writeback worker for read-only state
(`kernel/src/fs/neofs/inode.rs:114`), and checks write permission on mutations (for example `:187`).
The sink is installed by the test harness (`kernel/src/test_harness.rs:1794`, `:19305`), which is gated
behind `neo-test` (`kernel/src/main.rs:88`); image A's kernel feature list starts at
`tools/xtask/src/main.rs:4972`. J.c.'s read-only Try choice is explicit in the THINK at `:537`.

But the audit says **every non-driver caller** (`docs/THINK-2026-10-27-ot87-installer.md:513`) while
omitting direct harness calls at `kernel/src/test_harness.rs:19299`, `:19338`, `:27669`, `:27933`,
`:28008`, `:28139`, `:28312`, `:31485`, `:33629` and `:34046`. These are safe for image A because the
whole harness is absent, not because they write only through the listed sink. The test-only below-trait
virtio write helper is also worth naming as a driver exception
(`kernel/src/drivers/block/virtio_blk.rs:544`).

**Required correction:** list the harness calls as a grouped excluded surface with the `neo-test` gate,
and make `:524`'s proposed caller check aware of those existing exclusions. This is an audit completeness
finding, not a discovered extra disk-writing path in Try image A.

## 6. J.c.'s decisions and relay #86 item 6 — PASS

Erase requires typed confirmation plus the identity-bound one-shot claim
(`docs/THINK-2026-10-27-ot87-installer.md:135`, `:385`), is isolated to R4b, and requires Muse review
(`:648`). The ordinary Windows-GPT default stays opaque (`:127`), and R3 retains the Windows KAT and
Muse review (`:646`). This passes the policy check; item 2 still fails the detailed claim contract.

The revision explicitly states that VMD invisibility stops today's Zenbook NVMe (`:124`), and a future
VMD dispatch must ship the no-override protected-disk refusal in the same change (`:146`). It correctly
acknowledges that default guard policy and user confirmation alone would then permit erasure (`:143`).
This is a future dispatch requirement, not a claim that the protection table already exists.

Direct rung dependencies are now explicit (`docs/THINK-2026-10-27-ot87-installer.md:633`): R4 follows
R2/R3, R5 follows R4, R4b follows R4, and R7 also requires OT-51 ratification. The relay #86 dependency
wording finding is closed.

## 7. Newly introduced issue — FAIL: unidentified LogansOS reinstall is destructive too

`docs/THINK-2026-10-27-ot87-installer.md:190` allows `blank-or-logansos` installation without stable
identity, explaining that the wrong disk would lose nothing because it is blank. That argument covers only
the blank half of the class. A completed LogansOS volume can contain user data (`:249`).

**Concrete input:** an identity-unavailable virtio disk with an existing completed LogansOS installation and
an `.arthur` memory file is offered Install (`:386`). W3-W5 rewrite the ESP and NeoFS seed (`:411`),
destroying the existing data despite the erase refusal for unidentified disks (`:189`). This is destructive
reinstallation without the stable-identity rule imposed on destructive foreign-disk erasure.

**Required correction:** distinguish genuinely blank disks, incomplete installs and completed LogansOS
volumes. Apply the stable-identity/destructive-confirmation rule to completed-volume reinstallation, or
specify a genuinely data-preserving reinstall operation. Add the existing-LogansOS/no-identity case to
the scenario table. The RAM-source, GPT and input changes introduce no other confirmed finding in this
narrow re-check.

**Disposition:** return to S3 for these bounded corrections. No code changes requested or made by this
review. The independent R1 design remains sound; this RETURN does not certify the later write paths.
