<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM HANDOVER: WiFi, OT-61 browser, and LogansOS 1.0

**Handover date:** 2026-07-25
**Repository:** `SecondStageTurbine/NeoPlutarch`
**Workspace:** `E:\AI Automation\AltimitNeo`
**PM governance worktree:** `E:\AI Automation\AltimitNeo\Worktrees\pm-governance`
**PM branch:** `codex/pm-mt7601u-warm-reset-qualification`
**Handover base (`origin/main` before this document):**
`3d7a1b766167a00e408a34a778250c1e6f30bb79`
**Overall state:** **No merge, hardware boot, reset, credential build, or gate
rerun is currently authorized.**

## 1. PM operating contract

The PM does not write production or test code.

The PM:

- independently verifies coder RETURNs;
- authors governance dispatches and dispositions;
- pushes governance documents to `origin/main`;
- grants single-use exact-SHA runner and hardware tokens;
- independently verifies GitHub Actions and silicon evidence;
- performs verify-before-merge only after every required gate is green.

All implementation is assigned to coders such as Jarvis, Cerebro, Chad,
Oracle, or Janus. Prometheus is **research and independent evidence only**.
Do not assign production or test-code implementation to Prometheus.

Never accept a RETURN because it says `TOKEN-OK`. Recompute identity, scope,
hashes, branch parity, CI SHA, and the evidence material to the decision.

Never merge while a required gate is RED. Never convert an unrelated or flaky
failure into green by assertion; establish artifact parity and controlled
reproduction, then authorize a fresh gate explicitly.

## 2. User's actual LogansOS 1.0 goal

The user's definition of a useful 1.0 is:

1. install LogansOS on a PC;
2. log into the desktop;
3. browse the Internet;
4. write a report;
5. print it.

That is the product-level finish line. WiFi is only one critical blocker.

LogansOS is intentionally non-POSIX. Do not introduce a general Linux or POSIX
personality as a shortcut. The browser program is OT-61: a bounded native
Servo + SpiderMonkey stack using private PAL and LogansOS capability/ABI
semantics. Graphics terminates through SWGL into Atrium's
`SharedPixelBuffer`. Read `docs/ADR-078-ot61-browser-c-ladder.md`.

There is not yet a complete, independently verified 1.0 release ledger for all
five user journeys. After the active WiFi and browser boundaries are cleared,
the PM must create that ledger and explicitly audit installer, login/session,
document editing, persistence/export, printer discovery/driver/spooler, and a
physical end-to-end install-to-print run. Do not claim the whole OS is near
100% merely because kernel or WiFi gates are green.

## 3. Immediate first action for the incoming PM

Read the current research dispatch:

```powershell
git fetch origin
git show origin/main:Plans/HANDOVER-2026-07-25-PM-WIFI-OT61-LOGANSOS-1.0.md
git show origin/main:Plans/RESEARCH-DISPATCH-2026-07-25-f4a0940-arthur-reversibility-red-prometheus.md
```

Wait for Prometheus's research-only return on the failed
`arthur_reversibility` gate. Do **not** rerun GitHub Actions while waiting.

The next PM decision is:

- if Prometheus proves scenario-specific artifact parity and a
  `PREEXISTING-FLAKE` or supported `RUNNER-SPECIFIC` verdict, independently
  verify it and publish one new exact-SHA four-shard token;
- if the result is `CANDIDATE-RELATED`, dispatch the responsible coder without
  changing the frozen candidate;
- if it is `INCONCLUSIVE`, keep all holds and request the missing bounded
  discriminator. Do not authorize a blind rerun.

## 4. Active WiFi candidate

| Field | Value |
|---|---|
| Branch | `codex/mt7601u-post-qselect-terminal-jarvis` |
| Candidate | `f4a0940872e865207128a240587f7cac036f0cf9` |
| Parent | `8ab6409fe711e531c744d0296f8855beab7b7b3a` |
| Child stat | exactly 5 files, `+602/-111` |
| Push CI | `30159747107`, exact SHA, attempt 1, green |
| Current disposition | frozen; four-shard RED; no hardware or merge |

