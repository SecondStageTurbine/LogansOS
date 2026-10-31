<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #247 OT-66 K1b (scoped directory caps): security review (Section 211)

**Seat:** Dexter · **Reviewed at:** `02b1a35a` (the dispatch's head) · **Base:** `origin/main` = `ae3df788` (review
run at `6551e179`; the PR merge-base is `e2483b29`, so the reviewed diff is unchanged)
**Branch:** `dexter/k1b-scoped-root-review` · **Answers:** `Plans/DISPATCH-2026-10-31-b38-b39-reviews.md` §211
**RAG:** not used for this review; every citation is a direct read of the fetched `pr247` tree
(`git fetch origin +pull/247/head:pr247`). I ran no gate; this is a static review and I claim no CI/QEMU result.
**Diff:** `git diff origin/main...02b1a35a` = 14 files, +288/−35.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | A scoped cap cannot reach outside its subtree (`..`, symlinks, a mount inside it, rename/link, an ungated path syscall) | **PASS** by inspection — the path shape is right; no test for `..`/symlink/mount (note 1) |
| 2 | A scoped cap cannot be widened by derive, grant, spawn or IPC | **PASS** — all four attenuate; a scoped-only process cannot mint `/` (note 2) |
| 3 | Arthur gets only his own subtree in image A, and what he held before still works | **FAIL for the lane's "every client" clause; Arthur himself PASS** — the Lens holds a `/` `LOOKUP\|READDIR` scope (**F1**) |
| 4 | Off mode is today's behaviour bit for bit | **PASS** — every behavioural kernel change is `#[cfg(cap-rooted-open)]`; the userland fallback is `sys_spawn`, as before |
| 5 | The prior-art section is present and its claims hold | **FAIL** — the survey is login/session prior art, not K1b's path-scoping mechanism; `THINK:90`'s "no client holds the root cap" is false (**F1**) |

**Findings:** **F1** (items 3/5) — the Lens is a client and is handed a root-scoped browse cap. **F2** (note) —
per-client revocation is lost because `sys_spawn_scoped` closes its own copies. No containment escape was found for
Arthur.

---

## 1. Escapes from a scoped directory cap — PASS (with a coverage note)

**`..` is folded before the check and the open, by the same function.** `normalize_absolute_path` splits on `/`,
drops `""`/`.`, pops on `..` and clamps at the root (`kernel/src/fs/mod.rs:1871-1893`). The check normalizes the
caller's path (`kernel/src/arch/x86_64/syscall.rs:6401-6405`) and `open_root_temporary` normalizes it again
(`fs/mod.rs:1471`) before `resolve_path_to_inode` (`fs/mod.rs:905-930`), so the string that passed the check is the
string that is resolved. Concrete probes: `/neofs/.arthur/../..` normalizes to `""` (not contained by
`neofs/.arthur`), and `/neofs/.arthur/../.system/x` normalizes to `neofs/.system/x` (not contained, and the
`system-store` rule also refuses it).

**The comparison is by whole components.** `scope_contains` requires `path == scope` or `path.starts_with(scope)`
with `path.as_bytes()[scope.len()] == b'/'` (`fs/mod.rs:1862-1869`). `neofs/.arthurx` is therefore not contained by
`neofs/.arthur`, and `""` (the root scope) contains everything.

**Symlinks do not create a bypass.** A final `InodeKind::Symlink` is refused (`fs/mod.rs:1479-1481`), and an
intermediate symlink is not traversed: NeoFS's `resolve_path` requires every intermediate record to be
`NeoFsKind::Directory` (`kernel/src/fs/neofs/inode.rs:191-210`); ext4's `lookup_raw` rejects `.`/`..` and does not
follow a link (`kernel/src/fs/ext4.rs:274-279`); the `/boot` namespace is a synthetic read-only `BootFs`
(`kernel/src/boot_modules.rs:441-457`). A planted `link -> /neofs/.system` under `.arthur` would make
`/neofs/.arthur/link/<file>` resolve to `NotFound`/`TypeForbidden`, not to the record.

