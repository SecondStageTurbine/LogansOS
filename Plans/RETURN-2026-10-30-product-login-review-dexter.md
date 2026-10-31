<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #186 security review: login in the product image

**Seat:** Dexter (DeepSeek family) · **Relay:** `#186`, slug `product-login`
**Reviewed at:** `#224` head `45798eb3598e5f36687cb410ea7496fb93131ae9` (`pr224`), and `origin/main` = `002160eba0ce3e7957b64db67dbc6fcb47ae2b5b`
**Branch:** `dexter/security-review-2026-10-30`

Every login implementation file is **byte-identical** between `pr224` and `origin/main` (`login.rs`,
`desktop_session.rs`, `lock.rs`, `kernel/src/rng/mod.rs`, `atrium/focus.rs`, `atrium/sak_logic.rs`,
`tools/xtask/src/login_scenarios.rs`), so implementation findings below hold for both. `#224`'s own delta is
27 lines of feature wiring plus a pairing test in `tools/xtask/src/main.rs`.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | Verifier stored only in derived form, salt from `kernel-rng` | **PASS** |
| 2 | Constant-time compare; backoff holds across a reboot | **FAIL** (backoff half) |
| 3 | SAK unforgeable; client cannot inject it or read lock/login keys | **PASS** |
| 4 | Skip not triggerable by an unprivileged process on a writable volume | **FAIL** |
| 5 | Without `kernel-rng`, first boot fails closed | **PASS** |

Two FAILs. **Item 4 is the gate-worthy one**: on the product image as configured, the credential record is
openly readable and replaceable by any process.

---

## 1. Is the password verifier stored only in the derived form, with the salt from `kernel-rng`? — PASS

The on-disk record is the 160-byte `NEOCRED1` slot (`userland/login-core/src/lib.rs:28-40`, layout `:94-111`):

- `verifier` = 32-byte Argon2id tag (`derive_verifier`, `lib.rs:231-242` → `vault-core::argon2id`, empty secret
  and AD), `recovery_verifier` = the tag's first 24 bytes;
- `salt` / `recovery_salt` = 16 bytes each, from `sys_getrandom`;
- `params`, `account_id` (random 16), `generation`, and a BLAKE3 checksum.

No plaintext and no reversible encoding: the password bytes never reach `encode` (`lib.rs:94-111` takes only the
tag), and the only plaintext in `.system/` is the display name in `account.meta` (`encode_meta`, `lib.rs:378-387`),
which is not a secret.

Salt provenance: `new_slot` draws `SlotRandom { salt, recovery_salt }` from `random::<SALT_LEN>()`
(`userland/neo-init/src/desktop_session/login.rs:277-285`), and `random` is `sys_getrandom`
(`login.rs:227-243`). `sys_getrandom` → `rng::fill_random` → ChaCha20 DRBG seeded from RDRAND + timer jitter,
which **panics rather than emits weak bytes** if entropy is unavailable (`kernel/src/rng/mod.rs:48-57`,
`kernel/src/arch/x86_64/syscall.rs:5490-5495`). The account id (`login.rs:570`) and recovery code
(`login.rs:291-302`, 20 Crockford-base32 symbols = 100 bits) use the same source.

Floor enforcement is real: `FLOOR = KdfParams::INTERACTIVE_V1` (64 MiB / 3 passes / 1 lane,
`userland/vault-core/src/policy.rs:21`), `decode` refuses below-floor records (`login-core/src/lib.rs:158-160`),
and `derive_verifier` derives nothing below the floor (`:236-238`).

## 2. Is the comparison constant-time, and does the backoff hold across a reboot? — FAIL

**Constant-time: yes.** `constant_time_eq` compares equal-length slices with a full-width OR fold and no early
exit (`login-core/src/lib.rs:246-251`); `verify_password` uses it on the 32-byte tag (`:254-257`), and
`verify_recovery` on the 24-byte prefix (`:261-266`). The Argon2id derivation dominates the compare anyway.

**Backoff across a reboot: no.** The failure counter is a function-local `u32` that dies with the call:

