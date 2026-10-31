<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #152: #198 `80fa7b1b`, OT-66 L1: credentials and the login screen

Reviewer: Muse (Gwen), reviewing Claude-authored code. Pasted back by the PL on 2026-10-29 and banked by the PM
verbatim. PM note: the kernel-rng condition in the header is already met by S2's product-login branch
`claude/ot66-l1-product-login` @ `3a6e86d0`. That branch adds `kernel-rng` and `sak-lock` to
`PRODUCT_IMAGE_KERNEL_FEATURES` alongside the WM's `login`, with a test, `product_image_pairs_login_with_kernel_rng`,
that keeps them paired.

Read at squash SHA `80fa7b1b`, not from `main` and not from a summary. Design:
`docs/THINK-2026-10-27-ot66-login-session.md` §4, §5, §7 and §9 at that SHA. Behind `login`, which is off in the
product: the product feature list at this SHA (`tools/xtask/src/main.rs`, `PRODUCT_IMAGE_KERNEL_FEATURES`) contains
neither `login` nor `kernel-rng`.

The kernel-rng implication, stated plainly because J.c. decides on it: turning `login` on requires `kernel-rng` in
the product kernel build. `sys_getrandom` without `kernel-rng` is `ENOSYS` (`kernel/src/arch/x86_64/syscall.rs`,
`sys_getrandom_impl`), and neo-init's `random()` (`userland/neo-init/src/desktop_session/login.rs:223`) then fails
every account creation and recovery with `Err(180)` after 64 tries — fail-closed (no session, no fallback code), but
first boot and password recovery would both be broken, not merely weakened. Sign-in itself needs no randomness and
would still work. The `neo-test-login` test build already pulls `kernel-rng` (`kernel/Cargo.toml:851`).

1. The record — PASS. `NEOCRED1`'s reader refuses every malformed slot, and takes the highest valid generation:
   * `decode` (`userland/login-core/src/lib.rs:115`) checks, in order: exact length 160, magic `NEOCRED1`, version 1,
     then the checksum — the first 16 bytes of BLAKE3 over bytes 0..136 — before any field is trusted, then zero
     reserved bytes 152..160, then flags (unknown bits refused; recovery fields present without the flag refused), then
     generation 0, then the floor, last. A torn write fails the checksum; a short file fails length; both are refused
     alike.
   * Below the floor is refused, not honoured: `meets_floor` (`userland/vault-core/src/policy.rs:28`) requires every
     dimension at or above `FLOOR`, and `FLOOR` is exactly `KdfParams::INTERACTIVE_V1` (64 MiB, 3 passes, 1 lane,
     `policy.rs:21`) — so "below `INTERACTIVE_V1`" and "below the floor" are the same refusal, enforced both in
     `decode` and in `derive_verifier` (`login-core/src/lib.rs:231`).
   * `select` (`login-core/src/lib.rs:209`) takes the valid slot with the highest generation (ties pick `a`, the
     creation-time case); a file that fails `decode` is simply not a candidate, so a torn newer file falls back to the
     older one and two torn files yield no record — never a default.
   * The hand-laid oracle matches §4.3 field for field: the §4.3 table's offsets (magic 0, version 8, flags 10, memory
     12, passes 16, lanes 20, account id 24, salt 40, verifier 56, recovery salt 88, recovery verifier 104, generation
     128, checksum 136, reserved 152) are exactly `encode`'s layout, and `userland/login-core/src/tests.rs` lays the
     slots out by hand from the table (not via `encode`), with verifiers from the P-H-C reference `argon2` CLI at its
     pinned commit and checksums from standalone BLAKE3. The tests assert the round trip, the reference verifiers
     (`oracle_verifiers_are_the_reference_cli_s`), the torn-newer fallback, and the below-floor refusal even when
     newest (`FLOOR_GEN3`, 32 MiB, generation 3, refused while generation 1 stands).
2. The KDF — PASS. One Argon2id configuration through vault-core's entry point: every derivation in the login path goes
   through `derive_verifier` (`login-core/src/lib.rs:231`) into `vault_core::argon2id`
   (`userland/vault-core/src/lib.rs:87`, Argon2id, version 0x13, RFC 9106), with the floor re-checked before deriving.
   The comparisons are constant time: `constant_time_eq` (`login-core/src/lib.rs:246`) OR-accumulates every byte with
   no content-dependent early exit, and both operands are fixed-length (32-byte password tags, 24-byte recovery tags),
   so the length branch cannot vary on a real input. The 64 MiB transient is freed before the session starts: the
   Argon2 working memory is allocated inside `hash_password_into` and freed when `argon2id` returns — every derivation
   happens inside `gate()`, and the session only starts after `gate` returns `Ok` (`desktop_session.rs:1051`, then
   intro and Home). The derived tags are `Zeroizing` and drop after each comparison; the tuning log line carries
   parameters and milliseconds only, never a secret. Observation, not a failure: the record slot itself (`Current`,
   plain bytes including the verifier) is dropped, not wiped, when `sign_in` returns — it lives only in WM heap memory
   no client can read, but it is not zeroized.