**Rename and hard-link escapes do not exist as syscalls.** The only syscalls that take an absolute path are
`SYS_OPEN` (`syscall.rs:6361-6382`) and `SYS_MODEL_LOAD` (`syscall.rs:2898-2921`), and both are gated
(`syscall.rs:6396-6409`, `:2924-2940`). `SYS_READDIR`, `SYS_MOUNT`/`SYS_UMOUNT`, `SYS_FILE_REPLACE`, and the rest
are fd- or handle-based; there is no rename, link, symlink, mkdir or unlink syscall in this build.

**Mounts.** `sys_mount`/`sys_umount` take a directory handle and require the backing `OpenFile` and the handle to
both carry `MOUNT` (`syscall.rs:6650-6690`; `fs/mod.rs:557-587`); target names are single components, with `..` and
`/` refused (`fs/mod.rs:1171-1176`). Arthur's scopes carry no `MOUNT` (`userland/libneo/src/lib.rs:1156-1173`), so he
cannot install or remove a mount. A mount placed *under* the scope by a `MOUNT`-capable (trusted) process is still
under the scope's path, so it is not a lexical escape; see the path-string caveat below.

**Caveat (note, not a client escape): a scope is a path string, not an inode/dentry binding.** The scope recorded at
open is the normalized path (`fs/mod.rs:1512-1515`); `root_dir_cap_for` compares strings (`process.rs:1288-1291`) and
`open_root_temporary` re-resolves the path (`fs/mod.rs:1471-1478`). A `MOUNT`-capable process that later installs a
mount at or above that path re-points what the scope authorizes. The scoped holder cannot do that (no `MOUNT`), so it
is a trusted-broker property, not a client escape; it is worth naming because K1b's scope is not a dentry handle.

**Coverage note (recommend a host KAT).** The only discriminator is `desktop_arthur_scope`, which checks one outside
path (`/neofs`) and one inside (`/neofs/.arthur/memory`) with `LOOKUP|READDIR`
(`userland/neo-arthur/src/main.rs:941-965`; `tools/xtask/src/main.rs:16145-16151`). It does not exercise `..`, a
symlink, a mount, the `neofs/.arthurx` whole-component edge, or `EACCES` vs `EPERM`. `scope_contains` is pure string
logic and is cheap to KAT; add those cases.

## 2. Widening by derive, grant, spawn or IPC — PASS

**Derive attenuates.** `derive_handle` requires the child's `CapRights` to be a subset of the parent's
(`kernel/src/sched/process.rs:3261-3268`), and `Handle::File::with_cap` attenuates the file bits to that subset
(`process.rs:1142-1174`), keeping the same `Arc<OpenFile>` (and therefore the same `scope` string).

**Grant attenuates.** `sys_grant` requires `GRANT` on the source (`syscall.rs:7904-7907`); the recipient gets
`actual = source_rights ∩ offered ∩ max_rights` (`process.rs:3726-3753`), on a clone of the same handle. The scope is
preserved and the rights can only shrink.

**Spawn attenuates.** With `cap-rooted-open`, a `Handle::File` is inheritable only through
`is_spawn_inheritable_dir_cap` — a directory whose rights are exactly `LOOKUP|READDIR`, or a directory carrying
`GRANT` (`process.rs:1312-1318`); the child is a clone with the parent's rights (`syscall.rs:1918-1927`).

**A scoped-only process cannot mint a `/` cap.** `libneo::root_dir_cap()` recognizes only `FLAG_ROOT_DIR`
(`libneo/src/lib.rs:1105-1122`), which `SYS_HANDLE_INFO` sets only for `file.is_vfs_root()`
(`process.rs:1119-1123`). The scopes handed to Arthur are `vfs_root = false`, so `sys_spawn_client` on a scoped-only
process falls through to plain `sys_spawn` (`libneo/src/lib.rs:1143-1153`); it cannot derive a wider client root.
`OPEN_SCOPE_DIR` cannot widen either: a new scope must be contained by, and its requested rights must be a subset of,
the handle it was opened through (`process.rs:1274-1304`), and the installed handle's meta-rights are masked by that
parent (`syscall.rs:6433-6444`).

