<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Return: #9 and #20 THINK re-check (Chad)

Heads read: #9 `12ada069e`; #20 `cfcdf7fb1`.
Earlier findings: `Plans/RETURN-2026-11-01-pr9-pr20-storage-thinks-review-chad.md`.

- [x] **PASS — #9 dirty-flag discriminators.** `docs/THINK-2026-10-31-l3a-fat-readonly.md:66`: the host KAT uses clean, BPB-only, FAT[1]-only, and both-dirty FAT16/FAT32 fixtures and checks each logged pair; it also checks hard-error handling. The gate scenario checks the BPB-only pair and unchanged boot/FAT/FSInfo hashes, with zero write attempts.
- [x] **PASS — #9 bad BPB geometry.** `docs/THINK-2026-10-31-l3a-fat-readonly.md:74`: field-by-field KATs cover zero and invalid sector sizes, invalid cluster sizes, reserved/FAT counts, FAT type/root-entry shape, root-sector alignment, total-sector consistency, zero FAT size, and geometry with no data region. Each malformed FAT16/FAT32 fixture must refuse at sector 0 without panic; rust-fatfs is also checked against panics.
- [x] **PASS — #9 read-only enforcement and kill-switch.** `docs/THINK-2026-10-31-l3a-fat-readonly.md:95-107,116-119`: the revision explains that rust-fatfs requires an adapter `Write` implementation, while `BlockIo` only holds `ReadOnlyDevice`, which exposes no device write. Feature-off consistently retains the native FAT32 driver, with a native-control scenario to gate that branch.
- [x] **PASS — #20 transfer boundary.** `docs/THINK-2026-10-31-l3u-usb-mass-storage.md:57,70-71`: the read boundary refuses a one-past-last-LBA request and a two-block read extending past the end before issuing a command. The 300-block KAT requires exactly 128 + 128 + 44 blocks, and checks the READ(10) maximum-LBA boundary.
- [x] **PASS — #20 unsupported-command discriminator.** `docs/THINK-2026-10-31-l3u-usb-mass-storage.md:69`: a scripted device that stalls unsupported commands checks that only the five listed command families reach the pipe; MODE SENSE, SYNCHRONIZE CACHE, PREVENT/ALLOW, START STOP UNIT, and WRITE(10) cause zero pipe calls.
- [x] **PASS — #20 slow spin-up.** `docs/THINK-2026-10-31-l3u-usb-mass-storage.md:52`: sense `04/01` waits about 250 ms and retries at most 40 times; UNIT ATTENTION retries are capped at five, while `04/02` and `3A` decline without retrying.

New change introduced by these fixes: the expanded T13 geometry KAT and the USB allowlist/boundary cases are specified alongside the prior scenarios; I found no new contradictory behavior in the assigned revisions.
