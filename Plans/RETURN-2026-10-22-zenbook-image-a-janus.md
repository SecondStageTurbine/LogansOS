<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook image A rebuild — Janus

**Date:** 2026-10-22  
**Branch:** `codex/zenbook-image-a-rebuild-janus`  
**Main built:** `4e9ea28ca3b9ef91e4e66904a0e59eacc6b9b8e0`  
**Build:** `docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom`

## Artifact

- ISO: `target/b1-model/A-product.iso` (copied from `boot/neoplutarch.iso`)
- Size: **635,021,312 bytes**
- BLAKE3: `cc632a4c2f50f49d7b85a99700208d645c6225e263402ed93ac0a66bfe1b972c`
- Kernel ELF: `target/x86_64-neoplutarch/debug/kernel` (37,111,296 bytes)
- Kernel BLAKE3: `b05f3a654d308d741215d602eda73a9673f9d70b868ec665323d696b632d4923`
- `lapic-timer`: **confirmed** — `strings -a` over the kernel ELF contains `[ kernel ] timer: LAPIC tick mode=`.

The pinned model used was `model-qwen2.5-0.5b.neom` (485,489,152 bytes), BLAKE3 `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe`; its sibling vocab also matched the pin (`e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265`). Xtask staged the model as `/models/model-qwen2.5-0.5b.neom`.

No B/C bundles were built. The build emitted the existing note that the optional MT7601U firmware blob is absent; it does not prevent the product ISO from building.

## Hand-off

`A-product.iso` is ready at `E:\AI Automation\workers\janus\target\b1-model\A-product.iso` for J.c. to write to the Kingston DataTraveler with Rufus in DD image mode. Confirm the target is the approximately 30 GB Kingston stick; never target the Zenbook's 954 GB internal disk. The runbook describes the image and bare-metal procedure in `docs/runbook-baremetal-zenbook-s14.md`.

## Build branch