The five child paths are:

- `kernel/src/drivers/usb/mt7601u/linux_parity_logic.rs`
- `kernel/src/drivers/usb/mt7601u/mod.rs`
- `kernel/src/test_harness.rs`
- `tools/ai-ops-tests/src/lib.rs`
- `tools/ai-ops-tests/tests/mt7601u_hostile_oracle.rs`

The repair:

- restores the Linux post-`Q_SELECT` initialization order;
- places one non-replenishing 250,000-charge operation ledger around MAC init
  through successful EEPROM acquisition;
- emits a staged, evidence-neutral terminal with response/RX/budget/fence
  evidence;
- latches and blocks both MCU and direct-register traffic after terminal;
- flushes durable evidence before and after BBP, WCID, and EEPROM stages;
- does not add a firmware reload, baked channel, or MCU-death claim.

Jarvis is the WiFi implementation owner.

## 5. Accepted WiFi review evidence

The following reviews are accepted for exact `f4a0940`:

- Oracle Lane A: `TOKEN-OK` for driver/Linux-order correctness;
- Cerebro Lane B: `TOKEN-OK` for contract, bounded ledger, sink, terminal,
  post-terminal gate, and negative controls;
- Prometheus Lane C v2: artifact substance accepted after independent PM
  verification.

The PM independently verified the three normalized `!mt7601u` pairs:

| Configuration | Bytes per side | Paired SHA-256 |
|---|---:|---|
| default | `3,139,424` | `6e1a3c1adffaf4fd79db542a5d5158ef9a02884385663fdd30282fb4c6156f86` |
| `usb-host` | `3,164,712` | `e9a87f553e144de6426cb4bcb258d2a60828b7f10d8d9abc7c6c797c6007e77c` |
| `usb-hid` | `3,185,208` | `eb7bcf2f73eb320c3c7068ce6518a1341157a2899e446cb92597e21b586812a9` |

Every pair is byte-identical and remains an ELF. Candidate OFF artifacts have
zero matches for `mt7601u`, `operation_budget`, `linux_parity`,
`post-qselect`, and `post_qselect`.

Prometheus reported alternate stable patch-ID values without showing them and
attributed the difference to Git version. The PM did not adopt that claim.
Independent Git `2.55.0.windows.3` recomputation reproduces the values pinned
in:

```text
Plans/EVIDENCE-FIX-FORWARD-2026-07-25-mt7601u-f4a0940-lane-c-prometheus.md
```

Pairwise replay equivalence and ten-commit patch-ID uniqueness are independently
confirmed. Do not reopen this metadata issue unless new actual patch content
differs.

## 6. Exact blank-credential WiFi ISO

Jarvis's immutable blank-credential artifact is:

```text
E:\AI Automation\AltimitNeo\Workers\jarvis\boot\neoplutarch.iso
```

Verified identity:

- size: `126,660,608` bytes;
- SHA-256:
  `feadfdafdf24d110ccc967d14a53c0b2649b3b5ecc1d61e686ab03b402fb1149`;
- kernel entry: `/boot/kernel`, `35,529,688` bytes, SHA-256
  `bdbcf8fcba9df356fe217af38a4400e67003800df5ac77f6adefbb1e4388bf36`;
- firmware entry: `/modules/mt7601u-fw`, `45,412` bytes, SHA-256
  `4511b1d840e22aea2bf5fdca419c91c0d94cbfb291b9ac4f8be6d9100d1a7046`;
- `limine.conf` names the firmware module;
- volume ID `ISOIMAGE`;
- publisher, application, and system IDs blank;
- preparer:
  `XORRISO-1.5.6 2023.06.07.180001, LIBISOBURN-1.5.6, LIBISOFS-1.5.6, LIBBURN-1.5.6`;
