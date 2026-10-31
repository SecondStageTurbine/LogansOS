<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook USB test image A — Janus

Date: 2026-10-22  
Return branch: `codex/zenbook-usbtest-image-janus`  
Source built detached: `6f09e826009fd824b10419da4f06b93fa1937f8b` (`origin/claude/zenbook-usb-input`)

**TEST IMAGE from an unmerged USB branch, for one Zenbook run only.** Image A only; no bundles were built for this task.

## Build and pinned inputs

Built successfully (exit 0) in the Janus worktree with:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

Toolchain: `nightly-2026-04-16`. The model was staged as `/models/model-qwen2.5-0.5b.neom`.

| Input | Bytes | BLAKE3 |
| --- | ---: | --- |
| `target/b1-model/model-qwen2.5-0.5b.neom` | 485489152 | `a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe` |
| `target/b1-model/model-qwen2.5-0.5b.vocab` | 7990862 | `e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265` |

Both inputs match the pin recorded in the prior Janus image A RETURN (commit `0294942e69a69725eacebae754a099dbfbcd4ae2`).

## Artifacts and verification

Copied `boot/neoplutarch.iso` to `target/b1-model/A-usbtest.iso` after the build completed. Both files have the same size and BLAKE3.

| Artifact | Bytes | BLAKE3 |
| --- | ---: | --- |
| `target/b1-model/A-usbtest.iso` | 635897856 | `9152b604a95b9249307d853a099c408c28f1029d0a1195a7d11b11279f45218e` |
| `target/x86_64-neoplutarch/debug/kernel` | 37987536 | `66ab8edeb9be987a6eaee01d8258001d8a0e6bad62a9e338234d88758a7cd2f5` |

The staged `target/iso-root/boot/kernel` has the same kernel BLAKE3. `strings -a` over the built kernel ELF confirms both requested markers, including:

```text
usb-summary: ---- USB input evidence ----
usb-summary: INPUT CHECK - press a key and move the mouse now (
```

The build emitted 41 kernel warnings and the note that optional MT7601U firmware is absent; `/modules/mt7601u-fw` was omitted and that Wi-Fi lane will fail closed. No source fixes were made. No physical Zenbook boot was performed by Janus.

RAG health was healthy (`ollama=true`, `qdrant=true`). Existing local `.gitignore` and `.ignore` changes were preserved and excluded from this RETURN commit.

## Handoff to J.c.

Use this exact file:

```text
E:\AI Automation\workers\janus\target\b1-model\A-usbtest.iso
```

Write it to the **KINGSTON USB stick with Rufus in DD image mode**. Confirm the selected device is the KINGSTON stick before writing. **Never select the 954 GB internal disk.** This is the unmerged USB diagnostic image for one Zenbook run only. At INPUT CHECK, press a key and move the mouse; capture the USB summary screen for review.

The ISO remains a local build artifact; the pushed branch carries this RETURN, not the ISO.