3. The cut guarantee — PASS. A password change writes only the older file, and `login_cut_steps` covers every write
   and flush:
   * `change` (`userland/neo-init/src/desktop_session/login.rs:657`) computes generation + 1 with `checked_add`
     (overflow refuses, never wraps), builds the new slot, and writes exactly `current.write_target()` — the file not
     holding the record (`login-core/src/lib.rs`, `write_target` = the other file) — via `write_slot` (`login.rs:301`),
     which writes the whole 160 bytes and `fsync`s. The newer file is never opened for writing by the change path, and
     `account.meta` is not rewritten on a change.
   * No other writer exists: `write_slot` is `pub(super)`, and its only callers in the tree are in `login.rs` itself —
     creation (`first_boot`, both files, generation 1, as §4.3 sanctions), the test-only below-floor seeder, the
     cut-test setup, and `change`. `desktop_session.rs` has no credential-file writes at all.
   * `login_cut_steps` (`tools/xtask/src/login_scenarios.rs:388`, `cut_steps`): a calibration boot runs the whole
     change uncut and counts its device writes and flushes (`ops`, refused unless 1..=200); with `NEO_LOGIN_CUT_ALL=1`
     the sweep cuts at every step 1..=ops, and every cut boot must then verify exactly one of the old and new
     passwords accepted, with the file the change doesn't write byte-identical (BLAKE3 before and after). The uncut
     change must leave the new password in file `b`, generation 2, with `a` unchanged. Caveat, stated plainly: without
     that env var the default run samples rather than sweeping — every flush, plus the first and last sector of each
     consecutive-sector run — so "every write and flush" is the `NEO_LOGIN_CUT_ALL=1` run. The kernel cut hook itself
     (`SYS_TEST_BLOCK_CUT`) was not re-read, but each cut boot asserts the cut fired at its step, so the sampled steps
     are proven cuts, not assumed ones.
4. The recovery code — PASS. 20 symbols from 32, from `sys_getrandom`, shown once, only its verifier stored, reissued
   after use — and unrecoverable from the record or a log:
   * Production issuance is `lc::recovery_code(&random::<CODE_LEN>()?)` (`login.rs:287`, `issue_code`): 20 random
     bytes, each byte's low five bits picking a symbol. 256 is a multiple of 32, so every symbol is equally likely; the
     100-bit code is Crockford Base32 without I, L, O, U.
   * The record stores only the recovery salt and the first 24 bytes of the Argon2id tag (`RECOVERY_VERIFIER_LEN`,
     `login-core/src/lib.rs`), never the code. Recovering the code from a truncated Argon2id tag is a preimage attack on
     Argon2id, not a lookup. No log carries the code: `show_code` (`login.rs:500`) logs only `LOGIN recovery code
     shown`, and the recovery log lines carry counts and delays only. The code is displayed on the card until Enter and
     then replaced by the setup card — it is not re-shown anywhere afterwards.
   * Reissue after use: `recover` (`login.rs:615`) issues a new code at generation + 1 and `change()` stores its
     verifier in the other file, so the used code's verifier is replaced; the two-boot scenario asserts the used code
     is refused on the next boot.
   * If `sys_getrandom` fails: `random()` (`login.rs:223`) retries up to 64 failed or short returns, then `Err(180)`,
     which propagates with `?` out of `first_boot` / `recover` / `new_slot` into `gate`'s `Err`, and `run()` returns
     the code without starting a session (`desktop_session.rs:1051`). There is no fallback code, no predictable code,
     and no partial account: creation and recovery fail closed. This includes the `ENOSYS` case when `kernel-rng` is
     off (see the header note).
   * Observations, not failures: (a) if `change()`'s write fails after `show_code`, the shown new code was never stored
     — the old code stays valid and the written-down new code never works. A robustness wart, not a bypass: no code
     grants access that shouldn't. (b) The shown code bytes are not wiped from WM stack memory after display (the
     formatted array is plain bytes, not `Zeroizing`); WM memory is not client-readable, so this is hygiene, not
     exposure. (c) Boot-test builds issue codes from a fixed two-code list (`desktop-session-boot-test` cfg) —
     test-only; production builds draw from `sys_getrandom`.
