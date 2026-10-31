<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: MT7601U warm-reset actuator qualification -> Chad

**Date:** 2026-07-24
**From:** PM
**To:** Chad
**Independent reviewer:** Janus
**Integration consumer:** Jarvis
**Controlling fan-out:** `Plans/DISPATCH-2026-07-24-mt7601u-test3-parallel-fanout.md`
**Verdict:** **BUILD AND HOST-TEST NOW. No hardware actuation, boot, workflow dispatch, or merge.**

## Purpose

Janus's read-only Lane 4 follow-up found a valid ACPI FADT reset-register declaration on the OptiPlex:

- Generic Address Structure: System I/O;
- address `0xB2`;
- width 8 bits;
- reset value `0x73`;
- reset-register support flag set;
- FADT revision 6 with a valid checksum.

These machine values are untrusted diagnostic evidence, not constants to copy into code. ACPI defines the register as a complete-system-reset mechanism but does not promise continuous USB VBUS. The reset actuator exists; its electrical behavior remains unqualified.

NeoPlutarch currently flushes and halts permanently after a terminal frame. A narrowly gated, attributable reset path is required before the PM can qualify a cold-to-immediate-warm procedure. Keep this work isolated from Jarvis's Wi-Fi repair.

## Read first

```powershell
git fetch origin
git show origin/main:Plans/DISPATCH-2026-07-24-mt7601u-warm-reset-qualification-chad.md
git show origin/main:Plans/DISPATCH-2026-07-24-mt7601u-test3-parallel-fanout.md
git show origin/main:Plans/FIX-FORWARD-2026-07-24-mt7601u-channel7-recovery-warm-proof-jarvis.md
```

Independently trap-mine ACPI 6.6 FADT and Generic Address Structure rules, the existing ACPI table discovery code, x86 port-I/O primitives, terminal logging, and structural feature gates. Derive and own the implementation.

## Scope

Work on Chad's own clean branch from current `origin/main`. This is a test-infrastructure coding lane.

Owned:

- pure FADT reset-register parsing and validation;
- host/KAT fixtures for valid and hostile tables;
- an exact-feature-gated bare-metal reset actuator;
- terminal flush/breadcrumb/one-actuator behavior;
- structural OFF proof.

Not owned:

- MT7601U driver, PHY, xHCI, network, DHCP, or sink-order repair;
- Jarvis's branch;
- boot-order mutation or Ubuntu configuration;
- any hardware invocation.

Do not edit Jarvis's worktree or branch. Prefer isolated new modules and a minimal integration seam so Jarvis can review and integrate without importing unrelated changes.

## Required parser properties

The parser must not trust the OptiPlex constants. It must discover and validate the live FADT through the existing ACPI table path.

At minimum:

- ACPI table signature, minimum length for every accessed field, and checksum;
- supported FADT revision/field presence;
- reset-register-supported flag;
- Generic Address Structure address-space identifier;
- bit width, bit offset, access size, and address;
- reset value;
- representability by the selected x86 I/O operation;
- rejection of zero, truncated, reserved, unsupported, malformed, or ambiguous values.

The parser returns a typed description. It performs no reset and no port I/O.

## Hostile host oracle

Add device-free deterministic coverage for:

- Janus's valid revision-6 System-I/O shape;
- missing reset-support flag;
- bad checksum;
- truncated table at every consumed boundary;
- unsupported address space;
- nonzero bit offset;
- incompatible width/access size;
- zero or out-of-range address;
- missing/invalid reset value;
- table revision too old for the consumed fields;
- trailing data and unrelated ACPI tables not weakening validation.

Host tests must never issue port I/O. Include a negative control demonstrating that a hard-coded `0xB2 <- 0x73` path would incorrectly accept a hostile or different-machine table.

## Actuator properties

The actuator must be structurally reachable only in the exact, explicitly governed bare-metal warm-reset qualification scenario. A runtime-disabled branch is insufficient.

When invoked after an authorized terminal PASS, it must:

1. use only the validated live FADT descriptor;
2. flush the durable evidence sink and serial path;
3. emit one unique final breadcrumb naming the FADT-derived address space, address, width, and reset value;
4. issue exactly one reset operation;
5. use no keyboard-controller, CF9, EFI, triple-fault, watchdog, or other fallback;
6. if execution returns, emit/retain a unique failure disposition when possible and halt permanently.

Do not claim this preserves VBUS. Do not invoke it on FAIL. Do not make it the default `debug_exit` behavior or change unrelated terminal semantics.

## Structural kill switch

With the qualification scenario disabled:

- the parser/actuator integration and call site must not be reachable in the kernel;
- no port-I/O call, reset breadcrumb, branch, static, or data may remain in normalized artifacts;
- ordinary bare-metal, QEMU, default, USB-host, USB-HID, and `!mt7601u` paths remain unchanged.

Return the accepted normalized artifact matrix, using the same toolchain, fixed build ID, clean targets, and normalization procedure. Include exact SHA-256 pairs; compilation alone is insufficient.

## RETURN

Return:

- exact branch, commit, parent, and frozen-base relation;
- own-stat and touched-file list;
- parser design and validation table;
- all hostile host-oracle commands/results;
- structural cfg map and normalized artifact equality table;
- proof that the code contains no hard-coded OptiPlex reset tuple in the actuation path;
- proof that no MT7601U, xHCI, network, DHCP, sink-order, manifest, credential, or evidence file changed;
- clean-worktree proof;
- exact push-CI URL if CI runs.

Push only Chad's branch. Do not open a PR, merge, fire `workflow_dispatch`, build a credential-filled ISO, alter UEFI variables, reboot, or touch hardware.

## Review and convergence

Janus will independently review the exact returned SHA against the observed FADT and the proposed VBUS qualification procedure. Jarvis will treat the branch as untrusted integration input and may integrate only after PM disposition.

Even a perfect host gate does not qualify VBUS. A later, separately authorized rig-qualification run must scope the exact reset actuator and both transitions with direct VBUS instrumentation. No third TEST 3 boot is authorized here.
