<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: narrow re-check of the write-guard fixes (#85, F1–F4) → Janus (Codex)

**PR:** #85, `claude/write-guard-hardening`. **Your RETURN:** `Plans/RETURN-2026-10-21-write-guard-review-janus.md`
on `codex/write-guard-review-janus` (`fde77daa`).

```
git fetch origin
git show origin/claude/write-guard-hardening:Plans/DISPATCH-2026-10-21-write-guard-recheck-janus.md
git diff 6bfcfbea origin/claude/write-guard-hardening -- kernel tools
```

WORKTREE: `E:\AI Automation\workers\janus` (yours alone; never another coder's folder, never the repo root)
BRANCH: `codex/write-guard-recheck-janus` (`git fetch origin && git checkout -B codex/write-guard-recheck-janus origin/claude/write-guard-hardening`)

## The question

This is a safety review. Your earlier review found four places where the protection for the PL's own
Windows laptop disk was weaker than intended. Please confirm that each protection now holds, and say
where it still does not.

| Item | What should now be true | Where | Evidence to check |
|---|---|---|---|
| **F1** | A GPT counts only when one copy is fully consistent: header CRC32, its own LBA, usable range and entry array in bounds, entry-array CRC32, and every partition inside the usable range. The primary is tried, then the backup in the last LBA. A GPT signature or protective MBR with no consistent copy protects the whole disk (`table=opaque-gpt-invalid`) | `write_guard_logic.rs`: `read_table`, `validated_gpt`, `crc32` | `cargo test -p block-guard-logic-tests` (19). `f1_a…f1_e`; your moved-entry-array case is `f1_a`. Also `f1_structurally_invalid_gpts_are_opaque`, `crc32_known_answers` (check value plus a real header from our ISO) and `sink_beyond_our_iso_gpt_arms` (the OptiPlex layout still works) |
| **F2** | No build that boots on real hardware can have the guard off, whatever the feature is called | kernel `drivers/block/mod.rs`: a `compile_error!` on resolved features `neo-test-block-write-guard-control` + `neo-test-bare-metal-mode`; xtask `feature_disables_write_guard` (last spec component); xtask `refuse_guard_off_image` (the control kernel carries `GUARD_OFF_MARKER`, and a bare-metal bundle containing it is refused and removed) | `cargo test -p xtask guard` (3). QEMU runs with the package-qualified and the whitespace spellings under `--bare-metal-mode`, both refused |
| **F3** | Every write in a bare-metal build goes through `GuardedDevice` | `virtio_blk::debug_submit_abandoned_write` asks `guard_allows_raw_write` and is compiled out of bare-metal builds; its scenario `compile_error!`s with bare-metal mode | Grep: the write opcodes (`NVM_WRITE`, `ATA_WRITE_DMA_EXT`, `VIRTIO_BLK_T_OUT`) are reached only from each driver's `submit`, which only `GuardedDevice` calls. Is there any other caller? |
| **F4** | A CPU that panics while holding the serial lock still gets the panic onto the screen, and onto the UART | `serial::enter_panic` and `print_panicking`: screen first, then `try_lock`, else a direct UART write; `panic.rs` calls it after `panic_takeover` | Scenario `fb_panic_holding_serial` (PASS with `serial_lock_held=yes`); its control `-control` restores the blocking acquire and must time out |

Please check:
1. Is any consistent-looking GPT still able to make the Windows layout read as having free space where it
   does not, without fixing both CRCs?
2. Can any build path that produces a bare-metal image still turn the guard off?
3. Is there any block write in a bare-metal build that does not pass `GuardedDevice`?
4. On the panic path, is there any other lock the first panic line can wait on before reaching the
   screen?

The head also carries the fixes for the second reviewer's (R1) pass 1, which you may also find useful
to confirm:
- **R1-1:** a disk with no table but a boot signature or a whole-disk volume is protected
  (`opaque-filesystem` / `opaque-boot-record`), and a malformed MBR is protected (`opaque-mbr-invalid`).
- **R1-2:** hybrid-MBR partitions count beside a valid GPT.
- **R1-3:** after a panic, other CPUs halt at their next draw and cannot switch the mirror off.
  `fb_panic_holding_serial` runs a CPU-1 drawer; control `neo-test-fb-panic-other-cpu-control`.
- **R1-4:** `log-sink-header` writes only regular files.
- **R1-5:** the guard-off control `compile_error!`s with `product` or `fb-progress-console` too, and only
  unguards virtio-blk.
- **R1-6:** only the primary NEOFS superblock sets the volume.

Host KATs: `block-guard-logic-tests` 25, `xtask` 47.

Note on scope: feature-off builds (no `fb-progress-console`) keep the panic path exactly as on `main`,
which still blocks on `SERIAL`. That is pre-existing, and kept for the kill-switch byte identity. Say if
you think it should change in this PR.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='GPT header CRC validation partition table'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-21-write-guard-recheck-janus.md` to `codex/write-guard-recheck-janus` and
push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- F1–F4 each closed or not, with evidence;
- any new finding with file:line and a repro.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
