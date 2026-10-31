<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FIX-FORWARD: MT7601U warm-reset qualification return -> Chad

**Date:** 2026-07-24
**From:** PM
**To:** Chad
**Independent reviewer:** Janus
**Integration consumer:** Jarvis
**Bounced candidate:** `2fb648ed8ae688ec65bff6ec5c81c3b05d3b92ec`
**Controlling dispatch:** `Plans/DISPATCH-2026-07-24-mt7601u-warm-reset-qualification-chad.md`
**Verdict:** **BOUNCE. Build and host-test one child recut. No hardware, reset, boot, workflow dispatch, merge, or integration.**

## PM disposition

Janus independently reviewed the exact immutable candidate. The actuator design is structurally sound:

- live FADT-derived descriptor, with no hard-coded OptiPlex tuple in the actuation path;
- exact triple-feature reachability;
- authorized terminal-PASS-only invocation;
- final derived-value breadcrumb and flush;
- exactly one byte-port write;
- no CF9, keyboard-controller, EFI, watchdog, triple-fault, or other fallback;
- fail-stop behavior if the reset write returns;
- source-level structural OFF gating.

The candidate is nevertheless not dispatch-complete. Both previously identified PM gaps are confirmed:

1. it returns reset-symbol/string absence, not the required normalized artifact byte-equality proof;
2. its trailing-byte parser test does not exercise XSDT/RSDT sibling traversal through the production discovery logic.

The candidate remains forbidden for Jarvis integration or hardware qualification.

## Required child recut

Append one child commit to `2fb648ed8ae688ec65bff6ec5c81c3b05d3b92ec`. Do not amend, rebase, force-push, delete, or rewrite the reviewed candidate.

### 1. Unrelated-sibling discovery oracle

Add a deterministic, device-free oracle that exercises the production ACPI discovery decision logic rather than only the FADT byte parser.

At minimum, prove:

- an XSDT with a valid unrelated sibling before the FADT still selects the FADT;
- an RSDT with a valid unrelated sibling before the FADT still selects the FADT;
- the selected FADT bytes still pass the existing signature, declared-length, checksum, revision, flag, GAS, address, and reset-value validation;
- the unrelated table cannot be mistaken for or weaken validation of the FADT;
- host tests perform no physical-memory access and no port I/O.

Derive the smallest test seam that binds the oracle to the shipped discovery logic. Do not hand-model a parallel selector while claiming production coverage. Preserve the accepted actuator semantics and feature boundary.

### 2. Seven-row normalized OFF equality matrix

Rebuild the exact seven candidate-OFF configurations previously inspected for reset symbols/strings. For every row, return:

- configuration/scenario name and complete feature set;
- baseline SHA and candidate SHA;
- pinned toolchain identity;
- fixed build ID and all reproducibility inputs;
- clean-target/build commands;
- normalization command and normalizer identity;
- normalized artifact path and byte size;
- baseline normalized SHA-256;
- candidate normalized SHA-256;
- explicit byte-equality verdict.

Every row must be byte-identical. Symbol/string absence is useful corroboration but is not a substitute for equality. Use clean, separate baseline and candidate targets and the same toolchain, build ID, and normalization procedure. Do not use a stale artifact from another worktree.

### 3. Gates

Run and return:

```powershell
scripts/worker-close-gate.sh
```

The complete script must pass in one clean sequence. Run the focused FADT/discovery oracle separately and report its exact command and test count.

Push only Chad's branch and return the automatic push-CI URL for the exact child SHA. This remains kernel-touching work, but **no `workflow_dispatch` runner token is granted**. Do not fire the four-shard workflow.

## RETURN

Return:

- branch, new immutable SHA, parent, and frozen-base relation;
- child own-stat and full branch stat;
- exact touched-file list;
- sibling-discovery oracle design and focused results;
- complete seven-row normalized equality table and reproducibility commands;
- full close-gate result;
- exact-SHA push-CI URL;
- clean-worktree and remote-parity proof;
- confirmation that no MT7601U, xHCI, network, DHCP, sink-order, boot manifest, credential, or evidence file changed.

Janus will independently review the new exact SHA. Jarvis may consume it only after a new PM acceptance. No reset actuation, VBUS qualification, third TEST 3 boot, workflow dispatch, merge, or PR is authorized.
