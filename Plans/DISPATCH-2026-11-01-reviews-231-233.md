<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 231-233: cross-family reviews of three gate-green kernel PRs (the always-on relay box)

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md` (public repo: no personal details; RAG not
available; plain checklist, PASS or FAIL with `file:line` and a concrete input for any FAIL; name the head SHA you
reviewed; RETURN on a new branch off `origin/main`, push, reply with the commit SHA; never push to `main`).

## Section 231: Chad, review of #15 (OT-88 A1a: the WM's client table)

**MODEL** GPT-6.1-Sol, **EFFORT** high: it changes the kernel's syscall and IPC paths.

`git fetch origin +pull/15/head:pr15 && git diff origin/main...pr15` (head `87bad1b7`).

1. Every new or changed syscall argument from user space is validated (lengths, indices into the client table,
   handles), with the enforcing `file:line`.
2. A client that exits, faults or is killed is removed from the table, and no later message, input route or turn
   reaches its stale slot (slot reuse included).
3. One client's fault or flood can't stall or starve the WM or other clients (bounded queues, per-client limits).
4. Input routing: input reaches only the focused client; nothing reaches a client while the lock screen is up.
5. The default and product builds are unchanged when the feature is off (kill-switch).
6. The prior-art / CP section the PR needs is present and its claims hold.

RETURN: `Plans/RETURN-2026-11-01-pr15-client-table-review-chad.md`, branch `codex/pr15-review-chad`.

## Section 232: Janus, review of #16 (L7.1: boot time from power-on)

**MODEL** GPT-6.1-Sol, **EFFORT** high: a new syscall and ACPI table parsing (FPDT).

`git fetch origin +pull/16/head:pr16 && git diff origin/main...pr16` (head `958e4fc1`).

1. FPDT parsing bounds-checks every length and pointer from firmware; a truncated, zero-length or overlapping
   record can't read out of bounds or loop forever.
2. The new syscall's ABI: struct layout has a hand-laid byte oracle in the tests; the user buffer copy uses the
   checked user-copy path, and a short or bad buffer returns an error, not a fault.
3. Missing FPDT (QEMU, most firmware) gives a defined "unknown" result, not zero or garbage.
4. The time arithmetic can't overflow or go negative (TSC frequency unknown, firmware time after kernel time).
5. The feature off is the previous build (kill-switch); the default image unchanged.

RETURN: `Plans/RETURN-2026-11-01-pr16-boot-time-review-janus.md`, branch `codex/pr16-review-janus`.

## Section 233: Jarvis, review of #29 (OT-61 rung 5: the fetch broker)

**MODEL** GPT-6.1-Sol, **EFFORT** high: a network capability boundary in the kernel (who may dial out).

`git fetch origin +pull/29/head:pr29 && git diff origin/main...pr29` (head `79bc7817`).

1. The browser process gets no dial capability of its own: list each path by which it could open a connection and
   the check that stops it.
2. The public-only dial right: loopback, private (RFC 1918), link-local, IPv4-mapped IPv6, NAT64 and multicast
   addresses are refused, after DNS resolution and on every Happy Eyeballs attempt (not only the first).
3. The broker's wire format has a hand-laid byte oracle; malformed or oversized requests are rejected.
4. Capability lifetime: the dial right can't outlive the process or be transferred to another process.
5. The TCP changes don't alter behaviour for existing callers (feature off is the previous build).

RETURN: `Plans/RETURN-2026-11-01-pr29-fetch-broker-review-jarvis.md`, branch `codex/pr29-review-jarvis`.
