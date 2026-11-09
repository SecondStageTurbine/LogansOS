<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat I, setup menu S4 — the Network page and `SYS_NET_IF_STATUS` (2026-11-09)

**Seat:** Seat I (cloud). **Branch:** `claude/setup-s4-net-info` off `origin/main` (S3 landed as `0b434c3e`).
**Design:** `docs/THINK-2026-11-02-setup-menu.md`: §3.3 (the Network page) and the **S4** section in §5.
**Feedback item:** F11 in `Plans/FEEDBACK-2026-11-02-preview.md`.

## What to build (the S4 section, as written)

1. **`SYS_NET_IF_STATUS`**, a read-only kernel opcode that fills a fixed-size `NetIfStatusV1`. The kernel gives back
   a snapshot of the interface, and nothing in it writes kernel state.
2. **The Network page**, compiled in this rung only. The call and the page land together, so there is never a page
   without a status source. Continue stays enabled when the link is down.
3. **Kill-switch `net-info`:** with it off, the syscall returns `ENOSYS`, the page shows "Network status is
   unavailable" and Continue, and the rest of the boot is unchanged.

## Oracles

- **Byte oracle (I-12):** a hand-laid `NetIfStatusV1` example in the THINK or the test, independent of the writer and
  reader. A round-trip alone isn't enough (CLAUDE.md, the byte-oracle rule). The host KAT checks the struct field by
  field, `lease=none` from a `snapshot()` with no lease, and `EINVAL` on a short buffer.
- **Gate `setup_menu_net`:** QEMU with `usb-net` on slirp, the CDC-ECM product path. The page logs
  `setup net link=up driver=cdc-ecm ipv4=10.0.2.15`. **Control:** with no NIC, it logs `link=down`, Continue is still
  enabled, and Home follows.

## Kernel-touching: the standing rules

- **Concurrency pitfall bank** (`docs/concurrency-pitfalls.md`): the snapshot reads interface state that the driver
  and the DHCP client write. In the PR, state for each touched class (lock ordering against the net lock, a torn read
  of a multi-field snapshot, an ISR-context lock) whether the change introduces it, guards it or leaves it alone, and
  name the discriminator.
- **I-14:** the syscall must take no AI-governance lock, and the snapshot must never block on the driver's ISR path.
- **4-shard gate** at the head via `scripts/worker-close-gate.sh`, then the PR run. Post the URL in a PM: comment.
- When it's green, the PM queues a cross-family review. Its checklist includes the net-lock/IF rule and the prior-art
  item.

## Close

Commit and push at every checkpoint. After merge, `cargo clean` and remove the worktree.
