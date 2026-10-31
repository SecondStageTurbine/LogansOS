<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-66, login, session and a human user

**Date:** 2026-10-27 · **Author:** S2 · **Dispatch:** `Plans/DISPATCH-2026-10-27-ot66-login-think-s2.md` ·
**Status:** design only, no code. Revision 2 answers the cross-family read (Chad, relay #98,
`Plans/RETURN-2026-10-27-ot66-login-think-review-chad.md`: items 1, 5, 7, 8 and 9 passed; 2, 3, 4 and 6 failed)
and records the K1 split (K1a-0 and K1a landed). A narrow re-check of those items follows.

**The ask.** The MVP tier (`Plans/DECISION-2026-10-27-release-tiers-mvp.md`, PL 2026-10-27) includes **log in**.
OT-66 (`docs/OPEN-THREADS.md:76`) records that there is no human-user object at all:
- no credential store for OS sign-in;
- no login or lock screen;
- no session broker beyond DS-1's advisory one;
- no logout.

**Code is cited at `origin/main` `beb392c4`** unless a line says otherwise. **Revision 2's new citations** (K1a, NeoFS's write path, neo-init's exit, the busy emblem) **are at `origin/main` `000eeadb`.**

## 0. Summary

1. **A user is a userland record, not a kernel object.** The kernel keeps no user, no uid and no ambient
   identity. A *session* is the set of processes the session broker spawns for one signed-in account, and its
   authority is exactly the capabilities the broker hands those processes at spawn. That is the Fuchsia/Genode
   shape (§11), and it is how the kernel already works: capabilities, inherited at spawn, derivable and revocable
   (ADR-010).
