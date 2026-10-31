<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPOSITION: MT7601U warm-reset qualification accepted and banked

**Date:** 2026-07-24
**From:** PM
**Candidate:** `fb2830d432ae5bf63214d69fb6541af4f599761f`
**Branch:** `codex/mt7601u-warm-reset-fresh-base-chad`
**Base:** `856874182b88679462f1a9d206726cfa3ff5eb1f`
**Push CI:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30139397576`
**Verdict:** **RESET QUALIFICATION TOKEN-OK. Bank the candidate. Do not integrate it before the active cold WiFi result.**

## Accepted review

Janus independently reviewed the exact candidate and returned code/actuator
`TOKEN-OK`:

- XSDT and RSDT discovery traverse unrelated siblings before selecting FADT;
- live FADT validation covers signature, length, checksum, revision, support
  flag, GAS shape, address, and reset value;
- the reset path is structurally gated by the exact three qualification
  features;
- invocation is reachable only after the authorized association terminal PASS;
- PASS and the FADT-derived breadcrumb are flushed before actuation;
- the actuator derives its byte port and value from the live descriptor;
- disassembly contains exactly one `outb %al,%dx`;
- no hard-coded tuple or fallback exists;
- if the write returns, the kernel logs, flushes, and halts permanently;
- focused oracle: 8 passed, 0 failed;
- incomplete feature combinations are rejected;
- the exact qualification kernel compiles.

The PM previously accepted replay parity, scope, the uninterrupted close gate,
and exact-SHA push CI.

## Corrected OFF artifact proof

The evidence-only rerun used:

- rustc `1.97.0-nightly`, commit
  `e8e4541ff19649d95afab52fdde2c2eaa6829965`;
- `CARGO_INCREMENTAL=0`;
- fixed `NEO_BUILD_ID=chad-reset-elf-equality-20260724`;
- identical remaps and `-Z location-detail=none`;
- GNU `objcopy 2.44 --strip-all`.

The normalized outputs remain ELF kernels. Only nonallocated `.symtab` and
`.strtab` were removed. Every `SHF_ALLOC` section and payload was preserved.

The PM directly re-hashed and byte-compared all fourteen files:

| Reset-OFF row | Bytes per side | Paired SHA-256 |
|---|---:|---|
| default | 2,722,240 | `592779fe94f9764ff1ee532900095fefdb80d582082d37100a556265e57ec489` |
| minimal/no-MT | 2,611,680 | `8694b2af0a95f112da091e43cff67872ff922379a0b8fe9fa8f74f7c34906089` |
| USB host | 2,751,656 | `9830a2bd7b3efe1d25792ac2dd018d72b381103856a19c139072d57783e649cd` |
| USB HID | 2,768,072 | `56273b904e916ea9a6250c234d070c863e0a267ee6f216358ef9ffb3f8a25c06` |
| QEMU IPC | 2,619,904 | `3231f259a0895fecfd99d8e1f06413224b49b8bdd6fe9d9ccb3157484f693ec4` |
| USB bare metal | 2,772,392 | `d0728d42ef6c24352d9e700906bffce158910bcc77d0a8fcea7a168e9a4d1916` |
| MT7601U association, reset OFF | 3,330,792 | `1c324983d39ef2ff104aace574499dceff21ea3ccc527d3d37d66f2f15297728` |

Every pair has direct byte equality. Machine-readable manifest:

```text
E:\AI Automation\AltimitNeo-warm-reset-elf-equality-fb2830d4\seven-row-elf-equality.json
sha256 7472b3657f798ddc7ca610db071b8bfaa37d3b2af89324689e063a1c9fbb928e
```

The candidate and baseline worktrees are clean. The remote reset branch still
resolves exactly to the candidate.

## Banked boundary

Do not integrate this candidate into Jarvis's WiFi branch yet. The active
priority is the separately authorized cold-only WiFi validation of
`625e531f353b417498a968dfa0b74f1d45c0e592`.

The cold result will establish whether the actual scan/join/DHCP issue is
fixed. Only after that result will the PM decide whether to dispatch reset
integration for a separate immediate-warm proof.

No workflow dispatch, integration, credential ISO, reset actuation, hardware
boot, merge, or PR is authorized by this disposition.