- committed SSID and passphrase placeholders are `b""`;
- `CHANNEL=0`;
- neither locally supplied credential string occurs in the ISO.

This ISO is not hardware-authorized and contains no usable network
credentials. Do not burn or boot it merely because it exists.

## 7. Current four-shard RED

The single-use token was consumed by:

```text
Run: 30181808389
URL: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30181808389
Event: workflow_dispatch
Attempt: 1
Branch: codex/mt7601u-post-qselect-terminal-jarvis
SHA: f4a0940872e865207128a240587f7cac036f0cf9
```

Results:

- Shard 1: FAIL;
- Shards 2, 3, and 4: PASS;
- host tests/format/lint: PASS;
- SPDX: PASS;
- monolithic identity: PASS;
- overall: FAILURE.

Shard 1 failed only because `arthur_reversibility` timed out after all four
built-in 420-second attempts. The job was on `ryzen-4`, with 94.3 GiB visible
memory and 1,345.1 GiB free Docker-host disk. Capacity preflights passed.

The four attempts stopped at different points:

- attempt 1 reached the `write_file` agentic AST;
- attempts 2 and 3 emitted successful `write_file` accounting;
- attempt 4 reached the next model action and mask/candidate setup.

The post-failure artifact upload then hit GitHub storage quota. That did not
cause the timeout, but it prevented durable artifact upload.

This scenario is demonstrably pre-existing and flaky:

- green run `30135801447` at `625e531f...`, `ryzen-3`: attempt 1 timed out;
  attempt 2 passed in 10.8 seconds;
- green run `30104558141` at `2d43670e...`, `ryzen-5`: attempt 1 timed out;
  attempt 2 passed in 14.6 seconds.

That history is evidence for investigation, not permission to mark the RED
gate green. The controlling research dispatch requires scenario-specific
artifact equality and controlled comparator/candidate reproduction:

```text
Plans/RESEARCH-DISPATCH-2026-07-25-f4a0940-arthur-reversibility-red-prometheus.md
```

Prometheus may research and build comparison evidence only. Prometheus must
not change code, commit, push, or dispatch a workflow.

## 8. WiFi silicon history and why `f4a0940` exists

Earlier TEST 3 results:

1. A cold scan completed channels 1-6 and found eight BSSIDs, then timed out
   on channel 7's first MCU frequency-plan `RANDOM_WRITE`. A forced firmware
   reload stalled at ILM byte 0. DHCP and LINK UP were never reached.
2. The immediate second boot performed a full cold firmware upload rather than
   the expected resident-firmware path, disproving that warm-proof procedure.
3. Oracle established that missing ZLP is not the proximate cause. The
   frequency-plan frame is 40 bytes and the firmware chunk frame is 14,344
   bytes; neither is a 512-byte multiple. Forced reload and "MCU death" claims
   are not supported.
4. Candidate `625e531f...` then performed one hardware boot. Despite the cold
   procedure, the dongle reported `COM_REG0=1`, took the WARM-RESYNC path, and
   skipped upload. It progressed through `MCU_Q_SELECT`, then emitted nothing
   for nearly seven minutes before the first MAC-init `RANDOM_WRITE`.
5. `8ab6409f...` restored the missing Linux post-`Q_SELECT` initialization.
   Child `f4a0940...` bounded and instrumented that entire stage.

No `f4a0940` silicon boot has occurred.

The next silicon run, if eventually authorized, must test real
`CHANNEL=0` scan, AP discovery, association, deferred `CAL_BW`, WPA/GTK, DHCP,
nonzero IPv4, `LINK UP`, exactly one terminal, durable sink agreement, and
explicit `fence_refusals=0`. No baked-channel fallback is allowed.

## 9. Credential discipline

WiFi credentials must never enter Git history or a governance document.
The user supplies them locally when a hardware token is granted.

The only permitted fill location is:

```text
kernel/src/drivers/usb/mt7601u/mod.rs
```

Required procedure:

1. prove exact candidate branch/local/remote parity and clean worktree;
2. fill only SSID and passphrase locally; keep `CHANNEL=0`;
3. require `git diff --stat` to name only `mod.rs`;
4. require no staged credential hunk;
5. never `git add`, commit, push, or `git stash` the filled file;
6. build and record ISO identity;
7. immediately restore:

```powershell
git checkout -- kernel/src/drivers/usb/mt7601u/mod.rs
```

8. prove SSID/passphrase are back to `b""`, `CHANNEL=0`, staged diff empty,
   worktree clean, and branch tips unchanged.

Before sharing serial or sink evidence, grep both for SSID and passphrase.
The passphrase must not appear. A scan-result SSID may appear; disclose it
consciously rather than accidentally.

## 10. Rig and live-Ubuntu constraints

Last known rig facts:

- Ubuntu SSH address: `<optiplex-ip>`;
- the live USB regenerates SSH host keys on boot;
- when the warning recurs:

```powershell
ssh-keygen -R <optiplex-ip>
```

- physical ownership of the box/LAN lease is the trust basis;
- modern `scp` fails because the live session lacks an SFTP subsystem;
  use `scp -O`;
- SSH was enabled through `multi-user.target.wants`, but if it does not return,
  start `ssh` physically at the box;
- the MT7601U enumerates cleanly as high-speed USB `148f:7601`;
- do not diagnose an empty scan as "dongle absent" without new contrary
  enumeration evidence.

Storage:

- `/dev/nvme0n1` is the confirmed internal NVMe boot target and evidence sink;
- the ISO burn covers only the leading portion of the NVMe;
- the evidence sink lives at offset 256 MiB and must be cleared before a newly
  authorized boot:

```bash
sudo -n dd if=/dev/zero of=/dev/nvme0n1 bs=1M seek=256 count=16 conv=notrunc,fsync
sync
```

- `/dev/sda` contains an Arthur NeoFS/model volume and is not a target;
- confirm the manifest omits filler disks and does not stage anything to
  `/dev/sda` before any `dd`;
- preserve prior PuTTY capture, then start
  `C:\Users\<user>\neoplutarch-serial.log` fresh, not append;
- let the test reach a bounded terminal. Do not halt because a stage appears
  stalled.

Do not assume the physical rig is still in the last reported state. Before the
next authorized run, re-inventory `lsblk`, branch/ISO identity, sink contents,
serial logging, USB enumeration, and SSH state.

## 11. Banked warm-reset qualification

The reset actuator is accepted and banked, not integrated:

| Field | Value |
|---|---|
| Branch | `codex/mt7601u-warm-reset-fresh-base-chad` |
| Candidate | `fb2830d432ae5bf63214d69fb6541af4f599761f` |
| Base | `856874182b88679462f1a9d206726cfa3ff5eb1f` |
| Push CI | `30139397576`, green |
| Disposition | code/evidence TOKEN-OK; banked |

Read:

```text
Plans/DISPOSITION-2026-07-24-mt7601u-warm-reset-qualified-banked.md
```

The actuator:

- discovers and validates the live ACPI FADT through XSDT/RSDT;
- is reachable only under the exact qualification feature triple;
- runs only after the authorized association PASS;
- flushes the final PASS and derived breadcrumb;
- performs exactly one FADT-derived `outb`;
- has no hard-coded OptiPlex tuple or fallback;
- logs, flushes, and halts if the write unexpectedly returns.

Its seven normalized reset-OFF pairs were independently accepted.

Do not cherry-pick or merge it into `f4a0940` yet. If cold WiFi succeeds and
immediate-warm residency remains an acceptance requirement, dispatch a fresh
integration child from the accepted WiFi candidate, then repeat code review,
OFF proofs, exact-SHA push CI, four-shard gate, and a separately authorized
cold-to-immediate-warm silicon sequence.

## 12. Active OT-61 browser lane

Exactly one parallel implementation lane was opened while WiFi was waiting:
Cerebro on OT-61 C1.1.