2. **But at `beb392c4` that authority wasn't the only authority.** Three ambient paths bypassed it:
   - any process could `sys_spawn` any module by name (`kernel/src/arch/x86_64/syscall.rs:1630-1700`);
   - any process could open any path from the root (`sys_open` with `OPEN_ROOT_TEMPORARY`, which
     `kernel/src/fs/mod.rs:1186-1189` itself calls temporary, the target being a capability-rooted open);
   - `sys_model_load` took a path and opened it through the same root form (Chad, item 3).

   **K1a (#168, `2922bf0c`) has since closed the second and third** under `cap-rooted-open`: the root form, and
   every model load, needs a `RootDirCap` (§3.4). `sys_spawn` is still open until K2.

   Until the root cap is held only by processes that should have it (K1b) and spawn is gated (K2), a password
   protects **the screen, not the data**: any process holding the root cap, or able to spawn one that does, could
   read or replace the credential file. The ladder says so, pins it in a test (the DS-1b precedent), and closes it
   with **K1** (capability-rooted open: K1a-0 and K1a landed, K1b open) and **K2** (`SpawnCap`). Both change the
   kernel's capability model, so both take the concurrency pitfall-bank pass, a cross-family AAG and a Muse review.
3. **Credentials:** one local account for the MVP.
   - **Hash:** Argon2id from the `argon2` crate the workspace already uses (`userland/vault-core/src/lib.rs:13,97`).
   - **Parameters:** never below vault-core's floor, 64 MiB, 3 passes, 1 lane (`policy.rs:17-21`). They're tuned up
     from that floor by a timing measured on the Zenbook and the OptiPlex.
   - **Record:** a fixed-layout `NEOCRED1` record with a hand-laid byte oracle, kept as two files (`.system/account.cred.a`
     and `.b`), so a change writes only one of them and a crash keeps the other (§4.3).
   - **Wrong passwords** get a growing delay, counted in a persisted `NEOFAIL1` record that a reboot doesn't
     reset (J.c., 2026-10-30), and never a permanent lockout.
   - **A forgotten password** is reset with a one-time recovery code shown at account creation.
4. **Screens.** The login screen, the lock screen and the logout confirmation are **WM chrome**: drawn by neo-init,
   never a client. Locked and signed out are one **exclusive compositor state**: the WM holds the only active input
   focus, so the kernel's focus gate stops every client from receiving a key, and only the WM's own lock or login
   surface is composited, with no client pixels, no busy emblem and nothing from a client started meanwhile (§5.2).
5. **Arthur** is per user: his memory and grants move under the account's home. The model is per machine. Before
   sign-in, Arthur isn't running and sees nothing.
6. **The installer and Try.**
   - **Account creation** is a first-boot screen on the installed system, not an installer step.
   - **Try** has no account and no login. It stays today's straight-to-desktop session, and nothing in it persists
     (OT-87 §6.3 mounts found volumes read-only).

## 1. What exists today

| Piece | Where | State |
|---|---|---|
| Boot status line standing in for a login prompt | `kernel/src/init/boot_frame.rs:109-117` | A TODO ("until Phase 1 replaces this with a real login prompt") |
| Kernel principal | `ProcessId`; handles in a per-process table | No user, uid or session id anywhere in the kernel |
| Spawn | `sys_spawn_impl`, `syscall.rs:1630` | **Ambient:** any process names any module. Inherited handles must carry `GRANT` (`:1689`), and file caps only as a spawn-inheritable directory (`:1677,1738`) |
| Open | `open_root_temporary`, `fs/mod.rs:1186-1200` | **Ambient:** any process opens any absolute path. Rights are capped by `FileRights::KNOWN_BUILD4B`, not by who asks. No directory-relative open exists (`SYS_OPEN` = 12 is the only open) |
| Model load | `sys_model_load_impl`, `syscall.rs:2519-2538,2563-2609` (at `beb392c4`) | **Ambient at `beb392c4`:** a path, opened through the root form. **Gated since K1a**: `model_load_path` needs a `RootDirCap` with `READ` (`syscall.rs:2619-2626` at `origin/main`) |
| Revoke, kill | `SYS_REVOKE` (9), `SYS_KILL` (7), `syscall.rs:984-986` | Present. Revoke walks derivations across processes (ADR-010 §Q2) |
| Session broker | DS-1b's WM, `LaunchCap` (`docs/THINK-2026-10-18-ds1-home-spine-shell.md:55-86`) | **Advisory**, and pinned by `DESKTOP launchcap advisory direct_spawn=allowed`. "The kernel `SpawnCap` rung (OT-66) flips exactly that assertion" (`:86`). The Lens's directory cap is "the cap-native shape, not yet a kernel guarantee — `sys_open` on the temporary root is still ambient" (`:171`) |
| Input focus | `kernel/src/atrium/focus.rs` | One global active-focus word. Only the compositor's `ACTIVATE` sibling moves it (`:7,195`). A client's receive root is gated by an id comparison, so input security and input routing are the same check |
| Secure-attention key (SAK) | `kernel/src/atrium/sak_logic.rs:31`, `focus.rs:584-600` | Super + Tab, Space or Escape, recognised in the kernel and delivered to the WM through a command latch, never to a client. The table is fixed: widening it is a kernel change |
| Arthur's state | `/neofs/.arthur/memory/*`, `/neofs/.arthur/grants.json` (`userland/libneo/src/grant_store.rs:9`) | Per machine. Memory is AEAD-encrypted per category, but each `.key` sits beside its `.mem` (`userland/neo-arthur/src/memory_encryption.rs:9-20`). That serves crypto-erase, not confidentiality at rest |
| Intro flag | `/neofs/atrium-introduced` (`userland/neo-init/src/desktop_session.rs:3530,3699`) | Per machine |
| Directories on NeoFS | `fs/mod.rs:758-772` | Userland creates regular files only. There's no mkdir syscall: directories come from the NeoFS seed (`tools/neofs-mkfs/src/format.rs:587`) |
| Lid and idle | — | No ACPI lid notification (`kernel/src/acpi/` has no lid or GPE path). No idle timer in the WM |

**Not this lane:**
- `userland/vault-core/` (ADR-075) is a password **vault** for the user's secrets.
- `kernel/src/net/identity.rs` (ADR-068) is NIC identity.

Neither is OS sign-in. This design reuses vault-core's Argon2id and its parameter floor, nothing else.

## 2. What the MVP login protects, and what it doesn't

Stated first, so no rung claims more than it delivers.

| Threat | MVP answer |
|---|---|
| Someone at the keyboard of a locked or signed-out machine | **Protected** once rung L1 lands: no client receives input or is shown until the password is accepted |
| A client process reading the credential file or another account's files | **Not protected until K1b.** K1a made the root form and model loads need a `RootDirCap`. K1b (L1.1) gives Arthur only his scopes (`/neofs/.arthur`, `/boot` read-only), so no client holds a root cap it can **read** through. The one exception is the Lens: the WM's read-only browser holds `/` with `LOOKUP\|READDIR` only. It can list names but can't open a file for reading, and `.system/` stays refused by `system-store` (see "K1b as built") |
| A client spawning a process outside its allowlist | **Not protected until K2.** After K2 only `SpawnCap` holders spawn |
| Someone who boots the **Try** USB with an installed LogansOS disk attached | **Not protected: Try can read the installed disk's files.** Try mounts the found volume read-only (OT-87 §6.3), which protects the disk from *writes*, not its contents from being *read*. Skipping the login doesn't change that: the files aren't encrypted. After K1b, the Try session's clients are granted no cap on the found volume, so Arthur and the Lens don't read it. But Try's neo-init holds the root cap, and anyone can build or boot another image that reads the disk. Confidentiality against this needs encryption at rest (§12, OT-87 §11.5). Until then, the installer and the first-boot screen say so: "Your files on this computer are protected by your password while LogansOS is running. Anyone who starts this computer from another USB stick or system can read them." |
| Someone with the disk (removed, or booted from another OS) | **Not protected.** There's no encryption at rest. Arthur's `.key` files sit beside their `.mem` files. Full-disk encryption is deferred with OT-87 §11.5, and wrapping Arthur's keys under the account's key is the optional rung L5 |
| Online guessing at the login screen | A growing delay per failure (§4.4), but no lockout, because one local account can't be locked out of its own machine |
| Offline guessing from a copied credential record | Argon2id at a cost of at least 64 MiB × 3 passes, tuned on the slowest target (§4.2). This only matters once the record is read-protected (K1) or the disk is encrypted |

## 3. What a user is, in a capability OS

### 3.1 The account record (userland)

An **account** is a record the session broker owns. The kernel never sees it:

| Field | Meaning |
|---|---|
| `account_id` | 16 random bytes from `sys_getrandom`, fixed at creation. Never shown, never reused |
| `display_name` | UTF-8, up to 64 bytes, shown on the login screen. Not an identifier: renaming changes nothing else |
| `credential` | The `NEOCRED1` record (§4.3) |
| `home` | The account's directory on NeoFS (§6.1). The session's file authority is rooted there |

For the MVP there is exactly **one** account, created on the installed system's first boot (§7).

### 3.2 Who mints it, and who holds the session

- **The session broker is neo-init's WM.** It is already the trusted compositor and the only intended spawner (DS-1b),
  and it already holds the scanout and the `ACTIVATE` focus sibling.
- It verifies the password, so the UI and the verifier are the same trusted process. greetd splits a greeter UI from
  a verifying daemon because its greeter is an untrusted program (§11). Here the login screen is the WM's own chrome,
  so there's no untrusted greeter to split off.
- **No new process is needed** for the MVP. A separate verifier process is a later option, if the WM's trust surface
  is judged too big; its interface would be greetd's message sequence, create, answer, start (§11).
- **The record is the WM's alone (Dexter #186 item 4, 2026-10-30).** In the product the root form of `SYS_OPEN` is
  ambient (`cap-rooted-open` is off), and even with it on every client's RootDirCap spans all of `/` until K1b's
  scoped caps. So any process could read the verifier and salt (an offline attack) or overwrite the record (the next
  boot is first boot: the account is gone). The kernel's `system-store` closes it: every root-form open or model load
  at or under a mount's `.system/` is `EPERM` unless the caller holds the `SystemStore` cap, which the kernel mints
  into neo-init alone and which can't be derived, granted or inherited. The path is checked normalized (`//`, `.`,
  `..` folded) and before it resolves. `login_record_guard` checks it: a no-caps process gets `EPERM` for the record
  (read and write), a `..` path, the directory and a new file, while `/neofs` still opens for it; without the
  feature it gets all of them. When K1b lands, `.system/` stays outside every directory cap a client is given.

### 3.3 A session is the caps granted at spawn

On a successful sign-in, the broker starts the session's clients (Home, Arthur, later the apps). Each gets **only**
what the broker passes at spawn, using today's inheritance rule (`syscall.rs:1670-1700`):
- its surface and session channels (DS-1);
- its `LaunchCap` with its allowlist (DS-1b);
- **after K1:** directory caps under the account's home, e.g. Arthur gets `home/.arthur`, read and write, and the
  machine's model directory, read only;
- **after K2:** a `SpawnCap` only if the client is allowed to launch others, and only for the modules on its list.

A process holds no authority it wasn't handed, and nothing is left over that names the user.

### 3.4 How it closes the ambient-spawn gap

- **K2** gates `sys_spawn` behind a `SpawnCap` handle:
  - init holds the root `SpawnCap` and passes a derived one to the WM;
  - the WM derives per-client ones (allowlisted, as `LaunchCap` is today) and revokes them at logout;
  - a spawn without one is refused with `EPERM`.

  DS-1b's pinned assertion (`direct_spawn=allowed`) flips to `direct_spawn=refused`, which is the test the DS-1 THINK
  prepared (`ds1 THINK:59-60,86`).
