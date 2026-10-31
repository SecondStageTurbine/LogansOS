<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-87 installer THINK second narrow re-check

**Relay:** #88. **Reviewer:** Chad, Codex family.
**Reviewed SHA:** `1c68712b54b765494c31be05ddd2f0218b3c9044` (revision 3 over `1f313ac6`).
**Worktree:** `E:\AI Automation\workers\chad`.
**Scope:** relay #87 findings and the five relay #88 checklist items. R1 is outside this re-check.

**Overall verdict: FAIL.** Revision 3 closes the original two payload-persistence counterexamples, the
audit omission and the unidentified completed-volume reinstall case. The identity exposure bound still
has a counterexample, and W1 still has a recoverability gap under the newly specified persistence model.

RAG health: `healthy`, `ollama=true`, `qdrant=true`. Read the dispatch and relay #87 RETURN from
`origin/main`; read the revision diff and affected THINK sections directly. Graft returned unrelated symbols.
Exact caller searches and direct reads checked the audit gates and GPT recovery parser at the reviewed SHA.
The replays below are specification traces, not executed tests. No build or tests run (review-only dispatch).

## 1. Durability — FAIL; the two original payload power-cut inputs now PASS

**Resolved contracts:**

- `docs/THINK-2026-10-27-ot87-installer.md:454` refuses devices without flush before writing.
  `:456` defines write, flush, read-back and step-boundary identity checks for each W-step.
  `:465` stops on flush error without proceeding to the next step.
- The recoverable INCOMPLETE GPT is flushed before W3 changes partition contents (`:459`), W3-W5
  are flushed before W6 (`:462`), and W6 flushes each GPT copy before reporting success (`:463`).
- Finish installing unconditionally re-runs W3-W6 (`:483`), removing the W6-only shortcut.
- The oracle now models arbitrary subsets/order of unflushed writes surviving power loss (`:497`).
- Activation flushes inactive-slot data and FileSize metadata before the pointer flip, and flushes the flip
  before success (`:671`). A failed pre-flip flush prevents activation; a failed post-flip flush reports an
  uncertain outcome (`:677`). The slot writer uses the same cache model (`:682`).

**Replay of relay #87's activation input:** cut power while slot data remains volatile. The required
pre-flip flush has not succeeded, so the flip is not issued. If the flip is issued, slot data and FileSize
metadata have already survived a successful flush. An old or new atomic commit sector selects a complete
slot. This counterexample is closed (`docs/THINK-2026-10-27-ot87-installer.md:673`).

**Replay of relay #87's install input:** W3-W5 data remains volatile and W6 attempts to clear INCOMPLETE.
The barriers now prohibit entering W6 in that state. After W6 starts, all payload is durable; a completed
GPT cannot get ahead of that payload. This counterexample is closed (`:462`).

**Remaining FAIL: W1 needs an internal array-before-header persistence barrier.**
`docs/THINK-2026-10-27-ot87-installer.md:476` writes the backup array and then its header, but `:459`
requires a flush only after the header. Submission order does not constrain the survival order of those
unflushed sectors under the model at `:498`.

**Concrete input and wrong result:** a blank 4-GiB, 512-byte-sector target; W1 submits the 32 backup-array
sectors and then the valid INCOMPLETE backup-header sector. Cut power before W1's flush; let only the
header persist. It has a valid header CRC and GPT signature, but its entry-array CRC does not match the
still-zero array. There is no primary GPT. The shared parser source tries the backup
(`kernel/src/drivers/block/write_guard_logic.rs:337`), rejects its array (`:503`), and classifies the disk
`opaque-gpt-invalid` (`:355`). This is neither the no-table blank class (`THINK:411`) nor valid INCOMPLETE
(`:412`). It contradicts W1's claimed outcomes and `:494`'s no-stranded-disk explanation. If the device
also lacks stable identity, it cannot use destructive erase to recover (`:215`).

**Correction:** persist and verify the backup entry array before submitting the header that makes it
recognisable, then persist/verify the header. Alternatively specify another recoverable protocol that keeps
the unknown-GPT refusal intact. Add the header-only-survives trace to the oracle.

**Flush-error model also needs correction:** `docs/THINK-2026-10-27-ot87-installer.md:503` models a
failed flush as persisting none of its covered writes. Failure does not establish that none persisted.
For example, let W6's complete backup GPT reach media and its flush return an error. The disk has an
advanced backup marker despite the failure; software can stop and withhold success, but cannot promise
the on-disk marker never advanced (`:465`, `:469`). Model arbitrary persisted subsets on failed flush too,
and distinguish no further marker submission from an uncertain outcome of a marker already submitted.

