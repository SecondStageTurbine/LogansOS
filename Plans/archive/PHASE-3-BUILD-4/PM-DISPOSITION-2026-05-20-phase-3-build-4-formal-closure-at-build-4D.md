# PM Disposition - Phase 3 BUILD-4 Formal Closure At BUILD-4D

Date: 2026-05-20
From: PM
Status: BUILD-4 CLOSED PER PL OPTION (A) / NO BUILD-4E
Subject: Formal closure of Phase 3 BUILD-4 (Foreign FS read-only + userspace polish) at BUILD-4D, per PL signal 2026-05-20.

Authority:

- `docs/ADR-021-phase-3-design-substrate.md` BUILD-4 boundaries
- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4D-close-consumption.md` §5.2
  (BUILD-4 closure options surfaced; option A recommended)
- PL signal 2026-05-20 directing option (A): "1, go with pm recommendation"
- Bible Article 1.3 (no speculative scope)

## 1. Disposition

BUILD-4 is formally closed at BUILD-4D per PL signoff on option (A). No
BUILD-4E will be opened. The "userspace polish" framing from ADR-021's
BUILD-4 boundaries is folded into Phase 4+ on-demand work, to be opened
when a userspace consumer surfaces that needs the relevant syscalls
(stat-equivalents, additional foreign-FS-level syscalls, etc.).

This matches Bible Article 1.3's discipline against speculative scope:
the right time to add stat-equivalents is when something needs them,
not preemptively.

## 2. BUILD-4 Sub-Build Ledger

| Sub-build | Scope | Status | Substrate commit |
|---|---|---|---|
| BUILD-4A | Directory enumeration + `sys_readdir` | CLOSED | per prior close evidence |
| BUILD-4B | Mount table + `sys_mount`/`sys_umount` + live-handle accounting | CLOSED | `3097126` |
| BUILD-4C | FAT32 read-only foreign filesystem driver | CLOSED | `a627d670` |
| BUILD-4D | ext4 read-only foreign filesystem driver | CLOSED | `aca3fb19` |
| BUILD-4E | (deferred) | NOT OPENED | n/a |

BUILD-4 net result: directory enumeration syscall, mount/umount syscalls,
mount table substrate, three filesystems under a single `MountedFs` trait
(NeoFS, FAT32, ext4) with no trait modification required, foreign-FS
read-only with noexec default + opt-in EXEC, Article 10.3 + ADR-021 Q9
type-import invariant verified end-to-end under both foreign filesystems.

## 3. Deferred-To-Phase-4+ Items

The following items from ADR-021's BUILD-4 framing are explicitly
deferred to Phase 4+ on-demand:

- **stat-equivalent syscalls** for richer file metadata access from
  userspace. Not currently needed; no userspace consumer requires this
  data. Re-opens when network stack or broader userspace exercises
  filesystem-layer metadata.
- **Additional foreign-FS-level POSIX-ish syscalls** beyond what BUILD-4
  delivered. Same condition.

These are forward markers, not commitments. If they never surface as
needed, they never land. Bible Article 1.3 governs.

## 4. Phase 3 Status After BUILD-4 Closure

- **BUILD-1, 2, 3, 4 (A through D):** closed.
- **BUILD-5 (AHCI/NVMe + bare-metal validation):** the only remaining
  Phase 3 lane. Hardware acquisition path in progress on PL side
  (T480s 20L8 path).

Phase 3 is now substantively complete in its substrate work. What
remains is the bare-metal validation lane that closes Phase 3 per
Bible Article 16.3.

## 5. Forward Decisions Queued For PL

### 5.1 BUILD-5 lane structure

BUILD-5 was defined in ADR-021 as "AHCI/NVMe + bare-metal validation."
Two natural ways to structure it:

**Option I — Single BUILD-5, hardware-gated.** Hold all lanes idle
until PL acquires bare-metal hardware (T480s 20L8 or alternative).
When hardware arrives, dispatch BUILD-5 as a single lane covering
AHCI/NVMe driver development AND bare-metal validation.

**Option II — Split BUILD-5a (driver in QEMU) and BUILD-5b (bare-metal
validation).** Open BUILD-5a now: AHCI driver development against
QEMU's AHCI emulation, NVMe driver development against QEMU's NVMe
emulation. BUILD-5b waits for bare-metal hardware to validate the
QEMU-developed drivers run correctly on real silicon.

PM does not push either option. Both are defensible:

- Option I matches the project's general "one lane closes before the
  next opens" discipline and avoids splitting a BUILD that ADR-021
  named as one.
- Option II would maintain agent momentum and let driver-development
  happen now while hardware ships, with the understanding that
  bare-metal validation is the actual closure gate.

If PL wants to keep agents busy, option II is the obvious move. If PL
wants natural pause-and-rest pacing after the intense BUILD-4 cycle,
option I is fine. PM's mild lean is toward option I (rest is
productive; the project has had a high-output day), but neither is
wrong.

### 5.2 BUILD-5 bare-metal target hardware

T480s 20L8 acquisition path remains the front-runner per the
parallel hardware-shopping discussion. The Best Offer at $85 on the
$116 listing was the recommended approach last update; PM has not
heard back on the outcome.

## 6. Lane Status

After this disposition:

- **Cerebro:** idle.
- **Jarvis:** BUILD-4D closed, idle.
- **Prometheus:** idle.

All three agents are idle. No outstanding dispatches. No active stop
conditions. No artifacts await consumption.

This is a natural pause point. Phase 3's substrate work is
substantially complete; what remains (BUILD-5) is hardware-gated and
PL-decision-gated. PM stands ready for PL direction on §5.1 and
§5.2; otherwise the project rests in stable state.

## 7. Day Summary

For the record: 2026-05-20 closed the following lanes:

- BUILD-4B (mount table + mount syscalls; commit `3097126`)
- BUILD-4C (FAT32 read-only; commit `a627d670`)
- BUILD-4D (ext4 read-only; commit `aca3fb19`)
- Jarvis FD-teardown interleaving audit
- ANSPEC-002 v0.1.1 amendment ratification
- Prometheus three-item follow-up bundle (ADR-022 forward marker,
  ANSPEC-001 §13 Q6 KernelAttestation, doc hygiene sweep)
- ADR-022 PL ratification
- This BUILD-4 formal closure disposition

Three sub-builds of BUILD-4 closed in one day. Two foreign-filesystem
drivers landed under the BUILD-4B substrate without trait
modification. Two stop conditions surfaced and handled cleanly
(mtools-in-container clarification, Prometheus wrong-file). One ADR
ratified. One major spec amendment (ANSPEC-002 v0.1.1) ratified. Zero
substantive process problems.