- **K1** is the "capability-rooted open" `fs/mod.rs:1186-1189` names as the target. It's in three parts:
  - **K1a-0 (landed, #166, `149d4a19`)** closed two capability-revoke escapes, a race and laundering through close
    or exit, with a `CAP_GRAPH` lock and re-parenting on close and exit, and amended ADR-010. L3's logout revocation
    relies on it.
  - **K1a (landed, #168, `2922bf0c`, via the browser lane OT-61 C2.1a)** made the root form need a `RootDirCap`:
    an ordinary directory file handle on the VFS root (`kernel/src/sched/process.rs:1171-1194`), whose rights bound
    what an open may ask for (`EPERM` with no cap, `EACCES` when it's too narrow). It gates `sys_model_load` the same
    way (`syscall.rs:2619-2626`), which closes Chad's item 3. It also added `SYS_HANDLE_INFO` and `SYS_PROCESS_SELF`.
  - **K1b (open)** adds the directory-handle form, `open_at(dir_handle, relative_path, flags, rights)`, with the
    rights a subset of the directory cap's, and ports the product call sites so no client needs the root cap. The
    Lens's directory cap (`ds1 THINK:171`) then becomes a real boundary.
  - **One rule for every directory-handle operation (K1 scope, all parts):** each operation that takes a directory
    handle (`readdir`, `mount`, `umount`, `open_at`, and any later one) checks the rights **on that handle**, the
    narrowed derived cap, never the rights of the backing `OpenFile` it shares with its parent. Derivation and
    inheritance narrow the handle, not the open file, so checking the open file would let a narrowed cap act with its
    parent's rights. Janus found exactly this in K1a (relay #112, F1: `readdir`, `mount` and `umount` checked the
    backing `OpenFile`). S5's fix-forward closes it for K1a's operations, and K1b's `open_at` follows the same rule
    from the start.

**K1b as built (L1.1, 2026-10-30): scoped directory caps, not `open_at`.** J.c.'s lane order asked for per-client
RootDirCap scoping, and that needs no new syscall and no port of the call sites above:
- Every directory opened by path records that path, and a handle on it is a scope.
- A root-form open (and a model load) needs a held scope containing the normalized path, compared by whole
  components, with rights covering the request. `EPERM` if no held scope contains the path; `EACCES` if every one that
  does is too narrow.
- `OPEN_SCOPE_DIR` opens a directory as a scope whose handle carries file rights (READ, WRITE) as the bound for opens
  beneath it.
- neo-init keeps `/`, and the WM spawns Arthur with `libneo::ARTHUR_SCOPES` (`/neofs/.arthur` read-write, `/boot`
  read-only) instead of a RootDirCap for `/`.
- Discriminator `desktop_arthur_scope`, kill-switch `cap-rooted-open`. Its probe checks five opens:
  - outside (`/neofs`): `EPERM`;
  - inside (Arthur's memory directory): opens;
  - an escape by `..` (`/neofs/.arthur/..`): `EPERM`;
  - a name that only shares the prefix (`/neofs/.arthurx`): `EPERM`, before anything resolves;
  - inside but asking for a right the scope lacks (`EXECUTE`): `EACCES`.
- `open_at` (relative paths through a directory handle) can still come later. The rights rule above already holds
  for it.
- **The Lens keeps a browse-only `/` (Dexter #247 F1).** The Lens is the WM's read-only file browser, and it shows
  the whole namespace by design ("Lens · read-only"). Its cap is `/` with `LOOKUP|READDIR` only:
  - it can list names, but can't open a file for reading (`READ` isn't in its rights);
  - `.system/` is refused at open by `system-store`, whatever the cap.

  Narrowing it would change what the Lens is for. If a later decision narrows it (for example to the account's home,
  L3), the WM changes the one `PATH` it opens.
- **Per-client revocation (Dexter #247 F2).** `sys_spawn_scoped` closes the WM's copies of a client's scopes after the
  spawn. The client's copies re-parent to the WM's `/` cap (CP-88), so they are still revoked with it (logout-wide).
  But the WM keeps no per-client handle to revoke just one client's scopes. L3's per-session revocation keeps those
  copies with the session, closing them at sign-out, rather than adding a revoke-by-cap-id path.
- **A scope is a path, not a directory object.** The check compares the normalized path against the scope's path.
  So a mount installed at or above a scope re-points what it covers. Only the mount authority can do that (#240), so
  this is a property of the trusted broker, not a client escape.

**Prior art for K1b's path scoping** (checked 2026-10-31; read for principle, no code borrowed):

| Project | How it scopes opens | License | Last commit | What we took |
|---|---|---|---|---|
| FreeBSD Capsicum (`cap_rights_limit`, `openat` beneath a directory fd in capability mode; `vfs_lookup.c` `NI_LCF_STRICTREL`/`NI_LCF_CAP_DOTDOT`) | A descriptor's rights bound what's opened through it; in capability mode only relative lookups beneath a directory fd are allowed, `..` tracked against the start directory | BSD-2-Clause | `freebsd/freebsd-src` `6d705f2b`, 2026-10-30 | The rights-subset rule and the "beneath" rule; not the vnode binding |
| Linux `openat2` `RESOLVE_BENEATH` / `RESOLVE_NO_SYMLINKS` (`fs/namei.c` `LOOKUP_BENEATH`) | Resolution refuses to leave the starting directory, checked during the walk | GPL-2.0-only | `torvalds/linux` `36aad21d`, 2026-10-30 | The principle only (GPL) |
| Fuchsia directory handles (`fuchsia.io`) | A process gets directory channels, and paths resolve relative to them; there's no ambient root | BSD-3-Clause (not re-verified: `fuchsia.googlesource.com` refused this container's proxy) | not verified | The model: a client's namespace is the directories it was handed |
| WASI preopens (Bytecode Alliance `wasmtime`; `cap-std` is its Rust directory-handle sandbox) | The host hands the guest pre-opened directories; every path resolves beneath one of them, with `..` refused at the top | Apache-2.0 WITH LLVM-exception (`wasmtime`); Apache-2.0 / MIT (`cap-std`) | `wasmtime` `c57fe564`, 2026-10-30; `cap-std` `b2112115`, 2026-10-19 | The preopen shape (`ARTHUR_SCOPES` is a preopen list); `cap-std` is the closest Rust design, but it's std-based and not usable in our `no_std` kernel |

The difference from all four: they bind a scope to a directory object (a vnode, a channel, a descriptor) and resolve
beneath it. K1b binds it to a normalized path and resolves from the root. That is sound here because resolution
follows no symlink part-way through and `..` is folded before resolution. The cost is the mount caveat above.

**Migration size (K1b):** userland product call sites of the root open are bounded: `neo-arthur` (11),
`neo-shell` (7), `desktop_session.rs` (7), `arthur_compositor.rs` (1) and `libneo/action.rs` (1). The other 41 in
`neo-init/src/main.rs` all sit in feature-gated test modules, which can keep the root form through a test-only grant from init.

### 3.5 Logout and revocation

Logout, in order:
1. The WM sends each session client a close request on its session channel. Arthur flushes and `fsync`s his
   memory: his save path is already write-ahead (`memory_encryption.rs:13-20`).
2. It waits a bounded number of WM ticks, then `sys_kill`s any client still running.
3. It `sys_revoke`s every cap it derived for the session (directory caps, `LaunchCap`, `SpawnCap`). ADR-010's walk
   invalidates the clients' copies even where they were passed on.
4. It shows the login screen.

The clean shutdown and restart lane (OT-90) reuses steps 1–3 before it flushes NeoFS and powers off.

## 4. Credentials

### 4.1 Algorithm

- **Argon2id, version 0x13 (RFC 9106),** through the RustCrypto `argon2` crate. The workspace already locks
  `argon2 0.5.3` (`Cargo.lock:134`), and vault-core uses it with an RFC 9106 known-answer test
  (`userland/vault-core/src/lib.rs:184-210`).
- **No new crate.** Login reuses vault-core's KDF entry point, or a sibling with the same parameter type, so there's
  one Argon2 configuration in the tree.

### 4.2 Parameters, and where the cost is tuned

- **Floor:** `KdfParams::INTERACTIVE_V1`, which is 64 MiB, 3 passes, 1 lane (`vault-core/src/policy.rs:17-21`).
  A stored record below the floor is rejected, not honoured (vault-core's F5 rule, for the same downgrade reason).
- **Tuning:** rung L1 adds a boot-time measurement line to neo-init,
  `LOGIN kdf params=m:65536,t:3,p:1 elapsed_ms=…`. It's run once on the Zenbook (image A) and once on the OptiPlex.
  The shipped parameters are the largest whose median on the **slower** of the two stays under about 1 s, and never
  below the floor. The chosen values and both measurements go in the L1 RETURN, and the record stores its own
  parameters, so they can be raised later without invalidating old records.
- **Memory budget:** 64 MiB is transient, during sign-in only, inside neo-init. It's freed before the session starts,
  so it doesn't compete with Arthur's model tier.

### 4.3 The record: `NEOCRED1` (a byte layout, so it ships with a byte oracle)

The record is **two files**, `/neofs/.system/account.cred.a` and `/neofs/.system/account.cred.b`. Each holds one
160-byte slot, little-endian:

| Offset | Size | Field |
|---|---|---|
| 0 | 8 | magic `NEOCRED1` |
| 8 | 2 | version = 1 |
| 10 | 2 | flags (bit 0: a recovery verifier is present) |
| 12 | 4 | Argon2 memory, KiB |
| 16 | 4 | Argon2 passes |
| 20 | 4 | Argon2 lanes |
| 24 | 16 | `account_id` |
| 40 | 16 | password salt |
| 56 | 32 | password verifier: Argon2id(password, salt), 32 bytes |
| 88 | 16 | recovery salt |
| 104 | 24 | recovery verifier, truncated: the first 24 bytes of Argon2id(recovery code, recovery salt) |
| 128 | 8 | generation (u64, starts at 1) |
| 136 | 16 | checksum: the first 16 bytes of BLAKE3 over bytes 0..136 |
| 152 | 8 | reserved, zero |

`display_name` and the home path live in a separate plain file (`/neofs/.system/account.meta`), so the
credential files stay fixed-size.

**The oracle (CLAUDE.md, "Format/ABI specs"):** L1 ships a hand-laid 160-byte example of each file with known password, salt and
parameters. The verifier bytes are computed by the P-H-C reference `argon2` CLI at a pinned commit (vault-core's
precedent, `lib.rs:208-209`) and pasted as literal bytes. The parser and writer are tested against that example, not
only against each other.

**Writes, and the guarantee under a cut (Chad, item 4).** There's no rename syscall and no seek (a handle's cursor
starts at 0, `fs/mod.rs:155-160`), so a record can't be swapped in one step. Two files make the guarantee independent
of how NeoFS or the device behaves when a write is cut:
- **A change writes only one file:** the one holding the older (lower-generation) slot. It opens that file, writes
  the whole 160-byte slot with generation + 1, and `fsync`s. The newer file isn't opened for writing, so none of
  its data or inode blocks is rewritten.
- **A reader** reads both files and takes the slot with the highest generation whose magic, version and checksum are
  valid and whose parameters meet the floor (§4.2).
- **The guarantee:** a cut at any point of a change (a torn 4 KiB block, a lost flush, a crash before NeoFS publishes
  its new root) can damage only the file being written. The other file holds the previous record, untouched, so at
  least one valid slot always remains, and it's the old password or the new one, never neither. This rests on one
  NeoFS property, stated so L1 tests it rather than assuming it: **a write to one file never rewrites a block
  belonging to another file.** Re-checked for L1 at `origin/main` `233892eb` (after #180, the commit barrier and
  deferred reuse, and #187, the allocator moved to `alloc_logic.rs`):
  - **The write path is copy-on-write.** `commit_regular_write` builds the new content, writes it to newly reserved
    data blocks and a new inode block, then commits the new child entry (`kernel/src/fs/neofs/inode.rs:555-610`).
    Every block comes from the allocator (`reserve_blocks`, `reserve_inode_block_and_generation`, `inode.rs:1068-1080`;
    `alloc_logic::allocate_one_block` and `reserve_block_and_generation`, `alloc_logic.rs:115-140`), which hands out
    only blocks the free map marks free.
  - **No block another file holds is ever free.** A plain write frees nothing (the old version's blocks are orphans
    until GC). Where a commit does free blocks (`release_old_regular_tree_blocks`, `inode.rs:1205-1230`), #180's rule
    2 keeps them out of allocation until the freeing generation is durable on every superblock copy
    (`allocate_from_free_map` skips deferred blocks, `alloc_logic.rs:66-100`; `commit_logic.rs:16-21`). #187's
    `release_run` (`alloc_logic.rs:142-167`, through `inode.rs:2297`) gives back only a run a failed large write has just allocated and nothing
    names yet (`inode.rs:308-331`, `:360-373`).
  - **The directories and the free map are copy-on-write too.** `commit_child_update` rewrites the parent chain into
    new blocks (`inode.rs:742-799`). `publish_superblock` writes the free map to a newly allocated range and names it,
    with its hash, in the superblock (`inode.rs:1300-1340`).
  - **The commit order.** `fsync` drains the whole mount (`inode.rs:671-683`) in role order, data, inode, root
    directory, free map, then the superblocks (`commit_logic.rs:24-44`). #180's barrier flushes before any superblock
    copy is written (`cache.rs:612-707`, rule 1, `commit_logic.rs:12-16`).
  - **The shared metadata a change does rewrite in place** is only the three superblock copies, which have their
    own torn-write recovery: generation selection over three copies. The new `.system` directory still names the
    other file's unchanged inode. Until the new root is published, the old directory is the live one and both files
    read as before the change.
- **Both files are created once,** at account creation (§7). A later change replaces one file's inode and the
  directory entry that names it; it never rewrites the other file's inode or data.
- **L1's close gate tests the cut:** a scenario injects a crash after each write and flush of a password change (the
  `phase3_neofs_crash_mid_overwrite_recovery` hook, extended to every step, not only before superblock
  publication), then boots and checks that exactly one of the old or new password is accepted. It also checks that
  the untouched file's bytes are unchanged (BLAKE3 before and after).

The oracle covers a valid pair, a pair whose newer file is torn, and a below-floor slot.

**Where `.system` comes from:** userland can't make directories (§1), so `.system/` is seeded the way `.arthur` is:
by `neofs-mkfs` for the VM image (`xtask vm-image`) and by the installer's NeoFS seed (OT-87 R4).

### 4.4 Wrong password, lockout, forgotten password

- **Wrong password:** the login screen says "That password isn't right." After the 3rd consecutive failure, each
  further try waits 2ⁿ⁻³ s (capped at 60 s) before it's even checked.
- **Where the counter lives (Chad, item 6):** in neo-init's memory, which survives everything but a reboot. The login
  screen and the verifier are part of neo-init (§3.2), and neo-init isn't restartable: when it exits, the kernel
  reaps it and drops into kernel-only mode with no respawn (`kernel/src/main.rs:463-480`). So there's no
  "restart the login screen" short of rebooting the machine.
  - **If a later rung moves the login screen or the verifier into its own process** (§10, Q1), the counter stays in
    neo-init, the supervisor that spawns it, and is passed in, never kept by the restartable part.
  - **It survives a reboot (Dexter #186 item 2; J.c., 2026-10-30), superseding "it resets at boot".** A per-boot
    counter gave three free guesses per boot. The count is a `NEOFAIL1` record (login-core `attempts`, a hand-laid
    byte oracle), kept as two copies (`.system/attempts.a` and `.b`) written alternately like the credential's, so
    a cut leaves the older copy whole. It counts for one account id (a record left by an account first boot
    replaced counts as zero). Two present copies that are both damaged read as the longest delay (fail closed).
    - **Counted before the check.** Each attempt writes the count plus one and flushes it before the password (or
      recovery code) is checked, so cutting the power mid-attempt never buys a free guess. A write that fails
      refuses the attempt.
    - **Reset only by a successful sign-in, unlock or recovery.** The lock screen counts with the same record.
    - **The cost:** someone at the machine who types wrong passwords makes its owner wait too, at most 60 s per
      try; still no permanent lockout.
    - **Protected like the credential:** both files are under `.system/`, which only the WM can open (§3.2,
      kernel `system-store`).
- **No permanent lockout:** with one local account, a lockout bricks the machine for its owner.
- **Forgotten password:**
  - At account creation the first-boot screen shows a **recovery code** once: 20 characters from a 32-symbol
    alphabet, about 100 bits, drawn from `sys_getrandom`. The user is told to write it down.
  - "Forgot your password?" on the login screen takes the code, and then a new password.
  - Only the recovery verifier is stored, Argon2id at the same parameters.
  - Using the code issues a new one.
  - With no recovery code, the honest answer for the MVP is reinstall (OT-87 R4b), because there's no encryption
    to bypass.

### 4.5 Multi-user, later (not built)

What it would need, so the MVP's shapes don't block it:
- **Records:** one record per account under `.system/accounts/`, and a seeded `home/<account_id>/` per account.
  The single-file MVP layout becomes the first entry.
- **The login screen** gets a user picker.
- **K1's directory caps** already isolate homes, because a session never holds another account's home.
- **Account administration** (create, delete, rename) needs an admin capability. Who holds it is a policy decision
  for that lane.
- **Fast user switching** keeps two sessions alive and swaps the composited set. It's out of scope until there's
  memory for two Arthur sessions.

## 5. The screens

All three are WM chrome on the Atrium surface protocol (ADR-073). None is a client, so the `OP_*` protocol gains no
login opcode. The WM never sends a password to a client.

### 5.1 Login

- **Content:** the wordmark, the account's display name, one password field, and "Forgot your password?".
- **Replaces** the boot status line (`boot_frame.rs:109-117`) on an installed system.
- **Input:** while the login screen is up, the WM's own focus object is the active route. No client has been started,
  so nothing else can read keys, even before K1/K2.
- **After success:** the 64 MiB KDF buffer is freed, and the session is started (§3.3).

### 5.2 Lock

- **When it locks:**
  - **an idle timer** in the WM: no key or pointer event for N minutes, default 10, a setting for the settings
    app (MVP);
  - **Super+L,** which needs `KEY_L` in `SAK_KEYS` (`sak_logic.rs:31`). That's a one-entry kernel change through the
    AAG, so no client can intercept the chord (rung L2);
  - **lid close: later.** The kernel has no ACPI lid notification yet. It belongs with OT-90's ACPI work.
  - **As built (L2):** the idle timer needs input the WM can't see: keys go to the active client's route.
    Counting client commits instead would let a client keep the screen unlocked by committing forever. So
    the kernel's `sak-lock` also ORs an activity bit (`0x10`) into the SAK latch for every produced key or
    pointer event (`focus::route_event`, `stage_event`). The WM already takes the latch every loop
    iteration, and only the input-system holder can take it. No new syscall. The lock needs the latch
    take (`atrium-pointer-forward`, in the product); without it the WM doesn't arm the lock at all.
- **Who owns input while locked:**
  - The WM activates its own lock-screen focus object (`focus.rs:195`, the `ACTIVATE` swap).
  - Every client's receive root then fails the kernel's id comparison, so **no client can read a keystroke**.
  - **This depends on only the WM being able to activate a route** (CP-96). Until the L2 pre-merge AAG's F1
    fix, any client could create a focus, mint itself an `ACTIVATE` sibling and take the route. Activation
    now also requires the non-delegable `InputSystem` authority, which only the session root holds.
  - The pointer route is the WM's already.
  - SAK chords go to the WM through the latch, as always.
  - **As built (L2b, kernel `sak-lock-route`):** the WM's swap alone left a hole, found by the L2 AAG: a WM
    parked in a synchronous send (a reply to a client that never receives it, CP-57) runs no code, so the
    route stayed the client's after Super+L. The lock is now a kernel route state (`focus::LOCK_ROUTE`,
    CP-97). The WM registers its own focus as the lock focus when it arms. The SAK filter sets the locked
    bit as it latches Super+L, before any WM code runs, and while it is set every route read is the lock
    focus and a pointer record can be forwarded only to it. The WM locks on idle and unlocks through
    `SYS_INPUT_SAK_TAKE`'s lock ops (input-system authority only); unlock makes the restored owner the
    route as it clears the bit. Without the kernel half the WM falls back to L2's swap. The bit takes no
    lock, so a receive or forward can straddle it (Jarvis #160): a receive re-checks after its pop, and
    every end of a lock empties what client queues held across it (the lock epoch), so nothing typed or
    forwarded before the chord reaches a client after it (CP-97, `lock_route_races`).
- **An exclusive compositor state (Chad, item 5).** Locked (and signed out) is a WM state, `Locked`, in which the
  WM composes **only** its own lock or login surface:
  - no client surface, decoration, shadow or glass sample of one;
  - no WM-drawn element that belongs to a client, in particular **Arthur's busy emblem** (`OP_BUSY`, drawn over its
    client at `userland/neo-init/src/desktop_session.rs:900-907`). An `OP_BUSY` received while locked is recorded
    and shown only after unlock;
  - a client spawned, or a surface created, while locked gets no focus, no input route and no pixels until unlock.
    The WM keeps it off the input route by never activating its focus object while `Locked`;
  - client presents are accepted (so clients don't stall) but not composited.

  Clients stay alive meanwhile: Arthur keeps running and nothing is lost. L2's close gate checks the state directly
  (a frame digest before and after):
  - a running client that commits new pixels and sends `OP_BUSY` while locked changes no byte of the composited
    frame;
  - a client **spawned while locked** that creates its surface, commits a unique visible pattern and sends
    `OP_BUSY` changes no byte of it either, and receives no input. Its surface appears only after unlock.
- **Unlock:** the same verifier as sign-in. On success the WM re-activates the previously focused client through
  DS-1's handoff (`libneo::atrium_handoff::HANDOFF`, `ds1 THINK:93`), so no stale key reaches the wrong owner.
- **A client can't block locking:** the idle timer and the SAK chord are the WM's, and the swap is a kernel atomic.
  A hung or hostile client can't hold focus against it. That's the property DS-1 left to DI-1's SAK, now used here.

### 5.3 Logout

- **Where:** a "Sign out" item in the shelf or Spine. Arthur can open it, but can't confirm it.
- **Confirmation:** "Sign out? Arthur will save what he's doing."
- **Steps:** as §3.5, then the login screen.

## 6. Arthur and the user

### 6.1 Per user, per machine

| Data | Per | Location after L3 |
|---|---|---|
| Arthur's memory (`memory/*.mem`, `*.key`) | user | `/neofs/home/<account_id>/.arthur/memory/` |
| Grants (`grants.json`), stakes, undo | user | `/neofs/home/<account_id>/.arthur/` |
| Intro flag | user | `/neofs/home/<account_id>/atrium-introduced` |
| The model and vocab | machine | Where OT-87 R2 puts them (NeoFS, read only to sessions) or the boot module |
| Settings | split | Machine (display, network) under `.system/`, user (idle time, theme) under the home |

**Migration (single account):**
- On the first sign-in after L3, an existing `/neofs/.arthur` belongs to that account.
- The broker passes the session a directory cap on it **in place**, without copying it.
- The home path in `account.meta` records where it is.
- A copy would double the writes and add a crash window for nothing.

### 6.2 Before sign-in

- Arthur **isn't spawned.**
- The model preload is a kernel boot-time step (`boot-model-preload`), so the model is resident and costs no
  sign-in time. But it isn't reachable: no process holds a cap to it until the broker grants one.
- The login screen has no assistant.
- After K1, Arthur's spawn carries only the signed-in account's home caps. Before K1, Arthur still opens its fixed
  paths through the root form. The account's home is then a convention, and L3 says so.

## 7. The installer and Try (OT-87)

- **Account creation is a first-boot screen on the installed system, not an installer step.** Three reasons:
  - The installer runs from the Try medium, whose own NeoFS is read-only (OT-87 §6.3). Writing a credential into the
    target from there would add a second writer for `.system/` and the installer's write path is already the most
    reviewed code in OT-87 (R4, Muse).
  - The first-boot screen runs on the system that will verify the password, with the KDF parameters measured there.
  - OT-87 §4.8 already leaves "a machine-name or user-name step" as later product work. This is that step.
- **The first-boot screen:**
  - **When:** shown when neither `.system/account.cred.a` nor `.b` holds a valid record, on a writable NeoFS.
  - **Account:** asks for a name and a password, twice.
  - **Recovery:** shows the recovery code and asks the user to confirm they've written it down.
  - **Then:** writes the record and goes to the desktop. The intro runs after it, as OT-87 §4.8 describes.
- **Try:**
  - **No account, no login screen,** and today's straight-to-desktop session.
  - **Nothing persists.** NeoFS is absent, or read-only when an installed disk is attached (OT-87 §6.3). A lock screen
    in Try would protect nothing that survives a reboot.
  - **An installed account on an attached disk** isn't used by Try. Try shows "LogansOS is installed on this machine"
    (OT-87 R5) and doesn't offer to sign in to it.
  - **Try can read that disk's files.** Read-only protects it from writes, not from reading (§2's Try row). After
    K1b, Try grants its clients no cap on the found volume, so Arthur and the Lens don't open it. Confidentiality
    against anyone with the machine needs encryption at rest, which is deferred.
- **The VM image (OT-87 R1):** `vm-image` seeds `.system/` empty, so `vm-run` shows the first-boot screen once L1
  lands. `--fresh-data` starts over.

## 8. The rung ladder

Dependencies, cycles and gates only, with no calendar estimates.

**Order:** L0 → L1 → L2. K1 is K1a-0 (landed) → K1a (landed) → K1b. K1b and K2 are independent of the L rungs and of
each other, and can run in parallel with L1. L3 depends on L1 and on K1b. L4 depends on L1. L5 is optional and depends
on L3.

| Rung | What | Depends on | Cost | Close gate | Kill-switch | Kernel cap model? | Muse? |
|---|---|---|---|---|---|---|---|
| **L0** | This THINK and a cross-family read | — | 1 doc, 1 review | PM merge after the review | — | — | — |
| **L1** | Credentials and the login screen:<br>- `NEOCRED1` reader and writer, with the byte oracle (§4.3)<br>- the KDF on vault-core's floor, and the `LOGIN kdf … elapsed_ms` line<br>- the first-boot account screen (§7)<br>- the login screen<br>- the recovery code<br>- the backoff<br>- `.system/` seeded by `neofs-mkfs` (and later by the OT-87 seed)<br>Userland only | — | Moderate. Userland and xtask; one byte oracle; a few scenario cycles | 4-shard gate. Scenarios, on the R1 VM layout (two boots):<br>- boot 1 creates the account; boot 2 shows the login screen, a wrong password is refused, the right one starts the session<br>- the recovery code resets the password<br>- a below-floor record is refused<br>- with login off, boot goes straight to the desktop, bit-identical to today<br>the cut scenario (§4.3): a crash after each write and flush of a password change leaves exactly the old or the new password.<br>KATs: the `NEOCRED1` oracle (a valid pair, a torn newer file, a below-floor slot), the backoff schedule, RFC 9106 through the shared KDF.<br>Timing lines from the Zenbook and the OptiPlex in the RETURN | `login` (off ⇒ today's boot to the desktop, the same scenario output) | No | **Yes**: credential handling and the recovery path, Claude-authored |
| **L2** | Lock and unlock:<br>- the idle timer<br>- Super+L (`KEY_L` in `SAK_KEYS`)<br>- the WM's lock focus and compositing<br>- unlock through the DS-1 handoff | L1 | Moderate. A one-entry kernel SAK table change plus WM work | 4-shard gate. Scenarios:<br>- a scripted key while locked reaches no client (the client's receive count is unchanged)<br>- the exclusive state (§5.2): a client's commit and `OP_BUSY` while locked leave the composited frame's digest unchanged, and a client spawned while locked receives no input<br>- a client **spawned while locked that draws**: enter `Locked`, spawn a client, have it create its surface, commit a unique visible pattern and send `OP_BUSY`; the composited frame's digest is unchanged and the client receives no input. After unlock its surface and pattern appear<br>- Super+L locks even while a client holds focus and never yields<br>- unlock returns focus with no stale key<br>- the idle lock fires at the configured time<br>The SAK table KAT updated | `login` (no lock without login) | **Yes, small**: the SAK table is kernel policy. Pitfall-bank pass on the latch path (CP classes for `focus.rs`), cross-family AAG | Yes, with L1's input path |
| **K1a-0** | Close two capability-revoke escapes: a `CAP_GRAPH` lock, re-parenting on close and exit, the ADR-010 amendment | — | — | **Landed: #166, `149d4a19`** | — | Yes (done) | — |
| **K1a** | The root form and `sys_model_load` need a `RootDirCap` (an ordinary directory handle on the VFS root, rights-bounded); `SYS_HANDLE_INFO`, `SYS_PROCESS_SELF` | K1a-0 | — | **Landed: #168, `2922bf0c`**, built by S4 for the browser lane (OT-61 C2.1a) to this THINK's names, with Muse relay #111 | `cap-rooted-open` | Yes (done) | Done (#111) |
| **K1b** | The directory-handle open, and the port:<br>- `open_at(dir_handle, path, flags, rights)`<br>- the product call sites ported (§3.4), so no client needs the root cap<br>- test drivers keep the root form through the §10.1 test-only grant | K1a | High. A new syscall, rights derivation from a directory cap, every product open ported, `..` and symlink-free path rules | 4-shard gate. Scenarios:<br>- `open_at` can't escape its directory (`..`, absolute paths)<br>- rights never widen<br>- revoke kills a derived directory cap, and files opened through it (K1a-0's re-parenting)<br>- every directory-handle operation checks the handle's own rights: a handle narrowed to `LOOKUP` can't `readdir`, one without `MOUNT` can't `mount` or `umount`, and `open_at` through it can't ask for more than it holds, even though its backing `OpenFile` (shared with a wider parent) allows more<br>- Arthur, the shell and the desktop work from their granted caps, holding no root cap<br>DS-1's Lens test flips from "advisory" to "enforced".<br>The §10.1 KATs: the test-only root grant can't build into product or image A | `cap-rooted-open` (off ⇒ the root form open to all, as at `beb392c4`) | **Yes.** Pitfall-bank pass (handle-table locking, cross-process revoke), cross-family AAG (Codex lens on the derivation and revoke seams) | **Yes** |
| **K2** | `SpawnCap`:<br>- `sys_spawn` requires a `SpawnCap` handle with the module on its list<br>- init holds the root; the WM derives per-client caps and revokes them at logout | — | Moderate to high. A new handle variant, derivation with a narrowed allowlist, every spawner ported | 4-shard gate. DS-1b's `direct_spawn=allowed` flips to `refused`. Scenarios:<br>- a client spawns only allowlisted modules<br>- a revoked `SpawnCap` refuses<br>- the WM's own launches still work | `spawn-cap` (off ⇒ ambient spawn, the DS-1b pin unchanged) | **Yes.** Pitfall-bank pass (spawn/teardown seam, the C1.1 class), cross-family AAG | **Yes** |
| **L3** | The account's home:<br>- `home/<account_id>/` seeded<br>- Arthur's state moved there by cap (§6.1)<br>- the broker grants per-session directory caps and revokes them at logout<br>- logout (§3.5, §5.3) | L1, K1b | Moderate | 4-shard gate. Scenarios:<br>- sign out kills and revokes: a client's cap is `EBADF` after logout<br>- sign in again and Arthur recalls what he remembered before<br>- the pre-L3 `/neofs/.arthur` is adopted in place on the first sign-in | `login` | No (it uses K1's syscalls) | Yes (logout's revocation is security-relevant) |
| **L4** | Settings hooks: the idle time and "change password", for the settings app (an MVP item of its own) | L1 | Low | 4-shard gate; change password rewrites the record and the old password is refused | `login` | No | Yes (change password) |
| **L5** | *Optional, later:* Arthur's memory keys wrapped under a key derived from the password, so a copied disk doesn't expose memory | L3 | Moderate. Key wrapping, and a recovery story that the recovery code must also unwrap | Its own design note first | `memory-key-wrap` | No | Yes |

**What the MVP needs:** L1, L2, K1b, K2 and L3 (K1a-0 and K1a are in). L4 rides with the settings app. L5 isn't MVP.

## 9. Security review routing

Per CLAUDE.md's cross-family routing ("Security reviews of Claude-authored code go to Muse"):
- **L1:** Muse. Credential storage, the KDF floor, the recovery path, and the backoff.
- **L2:** Muse (the input-isolation claim), plus GPT-6-Luna, high, for the WM code.
- **K1b and K2:** Muse **and** a cross-family AAG on the kernel seams. GPT-6.1-Sol, high: both rungs change
  handle-table and revoke paths, the SMP-adjacent core. Concurrency pitfall-bank pass in the dispatch.
- **L3:** Muse, for logout's revoke-then-show ordering.
- **This THINK:** a cross-family design read. GPT-6-Luna, high: design only, no SMP code.

## 10. Decisions and open questions

| # | Question | Answer |
|---|---|---|
| 1 | The verifier in the WM, or a separate process? | **Decided (PM, 2026-10-27): in the WM for the MVP** (§3.2). A separate verifier stays a later option |
| 2 | A recovery code shown once at account creation, or no recovery (reinstall as the only path)? | **Decided (J.c., 2026-10-27): the recovery code**, as §4.4 describes. L1 builds it |
| 3 | K1: port only product call sites, with a test-only root grant for the test drivers? | **Decided (PM, 2026-10-27): product call sites only.** The test-only grant must be impossible to build into a product or image-A kernel; §10.1 says how |
| 4 | The idle-lock default: 10 minutes, or off until the settings app exists? | **Decided (J.c., 2026-10-27): 10 minutes**, on by default. L2 builds it, and L4 makes it a setting |

### 10.1 Keeping K1's test-only root grant out of shipped kernels

Init hands the scenario drivers a `RootDirCap` only under a test feature, `neo-test-root-open-grant`. It's kept out of
every shipped kernel the way `neo-test-block-write-guard-control` already is (`kernel/src/drivers/block/mod.rs:505-516`,
`tools/xtask/src/main.rs:5204-5212`), in three layers:
1. **The feature implies `neo-test`,** and the grant code is compiled only under it.
2. **A `compile_error!`** in the kernel fires if `neo-test-root-open-grant` is combined with `product`, `desktop`,
   `fb-progress-console` or `neo-test-bare-metal-mode`. Image A's feature list includes `desktop`, which pulls in
   `product`, so image A can't be built with it.
3. **xtask refuses** the feature in every spelling Cargo accepts (bare, package-qualified, with whitespace), the
   `feature_disables_write_guard` pattern. That covers `iso`, `iso --with-model`, `image-kernel-build` and
   `vm-image`. `product-elf-check` gains the grant's log marker in its `absent` list, so a product ELF that
   contains it fails CI.

K1's close gate adds KATs for layers 2 and 3: a build with the forbidden pair fails, and xtask refuses each spelling.

## 11. Prior-art survey (CLAUDE.md, "Unfamiliar features", 2026-10-27)

Last commits were read with `git` from each repository on 2026-10-27 (shallow clones; the SHA is the default branch's
head).

| Candidate | What it does, and how | License | Last commit | Alive? | Fit, and what we take |
|---|---|---|---|---|---|
| Fuchsia account and session managers (`fuchsia.googlesource.com`) | Accounts and authentication live in userland components. A session gets only the capabilities the component framework routes to it; no ambient user identity | BSD-3-Clause (per the dispatch; **not verified here**) | **Not verified:** `fuchsia.googlesource.com` is blocked from this session (HTTP 403 at the proxy), and no GitHub mirror resolved | Active project (by reputation; not checked) | **Design:** the user as routed capabilities, not a kernel uid (§3). No code |
| [genodelabs/genode](https://github.com/genodelabs/genode) | Every service is reached through a *session* requested from the parent, which routes and can close it. No user concept in the kernel | AGPL-3.0 with a linking exception (repo `LICENSE`) | `0f275e7a`, 2026-05-20 | Yes | **Principle only** (AGPL): the session as a parent-owned grant set the parent can tear down (§3.3, §3.5). No code |
| [seL4/camkes-tool](https://github.com/seL4/camkes-tool) | Static capability distribution at build time; no runtime users or sessions | BSD-2-Clause (also GPL-2.0-only and CC-BY-SA files, `LICENSES/`) | `e6fa51fd`, 2026-07-22 | Yes | **Nothing.** A static system has no login. It confirms only that the kernel needn't know users |
| [systemd/systemd](https://github.com/systemd/systemd) (`src/login`, logind) | Seats, sessions and session lifecycle; locking by signalling the session; idle hints; inhibitors | LGPL-2.1 for logind (repo `LICENSE.LGPL2.1`; GPL-2.0 for parts) | `a09e5f55`, 2026-10-26 | Yes | **Principle only:** the session lifecycle (create, lock, unlock, terminate) and the idle hint (§5). No code |
| [linux-pam/linux-pam](https://github.com/linux-pam/linux-pam) | Pluggable authentication modules, chained per service | BSD-3-style, or GPL at the user's choice (`COPYING`) | `584ef227`, 2026-10-12 | Yes | **Nothing for the MVP.** One local password needs no module stack. A later multi-factor lane may borrow the *conversation* idea, which greetd also uses |
| [kennylevinsen/greetd](https://github.com/kennylevinsen/greetd) | A login daemon. An unprivileged greeter talks to it over an IPC protocol: `CreateSession`, `PostAuthMessageResponse`, `StartSession`, `CancelSession` | GPL-3.0 (repo `LICENSE`; `greetd_ipc` is `GPL-3.0-only`) | `d5f27c3a`, 2026-10-25 | Yes | **Principle only:** keep the UI from the verifier when the UI is untrusted, and a message sequence for a later split (§3.2, §10 Q1). No code |
| [redox-os/userutils](https://github.com/redox-os/userutils) (with `redox_users`) | Unix-style `login`, `passwd`, `useradd` over `/etc/passwd` and a shadow file, with uid/gid | MIT (repo `LICENSE`) | `73aea8b3`, 2026-09-16 | Yes | **Little.** It's the uid/gid model we're not adopting. It confirms a blank-password and a re-prompt flow (`src/bin/login.rs:147-187`). No code |
| [redox-os/redox](https://github.com/redox-os/redox) | The distribution that ships `userutils` | MIT (repo `LICENSE`) | `b3f371c1`, 2026-09-26 | Yes | Context for the row above only |
| [RustCrypto/password-hashes](https://github.com/RustCrypto/password-hashes) (`argon2`) | Pure-Rust Argon2 (RFC 9106), `no_std` with `alloc` | MIT OR Apache-2.0 (`argon2/Cargo.toml`) | `2d15c850`, 2026-10-18 (`argon2` 0.6.0 upstream) | Yes | **Code, already in the tree:** the workspace locks `argon2 0.5.3` for vault-core (`Cargo.lock:134`). Login reuses it (§4.1). Moving to 0.6 is a separate dependency change |

**Outcome.** Nothing is vendored. The design takes:
- the *routed-capabilities* user from Fuchsia and Genode;
- the session lifecycle from logind (in principle);
- the greeter/verifier split as a later option, from greetd (in principle);
- the Argon2id code we already depend on.

The `NEOCRED1` record, the screens and the K1/K2 kernel rungs are ours, because no candidate fits a capability kernel
with a trusted compositor.

## 12. What this THINK doesn't do

- **No encryption at rest.** Deferred with OT-87 §11.5. L5 is the optional step toward it for Arthur's memory.
- **No multi-user.** §4.5 lists what it would need.
- **No network accounts, no biometrics, no hardware keys.**
- **No lid lock.** That needs ACPI notifications, with OT-90.
- **No settings app.** L4 only exposes the hooks it needs.
