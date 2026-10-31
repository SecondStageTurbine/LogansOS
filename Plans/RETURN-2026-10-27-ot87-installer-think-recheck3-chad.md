<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-87 installer THINK revision 4 re-check

**Relay:** #90. **Reviewer:** Chad, Astra review lane.
**Reviewed SHA:** `13df63606fee900b18a8978f320702812baede8e` (revision 4 over `1c68712b`).
**Worktree:** `E:\AI Automation\workers\chad`.
**Scope:** relay #88 findings plus the new prior-art survey, per the relay #90 dispatch.

**Overall verdict: FAIL, limited to the new FAT-formatting plan in §11.** The relay #88 blockers are
closed at design level. The survey introduces an on-device ESP format in W4 that conflicts with the
prebuilt ESP, slot map and W3/W4 state machine. Reconcile that single design choice before R4 dispatch.
This is not another failure of the corrected identity or persistence contracts.

RAG health was `healthy`, with Ollama and Qdrant both true. Reviewed the revision diff, affected sections,
the dispatch and the existing relay #88 findings; directly checked the GPT parser's fallback behavior.
Graft discovery returned unrelated symbols. Upstream spot-checks used GitHub's API and raw files at
the recorded commits; the web tool could not open the API URLs, so PowerShell fetched them directly.
Only the THINK changed in the reviewed revision. No build or tests run; the traces below are specification
replays, not executed test results. R1 is outside this review.

## 1. W1 and every GPT copy's ordering — PASS

`docs/THINK-2026-10-27-ot87-installer.md:475` requires array write, successful flush and read-back
before submitting its header, followed by header flush/read-back. This explicitly covers W1, W2 and
both W6 copies; the state table agrees at `:507`, `:508` and `:512`.

**Replay: header alone survives on a blank disk.** If the array flush has not succeeded, the header is
never submitted. If the header is submitted and survives, its array is already durable. A cut before the
header leaves no GPT signature and the zero first MiB remains blank; a cut after the header yields a
valid INCOMPLETE backup. The negative-control oracle at `:538` explicitly removes the barrier and
expects the old `opaque-gpt-invalid` failure. The relay #88 input is closed.

**Erase during W2:** W1's new backup is already durable (`:478`). When W2 overwrites a foreign
primary's array before replacing its header, the old header can temporarily fail the array CRC. The parser
then tries the independently validated backup (`kernel/src/drivers/block/write_guard_logic.rs:337`,
`:348`), which supplies the INCOMPLETE layout. An old primary that remains valid until replacement
can still describe the old disk; no new partition payload is written before the new primary is durable
(`THINK:479`). Thus the requested foreign-header/our-array interruption does not strand the target or
expose a partially installed payload as complete.

Wording qualification: `THINK:477` should say a **newly submitted** header never precedes its matching
durable array. An old header can temporarily mismatch the replacement array; the other valid GPT copy
is what makes that interval recoverable. This does not invalidate the specified write order.

## 2. Failed-flush model — PASS

`docs/THINK-2026-10-27-ot87-installer.md:484` now permits any persisted subset after a failed flush.
It separates stopping further submissions (`:487`) from uncertainty about already submitted markers
(`:489`). The UI withholds success, names the uncertain step and notes that W6 may already have
installed the disk (`:496`). The oracle applies the same subset model and stops submitting (`:535`).

**Replay: W6 backup-header flush fails after persisting its writes.** Its array was already flushed before
the header was submitted; W3-W5 payload was flushed before W6 (`:481`). The primary still says
INCOMPLETE and is preferred, while the backup may say complete. No primary update follows that failed
flush. If an error instead occurs during a primary update reached after successful backup completion,
an invalid primary falls back to the complete backup, or a valid complete primary wins. Any resulting
installed classification refers to fully durable payload. The installer still reports uncertainty, not success.
The prior all-or-nothing flush-error model is gone.

The phrase “after a later primary attempt” in the named trace (`:542`) must be read as a separate trace
or later restarted attempt, not permission to continue after the failed backup flush; rule `:487` is explicit.

## 3. Unreported-swap bound — PASS