- `userland/neo-init/src/desktop_session/login.rs:588` — `let mut failures = 0u32;` inside `sign_in`;
- `:603` — `wait_out(scene, focus, lc::delay_secs(failures))`;
- `:613-616` — `failures = failures.saturating_add(1)`; nothing is written to `.system/`;
- `login-core/src/lib.rs:356-366` — `delay_secs`: free for the first 3, then 2^(n−3) s capped at 60 s;
- The lock screen is the same shape: `userland/neo-init/src/desktop_session/lock.rs:82-84` (`failures` in
  `Screen`, in memory) and `:260` (`s.failures = 0` on every lock paint);
- The module doc states the property deliberately: "the failure counter lives in its memory (§4.4): only a
  reboot resets it" (`login.rs:16`).

So a reboot returns the attacker to **three free guesses**, and the cap that would matter — the 60 s ceiling —
never survives the reset. `recover` shares the counter (`login.rs:651`), so Tab does not escape it within a boot,
which is good; the bypass is the reboot, not the screen.

**Concrete sequence:** Type 3 wrong passwords → `LOGIN password refused failures=3 next_delay_s=1`. Reset the
machine. The counter is gone; the next boot again allows 3 guesses with no delay. Repeat. Each cycle costs one
boot for 3 Argon2id evaluations at 64 MiB/3 — the per-guess rate is set by the KDF, not by `delay_secs`.

**Severity, stated honestly:** this is a policy weakness, not a break of the storage model, and the module itself
says the record cannot stop someone who boots another OS (`login.rs:490-495`). The fix is a policy call, so I am
flagging it rather than prescribing: an attempt counter persisted next to the record (the `.b` file is already the
"other" write target and survives a cut) would carry the floor across boots; leaving it as designed is defensible
if the threat model is "opportunistic local user, and rebooting is already game over".

## 3. Is the SAK unforgeable — can a client inject it, or read keys typed at the lock or login screen? — PASS

**Injection: no userland path writes the latch.** `SAK_PENDING` is written only by
`atrium::focus::latch_sak` (`kernel/src/atrium/focus.rs:630-633`) and `note_activity` (`:640-642`).
`latch_sak` has exactly two callers, both in kernel HID drivers, both from the driver-side `SakFilter`:
`kernel/src/drivers/ps2/i8042.rs:515-522` and `kernel/src/drivers/usb/hid_keyboard.rs:292`. The recognizer is
pure and fixed (`kernel/src/atrium/sak_logic.rs:47-56`, `:78-95`): modifier-first, `Super` + a table member,
swallow-release, one bit per keycode. The consumer is gated on input-system
(`SYS_INPUT_SAK_TAKE`, `kernel/src/arch/x86_64/syscall.rs:8772-8783`). The only syscall that manufactures input
events is `sys_input_focus_test_inject_impl`, compiled only under `neo-test-*` features
(`syscall.rs:8785-8803`) — absent from the product image — and even it routes an event rather than latching a chord.
`latch_sak` is a lock-free `fetch_or`, so a chord cannot be lost to `DELIVERY` contention (the Jarvis F1/F5 fix).

**Reading keys at the lock/login screen: no.** Both screens read from the WM's own focus `HomeFocus.events`
(`login.rs:418`, `lock.rs:284`), which is the client root returned by `sys_input_focus_create` and activated at
session start (`userland/neo-init/src/desktop_session.rs:3350-3357`). Locking re-activates that same focus
(`lock.rs:233-239`). Reading requires a RECEIVE cap on the *active* object: `sys_input_focus_recv` demands
`RECEIVE` and no `ACTIVATE` (`syscall.rs:8591-8607`) and `try_recv` re-checks `is_active`
(`kernel/src/atrium/focus.rs:246-252`, `:256-274`). What a client is given is the **ACTIVATE-only** sibling
(`desktop_session.rs:1305`, `:1310`), which cannot RECEIVE. And a client cannot move the route back onto itself
while the lock is up, because `SYS_INPUT_FOCUS_ACTIVATE` needs input-system (`syscall.rs:8495-8497`) — the same
gate as #185 item 1.