**Note (F2) — per-client revocation is lost.** `sys_spawn_client` derives a client root cap and **keeps it in the
WM's table** so the WM can `sys_revoke` it (`libneo/src/lib.rs:1143-1153`). `sys_spawn_scoped` opens the scopes,
passes them, then **closes its own copies** (`libneo/src/lib.rs:1200-1204`). On close the child's scope edge is
re-parented to the nearest live ancestor, which is the WM's `/` cap (`process.rs:3012-3047`), so the only handle the
WM has left over the child's scope is the root: revoking that client's directory cap now means revoking the session
root (which also takes every other file handle opened through it). That is acceptable for a logout-wide revoke, but
it does not give L3 the per-session/per-client revoke the THINK plans. Keeping the WM's scope copies (and closing
them at session end), or adding a revoke-by-cap-id path, would restore it.

## 3. Arthur in image A, and the "every client" clause — FAIL on the clause (F1)

**Arthur's own containment is right — PASS.** The desktop and compositor spawn paths use
`sys_spawn_scoped(..., ARTHUR_SCOPES)` (`userland/neo-init/src/desktop_session.rs:2057-2068`;
`userland/neo-init/src/arthur_compositor.rs:68-96`). `ARTHUR_SCOPES` is `/neofs/.arthur` read-write and `/boot`
read-only (`libneo/src/lib.rs:1156-1173`), plus the already-narrower memory-directory handle inherited from the WM
(`desktop_session.rs:2046-2057`). Image A's model and vocabulary are `/boot/models/...` and `/boot/vocab/...` in the
`arthur-boot-module` / `arthur-vocab-binary` build (`userland/neo-arthur/src/main.rs:196-213,287-288`), which
`desktop-with-model` selects (`tools/xtask/src/main.rs:6894-6906`); grants, undo and stakes are under `.arthur`
(`neo-arthur/src/main.rs:173`; `reversibility_logic.rs:9-13`); memory is under `.arthur` (`memory_logic.rs:10`). The
product feature pairing is asserted (`xtask/src/main.rs:6114-6117,6631-6643,20807-20814`), and the scenario's probe
(`neo-arthur/src/main.rs:941-965`) plus pass check (`xtask/src/main.rs:16145-16151`) prove the outside open is
`EPERM` and the inside one succeeds. Arthur holds no `/` handle, so `root_dir_cap()` finds nothing to derive from.

**F1. The Lens is a client and is handed a root-scoped browse cap — FAIL of the dispatch's "every client" clause.**
`lens::bloom` opens `PATH = "/"` with `LOOKUP|READDIR` (`desktop_session.rs:5181,5371-5382`) and passes that handle to
the Lens (`:5391`). Every directory opened by path records a scope (`fs/mod.rs:1512-1515`), so this handle's scope is
`""`; it is a `RootDirCap` in the kernel's own terms (`file.is_vfs_root()`; `process.rs:1119-1123`), and it is
inherited by the Lens because the directory carries `GRANT` (`process.rs:1312-1318`; `file-caps` is on through
`cap-rooted-open`, `kernel/Cargo.toml:97`, so the installed handle carries `GRANT|DERIVE`, `syscall.rs:6433-6444`).
Concrete input, in image A (`iso --with-model`, so `cap-rooted-open` is in `PRODUCT_IMAGE_KERNEL_FEATURES`): the
Lens calls `sys_open("/neofs", OPEN_ROOT_TEMPORARY, LOOKUP|READDIR)` — it returns a handle, because
`scope_contains("", "neofs")` holds and the rights cover the request (`fs/mod.rs:1862-1869`;
`process.rs:1288-1294`). A following `readdir` lists the entries that are there (`.arthur`, `.system`, and any model
files present). It cannot read a file
(the cap has no `READ`; a regular-file open with `LOOKUP|READDIR` fails at `fs/mod.rs:1504-1507`), and
`sys_open("/neofs/.system", ...)` is still `EPERM` through `system_store_check`
(`syscall.rs:6392-6395,8793-8806`). So the credential file is still protected; what fails is the stated invariant
**"no client holds the root cap"** (`docs/THINK-2026-10-27-ot66-login-session.md:90`) and the dispatch's "Arthur and
every client get only their own subtree" (`Plans/DISPATCH-2026-10-30-lanes-L1-L2-L7.md:9`). Fix one of: hand the
Lens a narrower directory (e.g. `/neofs` or its own listing root) instead of `/`, or qualify the THINK to "no client
holds a **read** root cap; the Lens holds a `LOOKUP|READDIR` browse cap". If the Lens is deliberately trusted WM
chrome, say so there too.

