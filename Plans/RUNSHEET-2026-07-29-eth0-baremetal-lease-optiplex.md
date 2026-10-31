<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RUN SHEET: eth0-at-boot bare-metal lease (JOINT — PL + Chad) — 0.1 rung 4 acceptance

**Date:** 2026-07-29
**Candidate:** `codex/eth0-boot-product-flow-chad` @ **`c1e4405`** — **4-shard `workflow_dispatch` GREEN, PM-verified on the exact SHA** (run `30464488461`); push-CI `30464487327`.
**What this proves:** 0.1 milestone **rung 4 ("it's online")** — the OS gets a real DHCP lease **at boot, as a product**, with **no test scenario involved**. This is the last non-physical box on rung 4.
**Authoritative procedure:** `docs/runbook-baremetal-arthur-optiplex.md` (access, disk layout, dd, `efibootmgr`, serial capture). This sheet is the checklist; the runbook wins on any detail.

> **This run differs from every previous bare-metal run: it is a PRODUCT boot, not a `neo-test` scenario.** There is no `NEO_TEST/1 event=PASS` line. The acceptance evidence is the **`eth0:` status line** on serial and the boot frame.

## 0. Current blocker (PL action — this is the whole hold-up)
- `ssh ubuntu@<optiplex-ip>` **times out**; `C:\Users\<user>\neoplutarch-serial.log` is **stale from 2026-07-27 18:49** with no eth0 markers. The OptiPlex is off, or not booted into the Ubuntu live USB.
- **PL:** power the OptiPlex on, boot the **Ubuntu live USB**, confirm `ssh ubuntu@<optiplex-ip>` answers (if host-key churn: `ssh-keygen -R <optiplex-ip>`).
- **PL: attach the USB-Ethernet dongle** (AX88179/ASIX or a CDC-ECM class device) — **the whole point of this run.** Confirm on the live USB: `lsusb | grep -i -E '0b95|ethernet'`.
- Confirm the LAN has a working DHCP server (the same one that served `<optiplex-ip>` on 2026-06-13).

## 1. Pre-flight — re-inventory (do NOT assume last state)
`lsblk -o NAME,MODEL,WWN,SIZE,TRAN` and confirm:
- **`/dev/nvme0n1`** = the internal boot target (INTEL SSDPEKKR256G8) — **this is the dd target**.
- **`/dev/sda`** = the Samsung **Arthur model volume** — **NEVER a dd target on this box.** (Note: the M710q is the *opposite*; identity-check by model/WWN, never by bare name.)

## 2. Build the PRODUCT ISO (Chad, build host) — not a scenario build
```
cargo xtask iso --features product          # inside the dev container / with LIMINE_DIR set
# e.g. docker compose run --rm neo bash -lc 'LIMINE_DIR=/opt/limine cargo xtask iso --features product'
```
`product = ["kernel-diagnostic","accel-sim","inference-preemptible","boot-eth0"]`. Confirm the built ISO is the **product** kernel (not `default`, which has `boot-eth0` OFF and would prove nothing).

## 3. Stage to the OptiPlex
```
scp -O boot/neoplutarch.iso ubuntu@<optiplex-ip>:/tmp/
# identity-check FIRST, then write the ISO to the NVMe (NOT sda):
sudo -n dd if=/tmp/neoplutarch.iso of=/dev/nvme0n1 bs=4M conv=fsync status=progress ; sync
```

## 4. Boot + capture (JOINT)
1. **PL:** PuTTY serial → log to `C:\Users\<user>\neoplutarch-serial.log` — **start fresh** (preserve the prior capture first).
2. `sudo efibootmgr -n 0001 && sudo reboot` (one-shot; falls back to Windows → boot-select back to Ubuntu after).
3. **Let it run to completion.** The eth0 bring-up is bounded by construction (256 link-settle passes, 4 DHCP attempts, 750k pump budget per attempt) — it self-terminates with a status line; do not power-cycle early.

## 5. Acceptance — what PASS looks like
**PASS (rung 4 closed):** serial shows the bring-up chain and a real lease, and the boot frame shows the IP:
```
[ kernel ] eth0: boot bring-up enabled (usb enumerate -> cdc-ecm bind -> dhcpv4)
[ kernel ] eth0: link up ip=<a.b.c.d> router=... dns=... fence_refusals=0
```
Required: **nonzero IPv4**, **`fence_refusals=0`** (I-14(c) containment), the IP visible on the **framebuffer** (PL's live eyes-on), and **boot completes normally** afterwards.

## 6. Discriminator bank (the run must be conclusive either way)
| Observation | Meaning | Next step |
|---|---|---|
| `eth0: link up ip=<a.b.c.d>`, `fence_refusals=0` | **PASS — rung 4 closed** | PM verify + I-14 AAG → merge |
| `eth0: no device` **with the dongle attached** | enumeration/bind failure (not the degrade path) | capture xHCI enumeration + whether `148f`/`0b95` bound; this is a real bug |
| `eth0: no link` | dongle bound, no carrier | check cable/switch port; re-run |
| `eth0: no lease` | bound + carrier, DHCP unanswered | confirm the LAN DHCP server; check DISCOVER on serial (the `chaddr` non-empty prior-art trap) |
| `fence_refusals > 0` | **I-14(c) containment breach** | governance signal — bounce, not a NIC bug |
| Boot hangs or panics | **unacceptable regression** | the OFF path must still boot — capture and bounce immediately |
| `eth0: no device` **with NO dongle** | correct degrade (already proven in QEMU) | expected; not this run's target |

## 7. Post-run
1. Save the serial log; paste it to the PM for the byte-level read + merge disposition.
2. Boot-select back to Ubuntu.
3. **No credentials are involved in this run** (unlike the WiFi sheet) — nothing to revert, but confirm the worktree is clean.

## Merge (after PASS)
PM verify-before-merge (independent CI re-verify on the exact SHA + own `git show --stat`) + **I-14 containment AAG**, then `--ff-only`. Rebase-fresh first (the branch was 2 behind `main`). Disk-reclaim after.