`docs/THINK-2026-10-27-ot87-installer.md:209` retains the reported-swap bound of requests in flight
before the next check. `:213` makes reliable media-change reporting a prerequisite and names NVMe,
AHCI and virtio reporting as R4 work. `:216` explicitly gives an unreported swap no guaranteed bound.

**Replay of the masked W2 swap:** B reports A's identity, shares the expected last MiB, and differs in
the first MiB only where W2 overwrites it. With no media-generation event, W2 makes both sampled
windows match and subsequent boundary checks can pass. The revised text says B can remain undetected
for the rest of the install (`:220`), and the host-KAT row now expects exactly that outcome (`:249`).
This is a truthful limit, not a promise that fingerprints establish physical identity.

The target/link statement at `:226` is an implementation prerequisite to establish per driver, not evidence
that today's drivers already supply it. Review of the R4 implementation must enforce that prerequisite.
No additional unconditional guarantee is needed to close the specific relay #88 finding.

## 4. R4b kill-switch nit — PASS

`docs/THINK-2026-10-27-ot87-installer.md:783` now says disabling `installer-erase` restores R4's
**blank-or-incomplete** rule. It no longer permits reinstalling a completed LogansOS volume in R4.

## 5. Prior-art survey — FAIL on the borrow's integration; metadata and license policy PASS

All ten candidate rows have an approach, license, commit SHA and date (`THINK:832`, `:844`). The
check date and preservation of upstream license/URL/commit are explicit at `:823`. The survey table is
carried here for review; only the three marked “verified” were independently checked against upstream.

| Candidate | Approach / intended use | License recorded | Commit and date in survey | Check |
|---|---|---|---|---|
| Calamares | Modular Qt installer; design only | GPL-3.0-or-later | `95aa33fd`, 2025-05-06 | Row present |
| Subiquity | UI/config separated from executor; design only | AGPL-3.0-or-later / GPL-3.0-only | `5aad030a`, 2026-10-23 | Row present |
| curtin | Ordered storage executor; design only | AGPL-3.0-only | `93c36fb6`, 2026-10-09 | Row present |
| systemd-repart | Declarative partition growth; design only | LGPL-2.1-or-later | `a09e5f55`, 2026-10-26 | Row present |
| Redox installer | Rust GPT/FAT plus custom filesystem; design only | MIT | `b5a11310`, 2026-09-01 | Verified |
| Haiku Installer | Copies live system; design only | MIT, per-file qualification | `a12d4cd1`, 2026-10-26 | Row present |
| Limine | Existing bootloader; no new vendor | BSD-2-Clause | `8ad14430`, 2026-10-19 | Row present |
| gpt | Independent host reader | MIT | `73086def`, 2026-08-19 | Verified |
| gptman | Alternate host reader | MIT OR Apache-2.0 | `4fc144e1`, 2026-04-28 | Row present |
| fatfs | FAT formatter candidate; integration needs correction | MIT | `38b3a795`, 2026-10-16 | Verified |

**Upstream spot-checks, 2026-10-27:**