Metadata, not keystrokes: the `sak-lock` activity bit is latched for *every* produced event, including ones routed
to a client (`focus.rs:310-314`, `:635-642`). That is by design (the idle timer must see client-routed input) and
reveals only "input happened".

## 4. Can the skip be triggered by an unprivileged process on a writable volume? — FAIL

The literal skip branch is not reachable by deletion, but **the record is touchable**, and touching it reaches a
strictly worse outcome than the skip. So the property the item asks me to verify — *the gate cannot be defeated by
an unprivileged process on a writable volume* — does not hold.

### What is not reachable (good)

- The ABI has **no** `unlink`, `rename`, `mkdir`, `chmod`, or `link`. `SYS_OPEN` (12), `SYS_READDIR` (16),
  `SYS_FTRUNCATE` (23), `SYS_FILE_REPLACE` (24) are the whole namespace surface
  (`userland/libneo/src/lib.rs:120-134`). So `.system/` cannot be removed and the record cannot be made
  "unwritable" — writability is per-open, not a per-inode attribute.
- `mode()`'s two skip exits are `login.rs:123` (`.system/` absent) and `login.rs:141-146` (`open_rw` failed).
  With the files present, `open_rw` (`login.rs:155-161`) is a plain `sys_open(READ|WRITE)` and succeeds on a
  writable volume regardless of how full it is (an open allocates nothing).

So the answer to the parenthetical example is: no, a process cannot delete or lock `.system/`.

### What is reachable (the finding)

**Root-form file opens are ambient in the product image.** `open_path_and_install` gates the root form on a
`RootDirCap` only under `#[cfg(feature = "cap-rooted-open")]`
(`kernel/src/arch/x86_64/syscall.rs:6323-6328`, and `parent = Some(root_cap)` at `:6360-6361` vs `None` at
`:6362-6363`). That feature is **off by default** — `cap-rooted-open = ["file-caps"]`, and `kernel/Cargo.toml`
says so explicitly: *"OFF by default: the root form stays open to all, bit-for-bit the previous layer."* It is
enabled only by `neo-test-*` features. It is **not** in `PRODUCT_IMAGE_KERNEL_FEATURES`
(`tools/xtask/src/main.rs:5828-5896` as of `pr224`), and `#224` does not add it; `login` in neo-init depends only
on `desktop-intro` + `login-core` (`userland/neo-init/Cargo.toml`, `login = ["desktop-intro", "dep:login-core"]`).

With the feature off, **any process can open any root-form path with any rights**. `root_dir_cap_for`
(`kernel/src/sched/process.rs:1224-1248`) is not consulted at all.

### Concrete sequence

From any unprivileged client (Arthur, the shell — no spawn needed; they are already running, and `sys_spawn` is
itself still advisory-ungated per `userland/neo-init/src/desktop_session.rs:2187`):

1. `SYS_OPEN("/neofs/.system/account.cred.a", OPEN_ROOT_TEMPORARY, READ|WRITE)` → `0` (an `fd`).
   `fs::open_root_temporary` resolves from `/` and consults no process.
2. Read the 160 bytes: **the Argon2id verifier, its salt, and the KDF parameters**. That is exactly the offline
   attack the derived-form storage is supposed to make safe — 64 MiB/3 passes per guess is a cost, not immunity,
   and a weak password falls quickly.
3. Overwrite with `SYS_WRITE` + `SYS_FTRUNCATE` (or `SYS_FILE_REPLACE` where `neofs-file-replace` is on), e.g.
   write 160 zero bytes. The checksum no longer matches.
4. Next boot: `decode` fails (`login-core/src/lib.rs:125-127`), so `select` returns `None`
   (`:209-227`), `mode()` reaches `Mode::FirstBoot` (`login.rs:147-150`) — **not** the skip — and `first_boot`
   writes a brand-new record to both files (`login.rs:574-577`). The existing account is gone, and whoever is at
   the keyboard chooses the new password.

