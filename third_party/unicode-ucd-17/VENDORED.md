<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Unicode 17.0.0 `GraphemeBreakTest.txt`: vendored test data

The Unicode Character Database's grapheme-cluster break test, version 17.0.0. It is the independent oracle for
`userland/neo-edit-core`'s cursor stops (OT-67 E1).

- **Licence:** Unicode License v3, unchanged, in `LICENSE`. The file is used as test data only and ships in no image.
- **Version:** 17.0.0, because that is the Unicode version of the `unicode-segmentation` release the editor depends on
  (1.13.3: `UNICODE_VERSION = (17, 0, 0)` in its `src/tables.rs`). The test checks that pairing, so a dependency bump
  to a new Unicode version fails until this file is replaced with that version's.
- **Where it came from:** `unicode.org` is not reachable from the build container (the proxy refuses it). The file was
  taken with `git` from Unicode's own tools repository, never through a summarizing fetch (CLAUDE.md, exact bytes):

  ```
  git clone --depth 1 --filter=blob:none --no-checkout https://github.com/unicode-org/unicodetools.git
  git -C unicodetools show 578f6c0516f5f268292ebda50b51848407a06478:unicodetools/data/ucd/17.0.0/auxiliary/GraphemeBreakTest.txt
  git -C unicodetools show 578f6c0516f5f268292ebda50b51848407a06478:LICENSE
  ```

  The head was `578f6c0516f5` (2026-10-18). The file's own header reads `GraphemeBreakTest-17.0.0.txt`,
  `Date: 2025-03-24, 14:45:55 GMT`.
- **SHA-256:**
  - `GraphemeBreakTest.txt`: `e2d134d2c52919bace503ebb6a551c1855fe1a1faec18478c78fff254a1793ec`
  - `LICENSE`: `fe5c62b543e287981db198f2acfa0ca732d12591a1024536dc8fd85dacd77104`

  `neo-edit-core`'s test pins the first.