| Field | Value |
|---|---|
| Branch | `codex/ot61-c1.1-aag-fix-forward-cerebro` |
| Returned branch tip | `61d50a9951472b876389cf53b525d5222f7ac764` |
| Provisional code candidate | `a061bd6fd92476c8c5fd24079546b18516458ce4` |
| Authorized base | `87d4244fba43d936892eb3307b7155dc303a449c` |
| Candidate push CI | `30173847819`, exact SHA, attempt 1, green |
| Candidate stat | 21 files, `+4010/-87` |
| Disposition | evidence/scope BOUNCE; no runner token |

Do **not** use branch tip `61d50a99...` as the browser code candidate. Its only
delta from `a061bd6f...` is an unrequested 57-line
`scripts/worker-close-gate.sh` timeout change. That tooling finding is banked
separately. It is excluded from C1.1.

Cerebro still owes an evidence-only addendum for exact `a061bd6f...`:

1. rejected `d5e79fc7...` versus fixed `a061bd6f...` red/green proof for the
   ready-ring ownership blocker and 73-stack reclamation blocker;
2. corrected replay patch-ID commands and values;
3. real normalized OFF equality for default, `usb-host`, and `usb-hid`, with
   fixed build ID, `-Z location-detail=none`, and ELF-preserving strip;
4. one uninterrupted successful close gate using the script from exact
   `a061bd6f...`;
5. truthful dirty shared-tree and clean detached-worktree evidence.

Controlling document:

```text
Plans/EVIDENCE-FIX-FORWARD-2026-07-25-ot61-c1.1-a061bd6-cerebro.md
```

No browser four-shard token exists. After the evidence addendum, independently
review the exact provisional candidate before deciding whether to grant one.

The shared root tree has pre-existing untracked paths. Do not delete, move,
stage, stash, or modify them:

```text
3dvon8gkkn57fwbhz3y7wwibj (# Delete conflict 2026-07-24 hycu7qC #).o
7u3cdk43kz5wufxfa7krlvtsz (# Delete conflict 2026-07-24 ypvf1cC #).o
Plans/FOLLOWUP-2026-07-22-deterministic-iso-tworebuild-proof-owed.md
Worktrees/
docs/research/vello-recording-ir-inventory.md
```

Use clean detached worktrees for browser evidence.

## 13. Ownership guide

- **Jarvis:** MT7601U/WiFi implementation and exact candidate returns.
- **Cerebro:** OT-61 browser C1.1 implementation; historically owns the
  Arthur/virtio single-in-flight fix lineage if code work becomes necessary.
- **Chad:** banked ACPI/FADT warm-reset actuator implementation.
- **Oracle:** driver correctness, Linux parity, transport trap-mining, and
  independent review.
- **Janus:** physical warm-residency/rig analysis and independent actuator
  review.
- **Prometheus:** research, prior-art, history, artifact comparison, and
  independent evidence only; never coding.

Do not keep every lane on Jarvis. Use independent review and bounded parallel
work where it does not violate the current exact-candidate freeze.

## 14. RAG and repository discovery

The local semantic index is:

```text
http://127.0.0.1:8404
```

Health:

```powershell
Invoke-RestMethod http://127.0.0.1:8404/health
Invoke-RestMethod http://127.0.0.1:8404/stats
```

Use RAG for semantic discovery in unfamiliar code. Use `rg` for exact symbols
and direct reads for known files. The index root and startup instructions are
in `E:\AI Automation\AltimitNeo\AGENTS.md`.

## 15. Key governance documents

Read these before changing a hold:

```text
Plans/REVIEW-DISPATCH-2026-07-25-mt7601u-post-qselect-terminal-f4a0940.md
Plans/EVIDENCE-FIX-FORWARD-2026-07-25-mt7601u-f4a0940-lane-c-prometheus.md
Plans/AUTHORIZATION-2026-07-25-mt7601u-f4a0940-four-shard-jarvis.md
Plans/RESEARCH-DISPATCH-2026-07-25-f4a0940-arthur-reversibility-red-prometheus.md
Plans/DISPOSITION-2026-07-24-mt7601u-warm-reset-qualified-banked.md
Plans/RESUME-DISPATCH-2026-07-25-ot61-c1.1-aag-fix-forward-cerebro.md
Plans/EVIDENCE-FIX-FORWARD-2026-07-25-ot61-c1.1-a061bd6-cerebro.md
docs/ADR-054-phase-7.5-mt7601u-driver.md
docs/ADR-078-ot61-browser-c-ladder.md
docs/runbook-wlan0-dora-optiplex.md
docs/runbook-baremetal-arthur-optiplex.md
```

## 16. Exact verification commands

Current governance and candidate identity:

```powershell
git fetch origin
git rev-parse origin/main
git ls-remote origin refs/heads/codex/mt7601u-post-qselect-terminal-jarvis
git ls-remote origin refs/heads/codex/ot61-c1.1-aag-fix-forward-cerebro
git ls-remote origin refs/heads/codex/mt7601u-warm-reset-fresh-base-chad
```

Expected branch tips:

```text
codex/mt7601u-post-qselect-terminal-jarvis
  f4a0940872e865207128a240587f7cac036f0cf9

codex/ot61-c1.1-aag-fix-forward-cerebro
  61d50a9951472b876389cf53b525d5222f7ac764

codex/mt7601u-warm-reset-fresh-base-chad
  fb2830d432ae5bf63214d69fb6541af4f599761f
```

The published handover commit is the current `origin/main`; its parent must be
`3d7a1b766167a00e408a34a778250c1e6f30bb79`. The relay accompanying this
document supplies the handover commit's exact SHA.

Failed gate:

```powershell
gh run view 30181808389 `
  --repo SecondStageTurbine/NeoPlutarch `
  --json databaseId,headSha,headBranch,event,status,conclusion,attempt,url,jobs
```

Push CI:

```powershell
gh run view 30159747107 `
  --repo SecondStageTurbine/NeoPlutarch `
  --json databaseId,headSha,headBranch,event,status,conclusion,attempt,url
```

## 17. Acceptance and merge boundaries

### WiFi

Do not merge until:

1. the current Arthur failure is classified;
2. one newly authorized exact-`f4a0940` four-shard gate is all green in one
   workflow attempt;
3. PM independently verifies every job and exact SHA;
4. a separately authorized local-credential ISO is built, credentials are
   reverted, and the ISO is independently identified;
5. silicon reaches scan, association, deferred calibration, WPA/GTK, DHCP,
   nonzero IPv4, `LINK UP`, terminal PASS, sink agreement, and
   `fence_refusals=0`;
6. credential and `/dev/sda` hygiene are proven;
7. PM performs verify-before-merge and publishes the merge disposition.

### Browser C1.1

Do not merge until:

1. Cerebro closes the evidence addendum for exact `a061bd6f...`;
2. independent code/AAG review accepts that SHA;
3. a separately authorized exact-SHA four-shard gate is entirely green;
4. OFF equality and capability-confinement evidence are accepted;
5. PM publishes a merge disposition. Never substitute `61d50a99...`.

### Warm reset

Do not integrate merely because `fb2830d4...` is banked. Integration requires a
new exact child, review, artifacts, gate, and hardware authorization after the
core WiFi cold result justifies it.

### Whole OS 1.0

WiFi and browser merges are necessary but not sufficient. The final release
must pass a user-visible physical acceptance sequence:

```text
install -> first boot -> login -> desktop -> Internet browsing
        -> create/save/reopen report -> print physical page
```

Every arrow needs an owner, exact artifact, bounded failure evidence, and a
repeatable runbook.

## 18. Current holds in one line

**Wait for Prometheus's research-only Arthur verdict; keep `f4a0940`,
`a061bd6f`, and `fb2830d4` immutable; do not rerun, boot, reset, integrate, or
merge until the PM publishes the next exact authorization.**