That is the "file it shouldn't be able to touch" the item points at, one branch over from the skip: the outcome is
a wiped account plus exposed verifier material, and `Mode::Skip` is only not reached because the code re-selects
after the probe. A process that instead exhausts the volume makes the *skip* reachable too: with no record present,
`open_rw` falls to `OPEN_CREATE_REGULAR` (`login.rs:158`) and an `ENOSPC` there returns `Err` →
`Mode::Skip(read-only)` → the session starts with **no sign-in at all** on a writable volume.

### Why this is in scope for #186

`#224` puts `login` in the product image for the first time. The login's own promise — "It protects your files
while LogansOS runs" (`login.rs:491-495`) — is precisely the window in which this is false for the credential
file. The gate that closes it (`cap-rooted-open`, OT-66 K1a) exists, is written, and is tested
(`neo-test-cap-rooted-open`, `tools/xtask/src/main.rs:2581-2582`); it is simply not in the shipped feature list.
**Recommendation:** either add `cap-rooted-open` to `PRODUCT_IMAGE_KERNEL_FEATURES` (and the matching `cap-rooted-open`
userland feature, as `tools/xtask/src/main.rs:6827-6829` already does for test kernels), or record that the product
ships the ambient-root form as a known, deliberate limitation. This is a PL decision, but it should be an explicit
one, not a side effect of a feature list.

## 5. Without `kernel-rng`, does first boot fail closed? — PASS

Yes, and it fails before writing anything.

- `sys_getrandom` without the feature is `ENOSYS` (`kernel/src/arch/x86_64/syscall.rs:5510-5513`).
- `random()` treats every non-positive return as a failed try and gives up with `Err(180)` after 64
  (`login.rs:231-241`).
- On first boot the first draw is `issue_code(1)?` (`login.rs:567`), which reaches `random::<CODE_LEN>()`
  (`login.rs:300`); it returns `Err(180)` and `first_boot` propagates — **before** `new_slot` and before either
  `write_slot`, so no partial account is created.
- `gate` propagates to its caller, `desktop_session.rs:1071-1073`, which does `return code` — the session never
  reaches `show_home` (`:1079-1086`). No desktop, no unauthenticated session.

The pairing test is the guard that keeps this from shipping: `product_image_pairs_login_with_kernel_rng`
(`tools/xtask/src/main.rs:20445-20461` in `pr224`) asserts `kernel-rng` and `sak-lock` are in
`PRODUCT_IMAGE_KERNEL_FEATURES` and `login` in `desktop-with-model`'s init features, added alongside
`"kernel-rng"` / `"sak-lock"` at `:5890-5895` and `"login"` at `:6411-6419`.

Two notes on the guard, neither a FAIL: the test asserts *feature-string presence*, not behaviour, so it cannot
catch a build that satisfies the list but loses the syscall; and the fail-closed path is late — the user is shown
the "Welcome to LogansOS / What should I call you?" and password screens (`login.rs:548-566`) before the failure at
`:567`. Closed, but the failure should ideally be detected in `mode()` so it is not discovered after the user has
typed a password.

---

## Notes for the PM

- **Dispatch path error.** `kernel/src/input/focus.rs` (named in the dispatch for item 3) does not exist at
  `origin/main` or `45798eb3`. The SAK route lives in `kernel/src/atrium/sak_logic.rs`,
  `kernel/src/atrium/focus.rs`, and `userland/neo-init/src/desktop_session/lock.rs`. I reviewed those.
- `#224` head `45798eb3` is **not** an ancestor of `origin/main` (`origin/main` is 20 commits ahead of its
  merge-base `d6140243`), and its own delta is 27 lines in `tools/xtask/src/main.rs`. All six login implementation
  files are identical between the two refs, so the review is valid for both; the PM should still merge `#224`
  after a rebase, not assume the gate ran on a current base.
- RAG health-checked before use: `{"status":"healthy","ollama":true,"qdrant":true}`. Source reads were direct.
- **Suggested disposition:** item 4 blocks (or needs an explicit PL acceptance recorded in the ledger); item 2's
  backoff is a policy decision for J.c.; items 1, 3, 5 are clean.