5. The backoff — PASS. The schedule, the counter's home, and the verifier gate all hold:
   * `delay_secs` (`login-core/src/lib.rs:356`) is 0 for the first three failures, then 2^(n−3) seconds, capped at 60
     — the KAT table in `tests.rs` (`backoff_schedule_kat`) asserts 0, 0, 0, 1, 2, 4, 8, 16, 32, 60, 60, 60 and
     `u32::MAX` → 60, with `saturating_add` on the counter so it cannot wrap.
   * `wait_out` (`login.rs:430`) runs before every check — password and recovery alike — and keys typed during the
     wait are read and dropped, never queued for the check. The counter is a stack local in `sign_in`
     (`login.rs:564`), shared by the password and recovery attempts, living in neo-init's memory: no client exists yet
     during sign-in (item 7), no client can write WM memory, and neo-init is not restartable — the kernel reaps it into
     kernel-only mode with no respawn (`kernel/src/main.rs:504-517`). Only a reboot resets the counter, and a reboot
     costs a full boot, slower than the 60 s cap, so rebooting doesn't beat the delay. There is no permanent lockout,
     by design.
   * No path reaches the verifier without the wait: the only comparisons are `verify_password` / `verify_recovery`
     inside `sign_in` / `recover`, both after `wait_out`. A client cannot reset the counter (it cannot touch neo-init's
     memory or respawn it) and cannot invoke the verifier (it is a WM-local function, not a syscall).
   * Scope note, not a failure: the backoff binds the online screen only. Anyone who can read the record file can guess
     offline at Argon2 cost — THINK §2 states this plainly (client reads of the credential file are not protected until
     K1b; Try and disk removal read it too). That is the floor's job, not the backoff's, and it is not a backoff bypass.
6. The first-boot screen — PASS. A second boot cannot re-enter account creation over an existing account:
   * `mode()` (`login.rs:114`) decides before Home is shown: no `.system` directory → skip (Try); volume read-only (the
     current file, or `a` if none, cannot be opened for writing) → skip, so Try never signs in; no valid record on a
     writable volume → first-boot screen; a valid record → the login screen. After `first_boot` writes both files at
     generation 1, the next boot's `select` finds a valid record and takes the `SignIn` branch — creation is
     unreachable with a valid record present.
   * The only path back to first-boot is both files invalid (corrupt, or below the floor — the `login_below_floor` boot
     test asserts exactly this: refused records show the first-boot screen). That is by-design recovery for a machine
     with no usable record, and reaching it requires invalidating both files, which requires a writer with access to
     `.system` — the only write path pre-login is neo-init itself (item 3). An existing valid account is never
     overwritten by a second boot.
7. Input — PASS. While the login screen is up, no client can receive keys or be shown, as §2 promises — structurally,
   because no client exists yet:
   * The WM claims and activates its own focus before the gate: `home::claim_focus` (`desktop_session.rs:3279`)
     creates a focus, mints an activator and activates it, and `run()` calls `login::gate` with that focus
     (`desktop_session.rs:1051`) before intro, before `show_home`, and before any client spawn — Arthur is spawned only
     later, in the Home→Arthur round (`desktop_session.rs:1158`), after the gate returns `Ok`. The first composited
     frame after the gate is captioned "before any client exists."
   * `answer()` (`login.rs:388`) reads only the WM focus's events (`sys_input_focus_recv(focus.events, ...)`); no client
     focus object exists to receive anything, and the WM never forwards keys anywhere during the gate. The pointer
     route is the WM's (claimed at `run()` start) and `idle()` drains it via `pointer_idle`.
   * What is shown is WM chrome only: `begin()` shows Home with the login card and presents it; no client surface
     exists to composite. A gate failure ends `run()` with the error code before any client, intro, or Home frame — so
     a failed login never degrades to an unsigned-in desktop.
   * Limitation, stated plainly: this is the login screen's own isolation (no clients exist). The lock screen's
     isolation with live clients is L2 (#208, a separate review), not re-judged here.

Overall verdict: PASS. All 7 items pass. No FAIL was found: no concrete input was identified that the code at
`80fa7b1b` fails. Recorded observations (the unwiped slot/code bytes in WM memory, the shown-but-unstored code on a
failed recovery write, the sampled default cut run, the online-only scope of the backoff) do not change the verdict.
The one decision this RETURN puts to J.c. is the header note: turning `login` on adds `kernel-rng` to the product
kernel, and without it first boot and recovery fail closed rather than work.
