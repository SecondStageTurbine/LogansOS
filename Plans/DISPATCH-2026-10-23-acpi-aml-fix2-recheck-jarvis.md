<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #43): narrow re-check of PR #104 (2e/4a, 3d) → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own failed items from relay #39
(RETURN `5c9f9bfe`).

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `c910b0b9` (pinned). New commits on
the `63041450` you reviewed, plus a merge of `main` `5e40fd31`. The kernel, Cargo and `.github` trees equal main.

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/acpi-aml-fix2-recheck-jarvis
          git fetch origin && git checkout -B codex/acpi-aml-fix2-recheck-jarvis c910b0b9

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-acpi-aml-fix2-recheck-jarvis.md   # this file
git show 5c9f9bfe:Plans/RETURN-2026-10-23-acpi-aml-fix2-review.md               # your RETURN
git diff 63041450 c910b0b9 -- third_party tools/acpi-aml-grade Plans docs        # the fixes
```

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete AML input.

1. **2e/4a closed.** Check that `check_object_size` now bounds the results of `Concat`, `ConcatRes`,
   `ToHexString`, `ToDecimalString` and `ToBuffer`-of-String, and that your input
   (`14 15 4d 41 49 4e 00 70 11 06 0c 00 00 10 00 60 a4 87 73 60 60 00`) now fails closed rather than returning
   2 MiB. Check the author's audit claim that `Mid`, `ToString` and `Store` are bounded by their source, and name
   any other op that can still build an object over `MAX_OBJECT_BYTES` from legal operands.
2. **3d resolved.** The author says the checklist item was wrong: ACPICA treats an out-of-range `Index` as an error
   (`AE_AML_BUFFER_LIMIT` / `AE_AML_STRING_LIMIT` / `AE_AML_PACKAGE_LIMIT`, `exoparg2.c` ~382-411). Check that
   against ACPI 6.5 §19.6.63 (`Index`) on uefi.org. The author could not reach it; you could last time. Check that
   the KAT `index_past_the_end_is_an_error` pins your input
   (`14 0f 4d 41 49 4e 00 a4 88 11 04 0a 01 aa 01 00`) to `IndexOutOfBounds`, and that the corrected dispatch and
   findings §7 state the rule accurately.
3. **Nothing else regressed.** Check that the census still has 0 unclassified sites, the corpus report is
   byte-identical to main, the ratchet holds, and `cargo test -p acpi-aml-grade` passes. Run it if you can.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-acpi-aml-fix2-recheck-jarvis.md` on your branch and **push it**, with the verdict
  (CLOSED or STILL OPEN). Paste the pushed SHA to the PL.
- `cargo clean` if you built. No machine's disk is written by this lane.
