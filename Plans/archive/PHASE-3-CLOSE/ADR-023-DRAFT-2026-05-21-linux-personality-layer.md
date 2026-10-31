# ADR-023: Linux binary compatibility via personality layer

| | |
|---|---|
| **Status** | DRAFT — awaiting PL ratification |
| **Date** | 2026-05-21 |
| **Authority** | PM (proposal); PL (ratification per Bible Articles 24, 27) |
| **Supersedes** | None |
| **Superseded by** | - |
| **Related** | Bible Articles 1.4 (no parity-chasing), 3 (what we are not), 5 (userspace architecture), 9.2 (no loadable modules), 9.7 (USB HID input), 9.10 (networking devices), 11 (networking), 12.1 (capability-based security), 13 (phase ledger), 23.1 / 23.2 / 23.3 (no system AI persona / no telemetry / no embedded model), 28 (Product Accessibility, especially 28.2); ANSPEC-001 §9 (accelerator driver class); ADR-022 (fault-recovery forward marker — comparable forward-marker shape for sub-decisions deferred to Phase 8); `docs/research/ALTIMIT-real-world-implementation.md` (informational); `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-21-articles-3-and-5-6-and-28-2-personality-subsystems.md` (co-ratified Bible amendment) |

---

## Context

NeoPlutarch's product-accessibility posture is captured in Bible Article 28. Article 28.2 commits the project to a goal: "Run useful software. The POSIX compatibility shim (Article 3) means common command-line software with sane build systems can be ported with reasonable engineer effort. NeoPlutarch is not Linux, but it is not so alien that the existing software ecosystem is unreachable."

Bible Article 3's fourth bullet currently authorizes that compatibility shim in a single sentence ("We may offer a POSIX compatibility shim for porting userland tools, but the native ABI is our own"). The sentence permits the work but does not specify the mechanism, the binding model, or the isolation guarantees. Without specification, two failure modes are foreseeable:

1. Ad-hoc Linux-isms creep into the native syscall surface as individual ports paper over individual missing features. The native ABI degrades into "POSIX, but slightly different" — the worst of both worlds.
2. The project ships without any concrete compatibility surface and Article 28.2's accessibility commitment goes undelivered. The contribution surface narrows to people willing to rewrite their software against an unfamiliar ABI.

The triggering PL question (informal, 2026-05-21): *"I don't want to force people to build something just for Plutarch, can we make it so we can easily port Linux apps and games to the OS?"* — is the user-facing statement of Article 28.2's commitment.

This ADR ratifies the architectural mechanism by which that commitment is honored: a Linux **personality subsystem**, kernel-resident, opt-in per process, isolated from the native ABI by construction. The mechanism is general (a future ADR may justify a BSD or other personality), but this ADR commits the project only to Linux.

The shape of the personality model is well-established prior art. FreeBSD's Linuxulator has been in production for ~25 years; Solaris `lx` zones (commercialized by Joyent / SmartOS) is its commercial cousin; NetBSD's COMPAT_LINUX is the smaller-project version. WSL1 is the canonical failure precedent and is examined under "Alternatives considered" below.

### Bible alignment

This ADR is consistent with the following existing Bible commitments:

- **Article 1.4** — "Parity is not a goal; rightness is." The personality layer is *bounded* compatibility, not parity. It covers Linux userspace ABI; it explicitly does not cover Linux kernel-API extensions (cgroups, eBPF, namespaces, perf_event, kernel module loading). Those are Linux-internal architectural choices and reproducing them in NeoPlutarch would import Linux's internal model wholesale, which Article 1.4 declines.
- **Article 3** — fourth bullet permits "a POSIX compatibility shim for porting userland tools." The personality subsystem is the architectural form of that shim. The Bible amendment co-ratified with this ADR refines the fourth bullet to point at Article 5.6, replacing "shim" (a casual carve-out) with "personality subsystem" (a defined mechanism).
- **Article 5.5** — "There is no fork() — process creation is spawn() with an explicit ELF or memory image, by design." The personality layer emulates Linux `fork` *for personality-tagged processes only*; the native ABI continues to have no `fork`. The emulation lives in the personality, not in the native syscall surface.
- **Article 6.6** — "The kernel exposes inference-governance hooks but does not implement governance policy." The personality layer follows the same posture for foreign-OS compatibility: the kernel exposes the *mechanism* (syscall translation, capability binding, filesystem worldview), and userspace (or the personality's own userspace daemons) implements the *policy* (which Linux distro tree to mount, which userspace agents serve `/proc` and `/sys`).
- **Article 9.2** — "Driver loading is static at compile time for Phase 1–3. Loadable kernel modules are explicitly deferred and may never arrive." Personality subsystems are *statically compiled* (not loadable), satisfying this constraint. Conditional compilation via Cargo features is permitted because that is a build-time decision, not a runtime load.
- **Article 12.1** — "Every kernel object is referenced by a capability handle. Holding the handle is permission; there is no ambient authority." Linux file descriptors map cleanly to capability handles; the personality preserves the capability discipline for personality-tagged processes. Linux's `SCM_RIGHTS` fd-passing maps to NeoPlutarch's capability-passing IPC.
- **Article 23.1 / 23.2 / 23.3** — No system AI persona, no telemetry, no embedded model. None of these are touched by the personality layer; the personality runs userspace processes and does not introduce any new kernel-level model, persona, or data-collection surface.
- **Article 28.2** — *This is the article the personality layer concretely delivers.* The amendment co-ratified with this ADR updates Article 28.2's cross-reference from "the POSIX compatibility shim (Article 3)" to "personality subsystems (Articles 3, 5.6); the Linux personality (ADR-023)…", connecting the accessibility goal to the architectural mechanism.

### Scope

**Within scope (this ADR):**

- The personality-layer architectural model and binding mechanism.
- Tier scoping of Linux syscalls (Tier 1 / Tier 2 / Tier 3) and the bright-line exclusion list.
- Fork emulation strategy.
- Filesystem worldview model (Linux rootfs as bind-mounted overlay; `/proc` and `/sys` via userspace daemons; `/dev` static population).
- Graphics-stack design constraint (accelerator drivers per ANSPEC-001 §9 must remain *expressible* as Linux DRM-compatible character devices for the personality, without restructuring internally).
- Windows-game compatibility-by-transitivity via Proton-on-Linux-personality (no Windows personality required).
- The co-ratified Bible amendment (Articles 3, 5.6, 28.2; footer bump v1.6.0 → v1.7.0).
- Phase placement (implementation work is Phase 8+; forward markers identified for Phases 4, 6, 8).

**Out of scope (this ADR):**

- The actual implementation. PERSPEC-001 ("Linux Personality Specification") will be drafted at Phase 8 THINK; this ADR is its design contract.
- Other foreign-OS personalities (Windows, macOS, BSD). The personality *mechanism* is general; this ADR commits to *Linux only*. A future ADR may justify additional personalities and would reuse the mechanism.
- The native socket-equivalent API (Phase 4 design substrate). The personality must translate Linux BSD sockets onto whatever Phase 4 defines; that translation is a Phase 8 task built atop Phase 4's primitives, not a constraint on Phase 4's design beyond "be translatable from BSD sockets."
- Linux kernel-API extensions (namespaces, cgroups, eBPF, perf_event, kernel modules, Linux `capabilities(7)`). These are bright-line excluded; see Q3 below.
- Graphics-stack implementation (Mesa port, Vulkan compatibility surface, audio personality, input device personality). Those are Phase 9–10+ work atop the Linux personality and Phase 4/6 substrate.
- Training-time `.neom` import compatibility with Linux ML toolchains. Userspace `neo-import` is unrelated to the personality.

---

## Q1: Architectural model — where in the stack does personality dispatch happen?

### The question

Linux applications make raw syscalls through `syscall` instructions in inline assembly, sometimes inside statically-linked binaries that bypass libc entirely (Go binaries are the canonical example; many Rust binaries; many security tools that intentionally avoid libc). Where must the personality translation layer live to intercept these calls?

### Choice: kernel-resident syscall dispatch, branch on per-thread personality flag

The personality is a kernel subsystem at `kernel/src/personality/<name>/`. The kernel's syscall entry path reads the calling thread's **personality flag** (a small enum on the `Thread` struct: `Native` | `Linux`, extensible) and dispatches accordingly:

- `Native`: routes to the existing native syscall table.
- `Linux`: routes to `kernel/src/personality/linux/dispatch.rs`, which translates Linux syscall numbers and ABI conventions into native kernel operations.

The two dispatch tables are completely disjoint. A `Linux`-personality thread cannot invoke a native syscall by number; a `Native` thread cannot accidentally invoke a Linux syscall. Cross-personality interop happens through shared kernel objects (capability handles, file descriptors, shared memory mappings) — never through syscall-number aliasing.

### Rationale

Three properties this placement gives that other placements do not:

1. **Unmodified-binary compatibility.** Catches raw `syscall` instructions regardless of language, libc, or static linkage.
2. **Native ABI invariance.** The native syscall surface is unchanged. Personality-layer growth or evolution does not perturb native applications.
3. **Composable with capability discipline.** A capability handle resolved by a personality syscall is the same kernel object as one resolved by a native syscall. The capability table is per-process; the personality flag is per-thread (or per-process, see Q2); the discipline composes cleanly.

### Alternatives rejected

- **Userspace-only ABI shim** (the WSL1 first-design pattern). Rejected: cannot intercept `syscall` instructions in statically-linked binaries that bypass libc. WSL1's eventual abandonment in favor of WSL2 (a VM) is the canonical evidence.
- **VM hosting Linux as a guest** (the WSL2 pattern). Rejected: (a) requires virtualization infrastructure NeoPlutarch does not have and does not plan to host, (b) defeats the kernel-arbitrated AI substrate by giving Linux apps a separate kernel that does not see NeoPlutarch's AI primitives, (c) duplicates kernel resources across two kernels. Single-kernel hosting via personality keeps the AI substrate visible to all processes regardless of personality.
- **Loadable kernel module containing the personality** — rejected by Bible Article 9.2.

---

## Q2: Binding — when and how does a process acquire a personality?

### The question

A process needs a personality tag before its first syscall. How is the tag established and inherited?

### Choice: bound at `exec`, inherited by descendants, recognized via ELF brand + interpreter heuristics

The personality is fixed at `exec` time. The native ELF loader inspects, in order:

1. **Explicit ELF brand note.** A `NT_NEOPLUTARCH_PERSONALITY` note (analogous to FreeBSD's `NT_FREEBSD_ABI_TAG`) is the authoritative marker for native ELFs and explicitly-tagged foreign ELFs. Linux ELFs typically carry `NT_GNU_ABI_TAG`; presence of that note triggers Linux personality.
2. **Interpreter path.** ELFs naming `/lib64/ld-linux-x86-64.so.2` (or `/lib/ld-linux.so.2`) as their interpreter are recognized as Linux binaries even without the brand note.
3. **Fallback symbol heuristics.** Presence of `glibc`-versioned symbols (`GLIBC_2.x`) confirms Linux personality when brand note and interpreter path are absent or ambiguous (some statically-linked binaries strip the interpreter).

If none of the above match, the binary is assumed `Native`. Mis-detection is non-catastrophic: a Linux binary running under `Native` personality fails at its first syscall with a clear diagnostic; a Native binary cannot match Linux heuristics by accident (Native ELFs are produced by the NeoPlutarch toolchain with the explicit brand note).

Descendants of a personality-tagged process inherit the parent's personality across `spawn` and across the personality's emulated `fork` (see Q4). A process may *change* personality only by re-execing a binary of a different personality — there is no syscall that toggles personality mid-run.

### Rationale

- **Per-process, not per-thread.** Threads in a Linux process share an address space, an FD table, and (per Linux semantics) signal disposition. Splitting personality at thread granularity would require translating between two ABIs across thread boundaries within one address space — a class of bug that has historically been catastrophic in cross-personality systems. Per-process binding eliminates the category.
- **Bound at exec, not at spawn.** A process's personality is determined by the binary it runs, not by how it was created. This matches the established Linuxulator pattern.
- **Heuristic detection is sufficient.** Brand notes are authoritative; interpreter path is reliable for ~all dynamically-linked Linux binaries; symbol fallback covers the static-link edge cases. The combination has been adequate for FreeBSD for two decades.

### Alternatives rejected

- **Per-thread personality.** Rejected: the cross-personality intra-process ABI translation cost is not worth any flexibility it would offer. No use case justifies it.
- **Personality declared at process spawn by the parent, not the binary.** Rejected: forces the parent to know the child's ABI, which is exactly the kind of caller-side knowledge that POSIX got wrong with `execve`'s shell-vs-binary detection. Detecting from the binary is correct.
- **Syscall to change personality mid-run.** Rejected: opens an entire class of confused-deputy attacks. Personality is established by what code is running; re-exec is the legitimate way to change personality.

---

## Q3: Tier scoping — what does the Linux personality actually emulate?

### The question

The Linux ABI is large and growing. What subset must NeoPlutarch's personality cover, what subset is excluded by design, and what is acceptable to defer?

### Choice: three tiers + a bright-line exclusion list

**Tier 1 — Phase 8 initial deliverable.** The essential POSIX surface that gets a `bash` shell, a compiler, and a Python interpreter running:

- File I/O: `open` / `openat` / `close`, `read` / `write` / `pread` / `pwrite`, `lseek`, `stat` / `fstat` / `lstat` / `newfstatat`, `access` / `faccessat`.
- Memory: `mmap` / `munmap` / `mprotect`, `brk`, `madvise` (best-effort).
- Process: `fork` (emulated — see Q4), `execve`, `exit` / `exit_group`, `wait4`, `getpid` / `getppid` / `getuid` / `geteuid` / `getgid`.
- Signals: `rt_sigaction`, `rt_sigprocmask`, `rt_sigreturn`, `kill`, `tgkill`.
- IPC / pipes: `pipe` / `pipe2`, `dup` / `dup2` / `dup3`.
- Terminal: minimal `ioctl` for `TCGETS` / `TCSETS`, `isatty`-relevant queries.
- Time: `clock_gettime`, `nanosleep`, `gettimeofday`.

**Tier 2 — Phase 9.** Full POSIX threading and networking:

- Threads: `clone` (with the specific flag combinations glibc and musl use for `pthread_create`), `futex`, `set_robust_list`, `set_tid_address`, `gettid`.
- Networking: `socket` / `bind` / `listen` / `accept` / `connect` / `send` / `recv` family, mapped onto NeoPlutarch's Phase 4 sockets-equivalent.
- Polling: `poll` / `ppoll`, `select` / `pselect`, `epoll_create1` / `epoll_ctl` / `epoll_wait` (mapped to NeoPlutarch's event mechanism).

**Tier 3 — Phase 10+, as need arises.** Linux-isms with real but narrower utility:

- `inotify`, `eventfd`, `signalfd`, `timerfd`.
- `prlimit64`, `sched_getaffinity` / `sched_setaffinity`.
- `getrandom` (trivial — maps to native primitive).
- Extended attribute syscalls (`getxattr` / `setxattr`) — only if NeoFS exposes xattrs natively.

**Explicitly excluded — not in scope, ever, without a superseding ADR.** These are kernel-API extensions that would require importing Linux's internal architecture:

- Mount / network / PID / user / IPC / cgroup **namespaces**. NeoPlutarch's native primitives serve the same purposes through different shapes (capability handles).
- **cgroups**. Resource accounting is a native concern with a native API.
- **eBPF**. Bible Article 12.4 forbids in-kernel user-provided code execution.
- Linux's **`capabilities(7)`** (`CAP_NET_ADMIN`, etc.). Conflicts directly with NeoPlutarch's capability model.
- **`perf_event_open`**, kernel **tracepoints**, **kprobes**. NeoPlutarch's observability is native (ADR-014 territory).
- **`ptrace`** beyond a minimal subset sufficient for `gdb`-style debugging of Linux processes.
- **Kernel module loading** (`init_module`, `finit_module`). Bible Article 9.2.

### Rationale

The Tier 1 / 2 / 3 split is sequenced for delivery value: a Tier 1 implementation gets a developer shell + compiler + scripting language running, which is the bar for "useful Linux compatibility." Tier 2 adds servers and threaded software. Tier 3 is convenience.

The bright-line exclusion list is the most important defensive design choice in this ADR. WSL1's failure modes were (a) trying to emulate Linux kernel features beyond syscalls, and (b) lacking organizational incentive to maintain compatibility against an evolving Linux. The exclusion list pre-empts (a). Risk (b) is managed by pinning to a Linux LTS baseline rather than chasing latest (see Q6 / Consequences).

### Alternatives rejected

- **"Whatever Linux supports, we support."** Rejected as scope-unbounded and as Bible Article 1.4 violation (parity-chasing).
- **No tiering; one big-bang implementation.** Rejected as risk-unbounded for delivery.
- **Looser exclusion list.** Rejected on each candidate exclusion individually; each is excluded for a Bible-grounded reason (Article 9.2 for modules, Article 12.4 for eBPF, etc.).

---

## Q4: Fork emulation

### The question

Bible Article 5.5 rejects `fork()` as a native primitive. The Linux personality must nevertheless emulate it, because Linux applications (shells, build systems, daemon supervisors) hammer `fork`. How is the emulation built without contaminating the native ABI?

### Choice: COW page-table snapshot, implemented inside the personality

`fork()` in the Linux personality maps to a native `spawn`-equivalent that creates a new process *plus* a page-table snapshot of the parent. The snapshot uses copy-on-write at the page level — the same mechanism Linux uses, just constructed explicitly by the personality rather than inherited from a native primitive.

For `vfork()` (which Linux apps use to optimize the fork-then-exec pattern), the personality emulates the suspend-parent semantics without the COW work — the parent is parked until the child execs or exits.

`clone()` with thread flags is a different code path: it maps cleanly to native thread spawn, because thread creation *is* a native primitive on NeoPlutarch. `clone()` with process-forking flags maps to the `fork()` path above.

### Rationale

- **Native ABI unchanged.** The native syscall surface still has no `fork`; Article 5.5 holds. The COW mechanism lives inside the personality, consuming native page-table and process-creation primitives.
- **Cost is in the personality.** Linux applications that fork heavily pay an overhead that native NeoPlutarch applications do not. Acceptable in absolute terms; native applications are unaffected.
- **`vfork` is the genuine optimization.** The fork-then-exec pattern that dominates real workloads (shells, `make -j`, daemon supervisors) is `vfork`-optimizable, which makes the worst case substantially better than naive-`fork`-emulation suggests.

### Alternatives rejected

- **Emulate `fork` by full address-space copy (no COW).** Rejected: prohibitively expensive for the workloads that matter.
- **Add `fork` as a native primitive.** Rejected: explicit Bible Article 5.5 violation.
- **Refuse to emulate `fork`; require Linux apps to be patched.** Rejected: defeats the personality's purpose; Article 28.2's "reasonable engineer effort" promise breaks if every patched-out `fork` is a porting tax.

---

## Q5: Filesystem worldview

### The question

Linux applications expect a specific directory tree (`/usr`, `/lib`, `/bin`, `/etc`, `/var`, `/tmp`, `/proc`, `/sys`, `/dev`, `/home`). NeoPlutarch's NeoFS namespace is POSIX-ish (Bible Article 10.2) but is not Linux's hierarchy. How does the personality present a Linux-shaped filesystem without contaminating NeoFS or polluting the native namespace?

### Choice: Linux rootfs as bind-mounted overlay, with three emulated special directories

A **Linux rootfs** (a real Linux distribution's userland — Debian, Alpine, or similar) is mounted at a chosen path in the native filesystem, e.g. `/linux-rootfs`. The choice of distribution is per-deployment. A Linux-personality process sees `/linux-rootfs/*` as `/*` — path translation happens at the syscall boundary in the personality.

Native processes continue to see the same files at their real native paths. No path overlay magic for native processes; the translation is one-directional and explicit, scoped to personality-tagged processes only.

Three special directories are not real files — they are emulated:

- **`/proc`** is served by a userspace daemon (provisional name: `neo-procfsd`) that translates native observability syscalls into Linux-shaped `/proc` files. Process-state files (`/proc/<pid>/status`, `/proc/<pid>/maps`, `/proc/cpuinfo`, `/proc/meminfo`) are the priority surface. Phase 8 ships a curated subset; later phases extend coverage as concrete Linux applications demand specific files.
- **`/sys`** is served by `neo-sysfsd`. Coverage is intentionally narrow — Linux's `/sys` exposes substantial kernel-internal architecture that NeoPlutarch does not share. Priority paths: `/sys/class/net`, `/sys/class/block`, basic device topology. Applications relying on deep `/sys` introspection may fail; this is accepted.
- **`/dev`** is statically populated by the personality at startup with standard device nodes (`/dev/null`, `/dev/zero`, `/dev/urandom`, `/dev/tty`, `/dev/ptmx`, allocated `/dev/pts/N`, USB-HID `/dev/input/event*` once Phase 4 lands) backed by native device primitives.

### Rationale

- **NeoFS unchanged.** Bible Article 10's design is untouched; a Linux distro tree is just files in NeoFS.
- **Per-personality view.** Native applications never see the Linux path translation; the personality applies it as a syscall-boundary transformation.
- **Userspace daemons own policy.** `/proc` and `/sys` content is policy (which fields exist, what they expose, how often they update). Per Article 6.6's policy-vs-mechanism posture, that lives in userspace. The kernel exposes the observability primitives the daemons consume.

### Alternatives rejected

- **`chroot`-style hard isolation.** Rejected: too coarse; native processes lose access to shared files (`/tmp`, user home directories).
- **Filesystem-level namespace bind mount (Linux's mount-namespace pattern).** Rejected: requires importing Linux's namespace architecture, which is in the bright-line exclusion list.
- **Synthesize Linux paths purely at the personality syscall boundary with no real `/linux-rootfs`.** Rejected: requires the personality to know about every Linux file path applications expect, which is the unbounded scope WSL1 ran into.

---

## Q6: Graphics, audio, games — what does this ADR commit and what does it defer?

### The question

Linux games depend on graphics (Vulkan via Mesa, DRM/KMS at the kernel boundary), audio (PulseAudio / PipeWire), and input (libinput / evdev). The personality alone doesn't run games; an entire userspace graphics stack does. What design commitments does the personality require *now* from earlier phases, so that the eventual graphics-stack work in Phase 9–10 is not blocked by retrofit?

### Choice: one constraint on Phase 6 accelerator drivers; the rest is Phase 9+ work

The personality requires that NeoPlutarch's accelerator drivers (ANSPEC-001 §9) remain *expressible* as Linux-DRM-compatible character devices for personality-tagged processes. Implementation: a personality-only shim at `kernel/src/personality/linux/drm.rs` exposes `/dev/dri/cardN` and `/dev/dri/renderDN` semantics, calling into the underlying ANSPEC-001 accelerator subsystem.

The constraint is "exposes a compatible interface," not "is structured like Linux DRM internally." Native applications continue to use ANSPEC-001's syscall surface; only personality-tagged processes touch the DRM-compat path. ANSPEC-001 §9 may need an amendment when graphics drivers actually ship (Phase 6), but this is a small surface-area constraint, not a structural redirect.

Everything else — Mesa port, Vulkan compatibility, audio personality (PulseAudio/PipeWire shim), input device personality (`/dev/input/event*` backed by Phase 4 USB HID) — is Phase 9+ work and is **not** committed by this ADR. Those will be their own ADRs when Phase 9+ planning opens.

**On Windows games.** Proton (Valve's Wine + DXVK + VKD3D bundle) is itself a Linux application. If NeoPlutarch's Linux personality runs Linux applications and the graphics stack exposes Vulkan, Proton runs unmodified, and Windows games run inside Proton. **NeoPlutarch does not need a Windows personality.** The Linux personality, plus a working Vulkan stack, plus a Proton installation, covers a substantial fraction of the Steam catalog transitively.

### Rationale

- **Minimal early commitment, maximum late flexibility.** This ADR commits to one design constraint (Phase 6 DRM-compat option preserved). Everything else is deferred to its own ADR when its phase opens. ADR-022 is the precedent for this disciplined-deferral pattern.
- **Single graphics personality covers two ecosystems.** Linux+Proton subsumes both Linux and Windows games; this is an unusually high-leverage choice.

### Alternatives rejected

- **Commit now to a full graphics-stack design.** Rejected: too early; Phase 6 has not opened, accelerator driver design is downstream of Phase 5 AI substrate work. Premature commitment risks Phase 6 retrofit.
- **Write a Windows personality.** Rejected: Proton covers the use case transitively. Wine+DXVK is far more mature than any in-tree Windows personality would reach in any reasonable timeline.

---

## Alternatives considered (architectural-level)

### Alternative A — No foreign-OS compatibility

Maintain the native ABI alone; require explicit porting for every Linux application.

**Rejected.** Directly conflicts with Bible Article 28.2's accessibility commitment. The accessibility goal exists in the Bible specifically to prevent this stance.

### Alternative B — WSL2-style hypervisor guest

Run an actual Linux kernel as a hypervisor guest; Linux apps run inside the guest.

**Rejected.** (a) Requires Phase 6+ virtualization infrastructure not in any current phase plan. (b) Defeats the kernel-arbitrated AI substrate by giving Linux apps their own kernel that does not see NeoPlutarch's AI primitives. (c) Duplicates kernel resources across two kernels. The personality keeps the kernel singular and the AI substrate visible regardless of personality.

### Alternative C — Userspace-only libc shim

Implement Linux ABI as a userspace library.

**Rejected.** WSL1's first design. Cannot intercept `syscall` instructions in statically-linked binaries that bypass libc. Architecturally insufficient for the corpus of real Linux software (Go binaries especially).

### Alternative D — Adopt POSIX as the native ABI

Make POSIX the native shape; eliminate the dual-personality model.

**Rejected.** Bible Article 3 forbids. Also collapses the AI subsystem's syscall numbering (0x100..0x1FF) into POSIX's numbering and forces every kernel design decision to negotiate with POSIX's semantics (FDs as universal handles, `fork`'s COW expectations, signal ordering, `errno` set).

### Alternative E — Multiple foreign-OS personalities at launch (Linux + BSD + macOS + Windows)

Implement personalities for several foreign OSes simultaneously.

**Rejected for now.** Linux is justified by ecosystem size and by Proton providing Windows-game compatibility transitively. macOS (Darwin / Mach IPC / Cocoa frameworks) and BSD (similar but different from Linux in detail) lack the same payoff. The personality *mechanism* defined here is general; a future ADR may justify a second personality and would reuse the infrastructure.

### Alternative F — Loadable kernel module for the personality

Make the personality dynamically loadable.

**Rejected.** Bible Article 9.2.

### Alternative G — Forward-marker ADR only; defer all Bible amendments

Ratify design intent without amending the Bible.

**Considered and surfaced to PL as Option A in `Plans/PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md`.** PL chose Option B (this ADR + Bible amendment) over Option A and Option C. PM's recommendation had been Option A. PL's choice is recorded and respected; this ADR is the Option B work product. The forward-marker shape (ADR-022) remains available for *sub-decisions* deferred from this ADR (graphics-stack details, Mesa port, audio personality) — those will become their own forward-marker ADRs as their phases open.

---

## Consequences

### Positive

- **Article 28.2 commitment becomes deliverable.** The Bible's accessibility goal acquires a concrete architectural mechanism, not just a permissive sentence in Article 3.
- **Adoption friction reduced.** Linux developers can run their applications on NeoPlutarch without porting once Phase 8 lands. The contribution surface widens.
- **Game compatibility via Proton.** Without a Windows personality, a large fraction of the Steam catalog becomes reachable through Linux+Proton.
- **Dev tools work unmodified.** Compilers (gcc, clang), runtimes (Python, Node, Ruby), editors, build systems (make, ninja, cargo), version control (git) all work once Tier 1 lands. Major productivity multiplier for contributors.
- **Server software works.** nginx, postgres, redis, sqlite, and similar daemons run once Tier 2 ships.
- **Native ABI uncompromised.** The native syscall surface is unchanged by personality work. AI subsystem syscalls (0x100..0x1FF) remain NeoPlutarch's alone. Personality processes can access AI primitives only through native-ABI translation paths the personality layer chooses to expose.

### Negative

- **Maintenance burden against an evolving Linux.** Linux 6.x adds syscalls regularly. **Mitigation:** pin to a Linux LTS baseline (the current LTS at Phase 8 launch); accept that newer-Linux-only software may not run. Article 1.4 (no parity-chasing) authorizes this stance explicitly.
- **Fork emulation is slower than native Linux.** Heavy-fork workloads pay overhead native applications do not. **Mitigation:** confined to the personality layer; the COW mechanism reuses native page-table primitives; `vfork` covers the common fork-then-exec optimization.
- **Two filesystem worldviews.** Personality processes see Linux paths; native sees native. Path translation between worldviews is a real source of subtle bugs. **Mitigation:** explicit binding at exec; one-directional translation; no implicit overlay for native processes.
- **Graphics-stack design constraint on Phase 6.** Accelerator drivers must remain expressible as Linux-DRM-compatible character devices. **Mitigation:** the constraint is at the interface, not at internals. Native applications still use ANSPEC-001's syscall surface; only the personality-only shim touches DRM compat. ANSPEC-001 §9 amendment when graphics drivers ship.
- **Security surface expands.** Every emulated Linux syscall is a new path into the kernel. **Mitigation:** the personality is opt-in at compile time (Cargo feature) and at runtime (only invoked by personality-tagged threads). Bible Article 15 (Rust standards, `unsafe` discipline, no `unwrap` in steady-state) applies fully. Personality syscalls are held to the same standards as native syscalls and subject to the same fuzzing regime.
- **Risk of "personality scope creep."** Temptation to add Linux-only features to satisfy individual ports is real. **Mitigation:** Q3's bright-line exclusion list is explicit; adding a syscall outside declared tiers requires a superseding ADR, not a quiet PR.

### Neutral

- **Phase placement is late.** Implementation is Phase 8+. Earlier phases proceed on native primitives. The personality layers atop them.
- **No personality work blocks the AI substrate.** Phases 4–7 (network, AI substrate, accelerator drivers, inference scheduler) proceed entirely on native primitives. Personality is strictly additive on top.
- **Personality is opt-in at every level.** Build-time (Cargo feature), runtime (per-process binding), deployment-time (deployments not shipping a Linux rootfs don't expose the personality). Users who don't run Linux software never load personality code.
- **The mechanism is reusable.** If a future ADR justifies a second personality, this ADR's design generalizes: new directory under `kernel/src/personality/<name>/`, new variant on the personality enum, new dispatch table. No retrofit needed.

---

## Forward markers (implementation hooks accounting for actual phase state)

Phases 1.5 and 2 are closed; Phase 3 is substantively closed (BUILD-5b bare-metal validation pending). Hooks PM's earlier (rescinded) draft attempted to land in those phases are now Phase 8 retrofit work — touch the `Process` struct, syscall entry, and CI when Phase 8 opens; cost is essentially the same. The forward markers that remain live, indexed by phase:

### Phase 4 (network stack — not yet opened; THINK pre-loaded)

- **Phase 4 design hook:** the native sockets-equivalent API design considers Linux BSD-sockets translation as a downstream constraint. The personality's Tier-2 socket-family translation is built atop whatever Phase 4 defines; "be translatable from BSD sockets" is the only constraint on Phase 4's design imposed by this ADR. No specific Phase 4 design choice is mandated.

### Phase 5 (AI subsystem foundation — not yet opened)

- **No constraint on Phase 5.** The AI subsystem syscalls (0x100..0x1FF) remain NeoPlutarch's alone. Personality processes can access AI primitives via native-ABI translation paths the personality layer constructs in Phase 8; that does not constrain Phase 5's design.

### Phase 6 (accelerator drivers — not yet opened)

- **Phase 6 design hook:** accelerator drivers per ANSPEC-001 §9 must be designable such that a Linux-DRM-compatible character-device shim can be added in Phase 9 (graphics stack) without redesigning the driver internals. This is a minor surface-area constraint, not a structural redirect. ANSPEC-001 §9 may need an amendment at Phase 6 implementation time to make the constraint explicit; this ADR records the intent now so Phase 6 design is informed.

### Phase 8 (userland — not yet opened)

This is where the personality lands. Specific implementation items, captured here as a checklist for the eventual Phase 8 THINK:

- [ ] **PERSPEC-001 drafted** — Linux Personality Specification, the implementation contract for the personality (analogous to ANSPEC-001 for the AI subsystem).
- [ ] **`Process` struct gains `personality: Personality` field** — enum, initially `Native` + `Linux`. One byte per process. Phase 8 retrofit.
- [ ] **Syscall entry path reads personality flag and selects dispatch table** — `Linux` routes to `kernel/src/personality/linux/dispatch.rs`; `Native` routes to existing native dispatch.
- [ ] **CI gains a `personality-stub` build target** — ensures `kernel/src/personality/linux/` remains compilable as it grows from empty stub.
- [ ] **`kernel/src/personality/linux/dispatch.rs`** — Linux syscall dispatch table.
- [ ] **`kernel/src/personality/linux/syscalls/`** — Tier 1 syscall implementations.
- [ ] **`kernel/src/personality/linux/fork.rs`** — fork emulation with COW.
- [ ] **`kernel/src/personality/linux/signals.rs`** — Linux signal semantics.
- [ ] **`kernel/src/personality/linux/devfs.rs`** — `/dev` static population.
- [ ] **`userland/neo-procfsd/`** — `/proc` emulation daemon.
- [ ] **`userland/neo-sysfsd/`** — `/sys` emulation daemon.
- [ ] **Native syscall number non-collision audit** — verify native syscall allocations 0x000..0x0FF do not collide with eventual Linux personality dispatch numbers.
- [ ] **Boot verification** — Linux distro chroot starts, `bash` runs, `gcc` compiles a hello-world, `python3 -c 'print("hi")'` works.
- [ ] **LTP subset passes** — Linux Test Project syscall-correctness tests for Tier 1 surface, minus tests exercising explicitly-excluded features.

### Phase 9+ (graphics stack — not yet opened)

- [ ] Linux-DRM-compat character device shim (`kernel/src/personality/linux/drm.rs`).
- [ ] Mesa port or equivalent Vulkan stack on top of ANSPEC-001 accelerators.
- [ ] Audio personality (PulseAudio / PipeWire shim).
- [ ] Input device personality (`/dev/input/event*` backed by Phase 4 USB HID).
- [ ] Proton verification: a representative Windows game launches and renders.

Each of these items above earns its own ADR when its phase planning opens. This ADR's commitment is the architectural direction, not the implementation detail of any Phase 9+ component.

---

## Open questions

Following ANSPEC-001 §13's pattern: deferred items, named so they're not rediscovered.

- **Q-Open-1.** Should the personality be split into "Linux Tier 1 personality" and "Linux Tier 2 personality" as separately-buildable Cargo features, or a single `linux` feature with all tiers? Resolution: at Phase 8 THINK, when build-graph complexity is concrete.
- **Q-Open-2.** Does the personality require its own per-personality syscall numbering or does it just translate Linux numbers directly? Resolution: at Phase 8 THINK; direct translation is the FreeBSD pattern and is likely correct.
- **Q-Open-3.** How does the personality interact with NeoPlutarch's capability-derived FD model when Linux apps pass FDs via `SCM_RIGHTS`? Resolution: Phase 8 THINK; design likely maps `SCM_RIGHTS` to ANSPEC-002 capability-passing IPC.
- **Q-Open-4.** Does the Linux personality need to honor NeoFS's typed-inode discriminant (Bible Article 10.3)? If a Linux app tries to `mmap` a `.neom` file as `PROT_EXEC`, the kernel refuses; does the personality translate that refusal to `EACCES`, `ETXTBSY`, or something else? Resolution: Phase 8 THINK; likely `EACCES` with a personality-specific log entry.
- **Q-Open-5.** Should the personality expose a Linux `KernelAttestation`-equivalent so Linux software with TPM/DICE attestation needs runs cleanly? (ANSPEC-001 §13 Q6.) Resolution: Phase 8+ if and when ANSPEC-001 §13 Q6 itself resolves; not a Phase 8 launch blocker.

---

## Bible amendment co-ratified with this ADR

This ADR is paired with `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-21-articles-3-and-5-6-and-28-2-personality-subsystems.md`. The amendment:

- **Edits Article 3 fourth bullet** to point at Article 5.6 for the foreign-OS-compatibility mechanism.
- **Adds Article 5.6** formalizing personality subsystems.
- **Updates Article 28.2's cross-reference** so the accessibility-goal article points at the mechanism article.
- **Bumps Bible version** from v1.6.0 to v1.7.0.

Both artifacts are co-ratified; neither lands without the other. The Bible amendment cannot stand without the ADR (the mechanism it formalizes lives here); the ADR cannot stand without the Bible amendment (the mechanism is permitted by current Bible language but is not *formally established* until the amendment lands).

---

**Document end.**

*Per Bible Article 18.1 this ADR is the architectural record for foreign-OS binary compatibility on NeoPlutarch. Per Article 27 ratification status is set by PL signoff. PM recommendation was Option A (forward-marker shape); PL chose Option B (full ADR + Bible amendment); this ADR is the Option B work product.*