## 2. Identity — FAIL on the claimed bound for an unreported swap

**Resolved mechanics:** `docs/THINK-2026-10-27-ot87-installer.md:187` binds every install write to a
`ClaimedWriter` holding the device `Arc`. A registry slot change cannot redirect that reference. The
per-request generation checks run before submission and after completion (`:197`); identity and the
expected, overlaid first/last-MiB fingerprint recur at each flushed step boundary (`:179`, `:202`).
The exposure bound is explicit at `:210`, and tests for all three swap timings are listed at `:231`.

**Replay after the first write:** replacing the registry slot alone leaves the writer on its original device.
A reported medium change behind that object is rejected before the next submission or detected after
the current completion. An unreported change with sampled bytes still differing at the boundary stops
there. These specified cases close the corresponding relay #87 omissions.

**Replay between validation and submission:** a generation bump after the pre-check may permit the
current request to reach the replacement, but the post-check stops further writes (`:200`, `:232`). The
revision correctly admits that in-flight exposure; it does not promise zero writes in this case.

**Remaining counterexample: an unreported swap can evade more than one boundary.** The assertion at
`docs/THINK-2026-10-27-ot87-installer.md:208` assumes a replacement cannot carry the expected sampled
bytes. It can, and the installer's current step can overwrite the very bytes that would distinguish it.

**Concrete input and wrong result:** prepare two same-size, same-serial/model test disks A and B. A is
the selected disk after W1. B has the same last MiB as A at that point; its first MiB differs from A only in
the PMBR/primary-array/primary-header sectors that W2 will overwrite. B also has different data in the
interior. Swap to B behind the same device object immediately before W2's first write, without a driver
event. All generation checks pass. W2 overwrites every differing sampled byte with the expected output;
after its flush, both fingerprint windows and reported identity match. W3's boundary also passes, W4
overwrites B's NeoFS region, and subsequent checks can continue passing. The swap was never detected,
and exposure exceeded the claimed one step (`:212`). This is equality of sampled bytes, not a hash collision.

**Correction:** either make reliable media-change reporting a prerequisite for a guaranteed replacement
bound, or explicitly state that an unreported replacement matching the observed identity and sampled
bytes can remain undetected for the rest of the install. Fingerprints are useful change detectors, but
cannot establish the unconditional bound now claimed. Add the W2-masks-the-differences trace to the
host model; retain the existing reported-swap and ordinary differing-fingerprint cases.

## 3. Try audit — PASS

`docs/THINK-2026-10-27-ot87-installer.md:595` now lists all ten direct harness calls reported in relay
#87, with the whole-module `neo-test` exclusion. Exact search at this SHA confirms those call locations.
The enclosing gate is `kernel/src/main.rs:88`. The below-trait virtio helper is listed separately
(`THINK:600`), with its actual gate and explicit guard check
(`kernel/src/drivers/block/virtio_blk.rs:544`, `:553`). The proposed CI allowlist must verify the
test-only gates and reject a new caller or a caller losing its gate (`THINK:609`). This closes the audit
completeness finding; the CI mechanism remains an implementation deliverable.

## 4. Completed-disk reinstall — PASS

`docs/THINK-2026-10-27-ot87-installer.md:410` separates blank, incomplete and installed disks. Installed
reinstall is explicitly destructive and needs stable identity plus the typed code (`:426`). The claim
refuses every destructive install without stable identity (`:214`). The exact prior input—unidentified
completed LogansOS disk containing an `.arthur` memory file—is now a refusal/bit-identical scenario
(`:234`, `:743`). R4 accepts fresh targets only and explicitly refuses completed disks (`:742`); R4b
carries destructive reinstall (`:743`). This closes relay #87 item 7.

Minor consistency edit: R4b's kill-switch cell at `:743` still says off restores the old
`blank-or-LogansOS` rule. It should say R4's **blank-or-incomplete** rule, matching the revised R4 row.

## 5. Anything new in revision 3 — FAIL, limited to the findings above

The W1 header-only persistence outcome and failed-flush model are concrete remaining gaps in the
new durability contract (item 1). The one-step unreported-swap bound is false for the concrete input in
item 2. No additional confirmed issue was found in the other revised sections in this narrow review.
The CRC polynomial wording correction also closes relay #87's minor GPT wording note (`THINK:387`).

**Disposition:** return these bounded contract findings to the PL/S3. Per the relay #88 dispatch, another
review round on the same contracts routes to GPT-6-Astra. No implementation changed; no R1 hold implied.
