<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Bundled fonts (Desktop S2)

| File | Face | Version | Role | sha256 |
|---|---|---|---|---|
| `Inter-Regular.ttf` | Inter Regular (static) | 4.001 (release v4.1) | Arthur's proportional voice (design language §6.1): Arthur's surface, Atrium/Home, the Lens | `40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82` |
| `IBMPlexMono-Regular.ttf` | IBM Plex Mono Regular | 2.005 | `neo>` rail / structured data | `7c6fbddca4b700be918f5f6183d9bd4464fa427fe435f0b480d77fe2bb8c5a43` |

**Licences:** both SIL Open Font License 1.1. The fonts are *not* MPL-2.0 and
carry no SPDX source header; see the repository `NOTICE`.
- Inter: Copyright © 2016 The Inter Project Authors, no Reserved Font Name. Full
  text in [`OFL-Inter.txt`](OFL-Inter.txt), byte-identical to `LICENSE.txt` in the
  release archive below (sha256
  `262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a`).
- IBM Plex Mono: Copyright © 2017 IBM Corp. with Reserved Font Name "Plex". Full
  text in [`OFL-IBM-Plex.txt`](OFL-IBM-Plex.txt), byte-identical (after LF
  normalisation by the repository's `.gitattributes`) to the `LICENSE.txt` in
  its release archive.

## Provenance

Each `.ttf` is its publisher's own canonical TrueType file, **byte-for-byte
unmodified**, taken from the official GitHub release archive (raw bytes via
`curl`, never a summarising fetch):

| Release (tag) | Archive | Archive sha256 | Path inside the archive |
|---|---|---|---|
| [`rsms/inter v4.1`](https://github.com/rsms/inter/releases/tag/v4.1) | `Inter-4.1.zip` (33,707,794 bytes) | `9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e` | `extras/ttf/Inter-Regular.ttf` (411,640 bytes) |
| [`@ibm/plex-mono@2.5.0`](https://github.com/IBM/plex/releases/tag/%40ibm%2Fplex-mono%402.5.0) | `ibm-plex-mono.zip` | `6d23f01257663d8cc49a0d64c22ced630b79e0e2a0ac08a0da86e9a38bbc481c` | `ibm-plex-mono/fonts/complete/ttf/IBMPlexMono-Regular.ttf` |

Re-fetch with `curl -L -o Inter-4.1.zip
https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip` and `curl -L -o
ibm-plex-mono.zip
https://github.com/IBM/plex/releases/download/%40ibm%2Fplex-mono%402.5.0/ibm-plex-mono.zip`,
then compare the hashes above. Inter's static Regular is the TrueType in
`extras/ttf/`; the variable fonts and the `web/` WOFF2 files in the same archive
are not used.

`neo-text`'s host test `shipped_fonts_are_canonical_sfnts` checks that every
shipped file sums to the OpenType magic `0xB1B0AFBA` (`head.checkSumAdjustment`
intact) and carries the expected name records (family, version, OFL licence).

*History.*
- Until the 2026-10-18 S2-F1 fix-forward the Plex files were sfnts unwrapped
  from IBM's npm WOFFs, whose rebuilt table directory left
  `head.checkSumAdjustment` invalid (Codex cross-check, Chad).
- Until 2026-10-26 Arthur's proportional voice was IBM Plex Sans Regular 3.005
  (`IBMPlexSans-Regular.ttf`, sha256
  `975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5`, from
  `@ibm/plex-sans@1.1.0`). J.c. replaced it with Inter. Nothing ships it any
  more, so the file and neo-text's `plex-sans` feature were removed; it remains
  in git history.

## Why not subset

OFL-1.1 treats subsetting as creating a *Modified Version*, and Plex's Reserved
Font Name means a modified Plex could not keep the name "Plex" without IBM's
permission. Inter has no Reserved Font Name, but the same rule applies here:
the faces ship unmodified (no subsetting, no renaming), linked only into the
binaries that render text.

## How they reach the boot image

`userland/neo-text` exposes them as `fonts::INTER_REGULAR` /
`fonts::PLEX_MONO_REGULAR` (`include_bytes!`, behind the `inter` / `plex-mono`
features). `neo-arthur` enables both, so the faces travel inside the
`neo-arthur` ELF, which xtask stages as the `/modules/neo-arthur` Limine module.
`neo-atrium` (the window manager in `neo-init`) and `neo-worker` (the Lens)
enable `inter` only.

OFL §2 requires every redistributed copy to carry the copyright notice and the
licence as stand-alone, human-readable text. `cargo xtask iso` therefore stages
`OFL-Inter.txt`, `OFL-IBM-Plex.txt` and the repository `NOTICE` into the boot
medium's `/licenses/` directory (plain files on the ISO's Rock Ridge/Joliet
filesystem, readable by anyone who mounts the medium).
Unlike the MT7601U firmware (OT-27a), OFL fonts are freely redistributable, and
the host byte oracles need them in-tree.