**Least-privilege note on `/boot`.** `/boot` is not Arthur's own subtree; it gives every boot module read (the
`BootFs` only serves ordinary modules such as the vocab, not consumable models, `boot_modules.rs:436-457`, and the
model itself goes through the gated `sys_model_load` at `syscall.rs:2924-2940`). It is a deliberate, read-only
deviation needed for the model and vocabulary, not a data leak, but it is a deviation from the literal "only their
own subtree" wording and should be recorded as such.

**Coverage note (the dispatch asked for "the product boot").** `desktop_arthur_scope` is a DS-1 Home boot on the
`arthur-memory` image (`xtask/src/main.rs:2820-2835,13850`), not `iso --with-model` image A, and image A's
`/boot/models` + `/boot/vocab` opens are not exercised by any cap-rooted-open scenario: the login boots do carry
`cap-rooted-open` (`kernel/Cargo.toml:915-931`; `userland/neo-init/Cargo.toml:167-193`) but their Arthur is the
`desktop-home` (non-`arthur-boot-module`) build, whose model path is `/neofs/...`. The pairing assert checks the
feature lists, not a boot. Recommend a model/vocab open in the scope probe, or one product-boot leg.

**Forward note (not a defect of image A).** `arthur-model-from-neofs` (OT-87 R2) loads `/neofs/model-*.neom` and
`/neofs/*.vocb`; `ARTHUR_SCOPES` has no `/neofs`, so that build would get `EPERM` on the model if it is ever paired
with `cap-rooted-open`. Image A uses `/boot`, so it does not bite today; add a read `/neofs` scope (or the two model
names) when that build gets the feature.

## 4. Off mode — PASS

**Kernel.** Every behavioural change is behind `#[cfg(feature = "cap-rooted-open")]`: the `scope` field and its
initializer (`fs/mod.rs:207-216,305-309`), the `OPEN_SCOPE_DIR` match arm (`:1458-1464`), the scope assignment
(`:1508-1517`), `scope_contains` (`:1861-1869`), `root_dir_cap_for` (`process.rs:1273-1304`), the model-load gate
(`syscall.rs:2932-2940`) and the open gate (`:6396-6409`). With the feature off, `open_root_temporary`'s flag match is
the same `OPEN_ROOT_TEMPORARY` / `OPEN_CREATE_REGULAR` / `_ => Invalid` shape as before, and neither
`open_path_and_install` nor `model_load_path` has a cap block. `OPEN_SCOPE_DIR` is a new constant, but the off kernel
still answers it `Invalid` (`fs/mod.rs:1463`), which is the pre-existing unknown-flag path.

**Userland.** `sys_spawn_scoped` is `sys_spawn` with the feature off (`libneo/src/lib.rs:1206-1207`), and the call
sites it replaced used `sys_spawn_client`, which is also `sys_spawn` with the feature off (`:1143-1153`). The
`login_record_guard` change (`sys_spawn` → `sys_spawn_client`, `userland/neo-init/src/desktop_session/login.rs:970`)
has the same off-mode value. `ARTHUR_SCOPES` is a new constant with no off-mode effect.

**One configuration note, not an off-mode break:** `neo-test-login` now includes `cap-rooted-open`
(`kernel/Cargo.toml:915-931`) and `login-boot-test` now includes it (`userland/neo-init/Cargo.toml:167-169`), so those
test configurations are **on**, not off; the off mode itself is unchanged. **PASS.**

## 5. Prior art — FAIL (survey present, but not for K1b's mechanism; and one claim is false)

**What is present.** `docs/THINK-2026-10-27-ot66-login-session.md` §11 (`:528-552`) is a real survey (Fuchsia,
Genode, seL4/camkes-tool, systemd/logind, PAM, greetd, Redox, RustCrypto/argon2) with licence and last-commit
columns, and the "checked 2026-10-27" provenance line. Its login/session claims are plausible; the Fuchsia row
honestly marks its own licence/SHA as unverified. I did not re-fetch the upstream rows from here.

