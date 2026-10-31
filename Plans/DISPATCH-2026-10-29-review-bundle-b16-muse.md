<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #150–#152) → Muse: the copy-bounds fix, your K1a follow-ups, and login L1

Three security reviews of Claude-authored code. All three merged after batch 16 (run `40843436986`, green at
`fb039fbd`). **Read the files at each squash SHA from the repo, not a summary.** The PL pastes your RETURNs back, and
the PM banks them. Write each as a plain checklist: pass or fail, the enforcing `file:line`, and a concrete input for
any fail.

## #150: #201 `1faa0bec`, `copy_user_bytes` is fallible and every caller bounds its length. Banked as `Plans/RETURN-2026-10-29-copy-user-bytes-review-muse.md`

The defect: any process could panic the product kernel with one `SYS_TENSOR_WRITE` or `SYS_TENSOR_CREATE` whose length
exceeded the kernel heap. The copy happened before any handle or length check, and the allocation was infallible. The
PR body has a table of all 25 call sites with their bounds. File: `kernel/src/arch/x86_64/syscall.rs`.
1. Can any `copy_user_bytes` caller still reach the kernel heap with a length its ABI doesn't bound? Use the table.
2. Can `sys_tensor_write_impl` copy more than the minted tensor holds, for any `offset`, including overflow? Are the
   handle lookup and the WRITE right checked before the copy?
3. Does any path still allocate a caller-chosen length infallibly on the kernel heap, outside `copy_user_bytes`?
4. Are the tensor errors unchanged for valid inputs (rights, immutability, ranges)?
5. Stated limit: a valid tensor bigger than the free heap now gets ENOMEM. Is that fail-closed everywhere, with no
   partial write?

## #151: #197 `cbe57444`, your #166/#168 follow-ups. Banked as `Plans/RETURN-2026-10-29-k1a-followups-recheck-muse.md`

Your RETURNs: `Plans/RETURN-2026-10-27-k1a0-revoke-review-muse.md` and `...-k1a-rootdircap-review-muse.md`. Your
`is_directory()` point was already fixed by #174 (`57b7960e`).
1. Can any code outside `process::edges` reach the derivations map, or change an edge without a `CapGraph` guard?
   Check every method of `Derivations`, including the test-only ones.
2. Is `insert_without_cap_graph` reachable outside `neo-test-missing-cap-lock-negative`?
3. Does `CapGraph::retire` hold any `Arc<Process>` that could be the last reference when its `Vec` drops?
4. Does the ADR-010 text now match `retire_one` and `teardown`?

## #152: #198 `80fa7b1b`, OT-66 L1: credentials and the login screen. Banked as `Plans/RETURN-2026-10-29-ot66-l1-review-muse.md`

The design is `docs/THINK-2026-10-27-ot66-login-session.md` (§4, §5, §7 and §9). It's behind `login`, which is off in the
product. J.c. decides whether to turn it on after your review. That would add kernel-rng to the product kernel.
1. **The record.** Does `NEOCRED1`'s reader refuse a slot whose magic, version or checksum is wrong, or whose
   parameters are below `KdfParams::INTERACTIVE_V1`? Is the highest valid generation taken? Does the hand-laid oracle
   match §4.3?
2. **The KDF.** It's one Argon2id configuration through vault-core's entry point. Are the verifier comparisons constant
   time? Is the 64 MiB transient freed before the session starts?
3. **The cut guarantee.** A password change writes only the older file. Can any path write the newer file, or both?
   Does `login_cut_steps` cover every write and flush?
4. **The recovery code.** 20 symbols from 32 come from `sys_getrandom`; it's shown once, stores only its verifier, and
   is reissued after use. Can it be recovered from the record or a log? What happens if `sys_getrandom` fails?
5. **The backoff.** 2ⁿ⁻³ s after the 3rd failure, capped at 60 s, with the counter in neo-init's memory. Can a client
   reset it, or reach the verifier without passing it?
6. **The first-boot screen.** Can a second boot re-enter account creation and overwrite an existing account?
7. **Input.** While the login screen is up, can any client receive keys or be shown, as §2 promises?