- **fatfs:** default `master` tip is `38b3a7950bf0e765e34286d1dfe76c519ee347e2`, committed
  2026-10-16; repository is not archived. The [pinned manifest](https://github.com/rafalh/rust-fatfs/blob/38b3a7950bf0e765e34286d1dfe76c519ee347e2/Cargo.toml)
  and [license](https://github.com/rafalh/rust-fatfs/blob/38b3a7950bf0e765e34286d1dfe76c519ee347e2/LICENSE.txt)
  confirm MIT; `src/lib.rs:47` supports `no_std`. Default features include `std` and optional `chrono`,
  so a kernel integration would need explicit feature selection, not default features.
- **gpt:** default `master` tip is `73086deff58cb3c741b2ba51a53a2a7c6231a3f1`; repository is not
  archived. [Manifest](https://github.com/Quyzi/gpt/blob/73086deff58cb3c741b2ba51a53a2a7c6231a3f1/Cargo.toml)
  and [license](https://github.com/Quyzi/gpt/blob/73086deff58cb3c741b2ba51a53a2a7c6231a3f1/LICENSE)
  confirm v4.1.0 and MIT. The API reports August 18 at 22:10 UTC; the original
  [commit patch](https://github.com/Quyzi/gpt/commit/73086deff58cb3c741b2ba51a53a2a7c6231a3f1.patch)
  records August 19 at 00:10 +0200. The survey's date is correct in that timezone.
- **Redox installer:** GitHub mirror default `master` tip is
  `b5a11310e1ac915494efea6d18e242966da74f3f`, committed 2026-09-01; mirror is not archived.
  [License](https://github.com/redox-os/installer/blob/b5a11310e1ac915494efea6d18e242966da74f3f/LICENSE)
  is MIT. The [pinned installer source](https://github.com/redox-os/installer/blob/b5a11310e1ac915494efea6d18e242966da74f3f/src/installer.rs#L577)
  confirms the GPT path rejects non-512-byte sectors. This verifies the stated mirror evidence, not
  independent freshness of the GitLab upstream.

The planned code reuse is within the repository's stated license allowlist: MIT `fatfs`, and MIT `gpt`
as a host dependency. GPL/AGPL/LGPL rows are explicitly design-only. Nothing was actually vendored by
this revision. Keep notices/copyright and record the selected pin in the later vendoring change.

**Missing survey topics (scope observations, not demands for new R4 features):** explicit treatment of
encrypted installs/unlock or recovery credentials, and the firmware boot-trust/Secure Boot prerequisite.
For example, [Subiquity's security overview](https://canonical-subiquity.readthedocs-hosted.com/en/latest/explanation/security-overview.html)
describes encrypted installations and installation-time security updates. OT-87 can defer those capabilities,
but the survey should record that disposition rather than imply the surveyed shapes cover them. The local
Zenbook runbook already records Secure Boot off (`docs/runbook-baremetal-zenbook-s14.md:22`); distinguish
that test prerequisite from general-PC support. Alongside/manual partitioning is already explicitly deferred
in the Calamares row, so its absence from R4 is not a new finding.

## 6. New revision-4 issue — FAIL: W4 cannot also format the ESP as currently specified

`docs/THINK-2026-10-27-ot87-installer.md:846` and `:861` direct R4 to vendor `fatfs` for W4's
FAT32 format. That contradicts the unchanged design:

- §4.3 supplies two preformatted, populated ESP templates and their recorded contiguous slot maps (`:336`).
- W3 writes that ESP and slot A (`:509`); W4 writes/grows the NeoFS seed in partition 2 (`:510`).
- The kernel explicitly never formats FAT (`:551`), and §11 nevertheless says §4-§7 do not change (`:858`).
- The curtin row also attributes clearing NeoFS superblocks to W3 (`:834`), although W3 operates on the ESP.

**Concrete input and wrong result:** follow W3, writing and verifying the supplied ESP with Limine,
configuration and slot A; then follow §11's instruction to format that ESP in W4. `fatfs::format_volume`
initializes FAT and root-directory metadata ([pinned `src/fs.rs:1200`](https://github.com/rafalh/rust-fatfs/blob/38b3a7950bf0e765e34286d1dfe76c519ee347e2/src/fs.rs#L1200),
with zeroing at `:1240`, `:1253`, `:1265`). The existing file directory entries/chains and host-generated
slot map are no longer valid. Later W5-W6 do not restore the entire bootloader/files/preallocation, so the
disk cannot satisfy the specified boot gate. If §11 instead means format NeoFS's partition as FAT, it
directly violates W4's filesystem layout. The instructions do not define a coherent implementation.

**Smallest correction:** place the `fatfs` borrow in the **host ESP-template builder**, replacing `mformat`
there if desired. Generate the files, verify contiguous allocation and regenerate the slot map from that
output; preserve the bundle pin and W3 copy/W4 NeoFS split. Update §4.3, the R4 row and §11 consistently.
If on-device formatting is actually intended, amend the state machine, file population, slot-map provenance
and recovery gates explicitly instead of asserting that §4-§7 are unchanged. A survey does not require
introducing an otherwise unnecessary kernel formatter.

**Disposition:** relay #88 identity/persistence findings closed. Return the new §11 integration conflict
for a bounded correction; no implementation or R1 change requested by this review.