**What is missing for this build.** K1b's mechanism is a **directory-scoped path capability** — the same shape as
Linux `openat2`'s `RESOLVE_BENEATH`/`RESOLVE_NO_SYMLINKS` (5.6+), Capsicum's `cap_enter`/`cap_rights_limit`/
`cap_fcntls_limit` (FreeBSD), and the classic chroot-style `..`/symlink rules; the important design question is
path-prefix vs dentry/inode binding, which is exactly the caveat in §1 here. None of that is in §11, and the
K1b-as-built section (`:171-183`) adds no survey. The lane dispatch required "the prior-art survey for unfamiliar
subsystems" (`Plans/DISPATCH-2026-10-30-lanes-L1-L2-L7.md:5-6`, L1.1 at `:9`), so the absence is a missing deliverable,
not a formatting choice.

**The claim that does not hold.** `THINK:90` says "K1b gives each client only directory caps under its home, so no
client holds the root cap". The Lens holds a `/` `LOOKUP|READDIR` scope (F1). Even if the Lens is judged trusted, the
sentence needs the qualifier; as written it is false on the code at this head. **FAIL.**

---

## Outside the five (notes, no additional blocker)

1. **F1 is the substantive finding; F2 is a design note.** F1 is a least-privilege/browse disclosure, not a
   credential-file read: no `READ`, and `.system` is refused at open. Fix or qualify it before merge, or record it as a
   deliberate WM-chrome exception.
2. **K1b close-gate coverage.** The THINK's K1b row lists `..`/absolute paths, rights-never-widen, revoke, a narrowed
   handle's `readdir`, and `open_at` (`THINK:485`). This PR implements scoped directory caps, not `open_at`, so the
   `open_at` legs do not apply; the `..`, symlink, rights, narrowed-handle and revoke legs do, and only one
   outside/inside open is tested. Add (a) a host KAT for `scope_contains` (`""`, `a/b` vs `a/bc`, `a/b/../c`) and for
   the `EACCES`-vs-`EPERM` split, (b) a scenario leg for a `..` path and a too-narrow scope, and (c) a revoke leg if
   the WM keeps its scope copy (F2).
3. **The dispatch's "product boot" is not what the new scenario boots.** `desktop_arthur_scope` is a DS-1 Home boot;
   `login_*` boots the sign-in desktop with the feature but not the model build. The containment proof is real for
   Arthur's spawn path; the image A model/vocab path is only feature-asserted. This is a named coverage gap, not a
   code defect.
4. **`system_store_check` is path-based, not handle-based.** Arthur's scopes exclude `.system`, so this is not a K1b
   bypass; but if a future scope (or an inherited handle) ever covers `.system`, the open-time check does not protect
   it. Worth a sentence in the L3 plan.
5. **`sys_spawn_client` still hands a full `/` (all but `MOUNT`) to the shell/installer and the test drivers**
   (`libneo/src/lib.rs:1143-1153`; `neo-init/src/main.rs:4561+`). K1b ported Arthur, not every client. Image A spawns
   no client this way (the Lens's root cap comes from the WM's plain `/` open, F1), but if the installer/Try kernel
   later turns on `cap-rooted-open`, the shell path needs the same scoping.
6. **Path-string scope vs mount replacement** (§1 caveat) is the one design difference from a dentry-bound capability
   such as Capsicum/`openat2`; it is broker-controlled today, so a note rather than a defect.

## Disposition

**Arthur's containment passes by inspection; the every-client clause and the prior art do not.** I found no way for a
holder of a scoped directory cap to escape its subtree by `..`, symlink, mount/umount, rename/hard link, or an
ungated path syscall: the path is normalized once and checked by whole components, the only two path syscalls are
gated, symlinks are not traversed, and mount authority is absent from Arthur's scopes. The finding is **F1**: the Lens
— a client — is handed `/` with `LOOKUP|READDIR` and can browse the namespace (it cannot read a file; `.system` is
still refused), contradicting `THINK:90` and the lane's "every client". **Item 5 fails** because the survey covers
login/session prior art, not K1b's path-scoping mechanism, and the `THINK:90` claim is false on this code. **F2**
notes that closing the WM's scope copies trades away per-client revocation. Recommend: narrow or justify the Lens's
cap and fix the claim; add the `openat2`/Capsicum prior art; add the `..`/rights/revoke test legs; keep the WM's
scope copies if L3 needs per-client revoke.

This was a code review. I ran no gate and claim no CI or QEMU result.
