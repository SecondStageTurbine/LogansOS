<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Arthur userland arc and architecture — from inference-complete to usable assistant

**Date:** 2026-06-16
**Author:** Prometheus (design lane)
**Branch:** `prometheus/arthur-userland-arc-design`
**Status:** DRAFT — for PM/PL ratification
**Related:** Bible Article 29 (Arthur), Article 28 (Accessibility), Article 23 (What We Will Not Do), Article 6.6 (Inference governance seam), VISION doc, DISPATCH-2026-06-16-arthur-userland-arc-prep-prometheus.md

---

## 1. EXISTS-vs-MISSING substrate map

### 1A. Process creation + ELF loader — EXISTS

| Component | Evidence |
|-----------|----------|
| ELF parser/loader | `kernel/src/loader/elf.rs` — full ELF64 parse, segment mapping, entry-point extraction |
| User process spawn | `kernel/src/arch/x86_64/syscall.rs:59,619-709` — `SYS_SPAWN` loads by Limine-module name, installs handles in child |
| Per-process PML4 | `kernel/src/sched/process.rs:228-301` — `Process::new_user()` with own address space |
| User stack allocation | `kernel/src/loader/elf.rs` — `allocate_user_stack()` |

### 1B. IPC — EXISTS (rendezvous channels)

| Component | Evidence |
|-----------|----------|
| Sync rendezvous channel | `kernel/src/ipc/mod.rs` — blocking send/recv with yield-on-block |
| Channel create | `syscall.rs:60,716-749` — `SYS_CHAN_CREATE`, installs both endpoints in caller's table |
| Send / Recv | `syscall.rs:61-62,2122-2158` — `SYS_SEND`/`SYS_RECV` with user-buffer translation |
| Capability grant over IPC | `syscall.rs:67-68,2680-2723` — `SYS_GRANT`/`SYS_RECV_GRANT`, cross-process capability transfer |

### 1C. Capability system — EXISTS (full Phase 2 implementation)

| Component | Evidence |
|-----------|----------|
| Handle enum (8 variants) | `process.rs:427-479` — Channel, Process, File, Socket, TcpSocket, TcpListener, Tensor, Model |
| CapRights (meta + semantic) | `process.rs:83-112` — subset check, intersection, per-bit has_* |
| Meta rights | `process.rs:114-119` — USE, DERIVE, GRANT, REVOKE |
| Derive (restricted child cap) | `syscall.rs:65,2662-2671` — `SYS_DERIVE`, rights must be subset of parent |
| Revoke (cascade walk) | `syscall.rs:66,2673-2678` — `SYS_REVOKE`, walks derivation log |
| Grant / recv_grant | `syscall.rs:67-68,2680-2723` — cross-process via IPC channel |
| Per-process handle table + derivation log | `process.rs:238-244` — `SwitchMutex<HandleTable>`, `BTreeMap<CapId, Option<CapId>>` |
| CapId (global unique) | `process.rs:66-80` — atomic allocator, per-handle identity |

### 1D. AI/Inference syscalls — EXISTS (Phase 5-7 complete)

| Component | Evidence |
|-----------|----------|
| Tensor primitive (L0) | `syscall.rs:79-86,932-1128` — create/query/share/drop/write, syscalls 0x100-0x104 |
| Model object (L1) | `syscall.rs:87-89,1171-1265` — load/query/drop from `.neom`, syscalls 0x110-0x112 |
| Inference execution (L2) | `syscall.rs:91,1278-1416` — `SYS_INFER_RUN` with admission scheduler |
| AccelDriver trait + dispatch | `kernel/src/drivers/accel/` — Phase 6 complete, multi-device residency |
| Inference scheduler (Phase 7) | Admission control, per-tenant accounting, spatial isolation |

### 1E. Filesystem — EXISTS (VFS + backends)

| Component | Evidence |
|-----------|----------|
| VFS layer | `kernel/src/fs/mod.rs` — OpenFile abstraction, path namespace |
| Open / Read / Close / Fsync | `syscall.rs:69-72,2188-2455` — syscalls 0xC-0xF |
| Readdir | `syscall.rs:73,2244-2275` — cursor-based directory iteration |
| Mount / Umount | `syscall.rs:74-75,2374-2394` — NeoFS, FAT32-ro, ext4-ro backends |
| FileRights (semantic bits) | `libneo/lib.rs:349-376` — READ, WRITE, LOOKUP, READDIR, EXECUTE, MOUNT |

### 1F. Networking — EXISTS (full Phase 4 stack)

| Component | Evidence |
|-----------|----------|
| UDP sockets | `syscall.rs:92-99,1471-1926` — create/bind/send/recv/sendto/recvfrom/close |
| TCP stack | `syscall.rs:100-103,1491-1632` — connect/listen/accept/shutdown |
| DNS resolver | `syscall.rs:104,1634-1673` — `SYS_NET_RESOLVE` |
| Happy Eyeballs dial | `syscall.rs:105,1675-1709` — `SYS_NET_DIAL` |
| kTLS | `syscall.rs:106,1711-1730` — `SYS_NET_TCP_ENABLE_TLS` |

### 1G. Terminal / Console — PARTIAL (output only)

| Component | Evidence | Status |
|-----------|----------|--------|
| Serial console output | `kernel/src/drivers/console/serial.rs:1-78` — 16550 UART, `write_bytes()` for sys_write(fd=1) | EXISTS |
| UEFI framebuffer | Phase 0 complete | EXISTS (display only) |
| Keyboard input driver | Bible 9.7 — USB HID from Phase 4, PS/2 graceful degradation | PARTIAL — driver may exist but no line discipline |
| TTY / line discipline | Bible `console/mod.rs:5` — "once a window system arrives" | MISSING |
| Userland shell (`neo-shell`) | Bible Article 25.2 names it as convention; no binary exists | MISSING |

### 1G. Userland binaries — MINIMAL

| Component | Evidence | Status |
|-----------|----------|--------|
| `neo-init` (PID 1) | `userland/neo-init/src/main.rs` — test-scenario runner, not a supervisor | EXISTS (test harness) |
| `neo-worker` | `userland/neo-worker/src/main.rs` — IPC peer for test scenarios | EXISTS (test harness) |
| `libneo` | `userland/libneo/src/lib.rs:1-1425` — syscall wrappers, heap allocator, handle types | EXISTS (complete runtime) |
| Shell / REPL | — | MISSING |
| Package manager | — | MISSING |
| Arthur process | — | MISSING |

### 1H. Summary of gaps for Arthur

| Gap | Blocking for Arthur? | Notes |
|-----|---------------------|-------|
| **No interactive input surface** (keyboard → line discipline → process stdin) | YES | Arthur needs to receive user commands |
| **No shell / command surface** to invoke Arthur | YES | Something must spawn and talk to the Arthur process |
| **No capability-bounded action surface** (structured way for Arthur to effect changes) | YES | This is Article 29.3's security spine |
| **No structured IPC protocol** between user input → Arthur → actions | YES | Need a message format for NL commands and action results |
| Inference decode loop (8.1-8.5) | PARTIAL | Phase 8 inference arc; Arthur can't converse without it, but this dispatch designs around its completion |

---

## 2. The shell/command surface (arc 2a) — minimal-sufficient design

### 2A. What Article 28.2 requires

> "Boot, log in, type, point. A user with a fresh LogansOS install on supported hardware can power on, see a working display, use their keyboard and mouse or touchpad, and reach a working userspace shell..." (Bible:337)

### 2B. Design alternatives

**Option 1: Kernel-adjacent REPL.** A kernel-mode loop that reads keyboard input, echoes to framebuffer/serial, and dispatches built-in commands. This violates Article 6.6 (kernel exposes primitives, policy is userspace's) and Article 23.3 (kernel stays model-agnostic). Also creates a permanent kernel surface that can't be replaced.

**Option 2: Full POSIX shell.** Line editing (readline), job control, pipes, redirection, environment variables, builtins, history. This is the wrong shape for LogansOS — we're not Linux, and Arthur makes most shell conveniences unnecessary. The lean-patch rule (Article 15.7) rejects it.

**Option 3: neo-shell — minimal capability-aware REPL (RECOMMENDED).** A userland process that:
- Reads lines from a keyboard/serial input source
- Echoes to stdout (framebuffer or serial)
- Parses a minimal command syntax: `command [args...]`
- Spawns child processes via `sys_spawn`, granting them scoped capabilities
- Waits for children, reports exit status
- Has NO builtins beyond `spawn`, `grant`, `revoke`, `inspect`, `exit`

This is the smallest sufficient thing. It satisfies Article 28.2's floor ("reach a working userspace shell") and provides the invocation surface Arthur needs. The user types at neo-shell; neo-shell spawns Arthur with capability-bounded handles.

### 2C. neo-shell architecture

```
neo-init (PID 1)
  └── sys_spawn("shell", [keyboard_cap, display_cap])
        └── neo-shell
              ├── reads line from keyboard_cap
              ├── parses: "arthur" → sys_spawn("arthur", [model_cap, fs_caps, net_caps])
              ├── waits for Arthur's response via IPC channel
              └── prints result to display_cap
```

**Input path:** neo-shell needs a keyboard input source. Two approaches:
- **Serial stdin (Phase 8a interim):** Read from COM1 serial port. Works today with existing UART driver. Satisfies "type" for development and headless use.
- **Keyboard + framebuffer (Phase 8b):** USB HID keyboard driver (Bible 9.7, Phase 4 target) feeds a userland input daemon; neo-shell reads from it via IPC.

**Recommendation:** Start with serial stdin for Phase 8a. It's sufficient for Arthur to be usable and testable. Keyboard + framebuffer is a quality-of-life follow-up, not an Arthur blocker.

### 2D. neo-shell command surface (v1)

```
> arthur                          # spawn Arthur, connect via IPC, relay conversation
> spawn <name> [handle_indices]   # raw process spawn
> grant <channel> <handle>        # send capability over IPC
> revoke <handle>                 # revoke a capability
> inspect <process_handle>        # query process state
> exit                            # terminate shell
```

One command matters: `arthur`. The rest are capability plumbing for debugging and manual authority management. The `arthur` command is special-cased in neo-shell — it spawns the Arthur process with a pre-configured capability bundle (described in Section 3) and establishes an IPC relay loop.

---

## 3. The capability-bounded action surface (arc 2b) — the security spine

### 3A. Article 29.3 constraint

> "Arthur's authority is bounded by the kernel's capability system by construction — it can reach only what it has been granted, enforced by the same mechanism that governs every other principal." (Bible:352)

### 3B. The principle: model proposes, capability disposes

Arthur's inference loop emits structured intent (e.g., JSON tool-call tokens). These intents are **proposals**. The capability system is the **disposition** layer. Every action Arthur attempts goes through a capability gate:

```
User NL input → tokenizer → model inference → token stream
  → intent parser extracts {action, arguments}
  → Arthur process looks up required capability for action
  → if cap exists and has sufficient rights → execute via syscall
  → if cap missing or insufficient → deny, report to user
```

The model never calls syscalls directly. The Arthur runtime parses the model's output into structured intents, then executes them through its own capability-checked code paths. This is the "model proposes, capability disposes" invariant.

### 3C. Action surface — v1 action catalog

Each action maps to existing syscalls + required capabilities:

| Action | What Arthur does | Syscall(s) | Required capability | Rights needed |
|--------|-----------------|------------|-------------------|---------------|
| **read_file** | Read file contents | `SYS_OPEN` + `SYS_READ` + `SYS_CLOSE` | File handle (granted by neo-shell at spawn) | `FileRights::READ` |
| **write_file** | Write/append to file | `SYS_OPEN` + `SYS_WRITE`(via tensor? — see note) + `SYS_CLOSE` | File handle | `FileRights::WRITE` |
| **list_dir** | List directory entries | `SYS_READDIR` | File handle (directory) | `FileRights::READDIR` |
| **spawn_process** | Launch another process | `SYS_SPAWN` | — (Arthur can always spawn; child gets NO caps unless Arthur grants them) | N/A — spawn is ambient, child is capability-bounded |
| **kill_process** | Terminate a process Arthur spawned | `SYS_KILL` | ProcessHandle (from spawn return) | `process_rights::KILL` |
| **inspect_process** | Query process state | `SYS_INSPECT` | ProcessHandle | `process_rights::INSPECT` |
| **wait_process** | Wait for child exit code | `SYS_WAIT` | ProcessHandle | `process_rights::WAIT` |
| **net_dial** | Connect to remote host | `SYS_NET_DIAL` | TcpSocketHandle (granted) or create via `SYS_SOCKET_CREATE` | `socket_rights::READ \| WRITE` |
| **net_send** | Send data over TCP | `SYS_SOCKET_SEND` on TcpSocket | TcpSocketHandle | `socket_rights::WRITE` |
| **net_recv** | Receive data from TCP | `SYS_SOCKET_RECV` on TcpSocket | TcpSocketHandle | `socket_rights::READ` |
| **infer** | Run model inference | `SYS_INFER_RUN` | ModelHandle + TensorHandles | `model_rights::USE`, `tensor_rights::USE` |
| **load_model** | Load a .neom model | `SYS_MODEL_LOAD` | — (ambient; loaded model is a new capability) | N/A |

**Note on file write:** Currently `SYS_WRITE` (0x1) only supports fd=1 (serial stdout). A new syscall or extension of `SYS_WRITE` to accept a `FileHandle` is needed for Arthur to write arbitrary files. This is a small kernel addition — the VFS layer already supports write operations; the syscall just needs to route to it.

### 3D. Capability bundling at spawn

When neo-shell spawns Arthur, it grants a **scoped capability bundle**:

```
neo_shell_spawn_arthur():
    // 1. Create IPC channel for conversation relay
    (shell_end, arthur_end) = sys_chan_create()

    // 2. Open filesystem root with restricted rights
    fs_root = sys_open("/", OPEN_ROOT_TEMPORARY, FileRights::READ | FileRights::READDIR)

    // 3. Derive a read-only subset (no WRITE, no MOUNT, no EXECUTE)
    arthur_fs = sys_derive(fs_root, CapRights{meta: USE, semantic: 0})
    // FileRights are separate from CapRights — derive on the File variant
    // produces a new FileHandle with intersected FileRights

    // 4. Grant Arthur model capability (if a default model is configured)
    model_cap = sys_open_model("/neofs/arthur.neom")

    // 5. Spawn with handle bundle
    handles = [arthur_end, arthur_fs, model_cap]
    (pid, proc_cap) = sys_spawn("arthur", handles)

    // 6. Arthur receives handles at indices 0, 1, 2
    //    Handle(0) = conversation channel
    //    Handle(1) = filesystem root (READ+READDIR only)
    //    Handle(2) = default model
```

**Key security property:** Arthur starts with ONLY what neo-shell grants. It can't access files it wasn't given handles for. It can spawn children, but those children inherit nothing unless Arthur explicitly grants them capabilities. The capability system's derivation log ensures revoked caps cascade to all derivatives.

### 3E. Capability escalation — how Arthur gets more authority

Arthur may need additional capabilities during operation (e.g., write access to a specific file, network access). Two mechanisms:

**Mechanism 1: Pre-grant at spawn.** neo-shell grants a broader set of capabilities upfront. This is simpler but less secure — Arthur has more authority than it immediately needs.

**Mechanism 2: On-demand grant via IPC.** Arthur requests a capability through the conversation channel; neo-shell (or a supervising policy process) evaluates the request and grants it via `SYS_GRANT`. This is the "model proposes, capability disposes" pattern at the system level: Arthur's model says "I need write access to /home/logan/report.txt"; the shell/policy decides whether to grant it.

**Recommendation:** Mechanism 2 for v1. It demonstrates the security spine working and gives the user visibility into what Arthur is authorized to do. The conversation channel carries both NL messages AND capability grant requests/responses.

---

## 4. Arthur's userland app architecture

### 4A. Process model

Arthur is a single userland ELF binary (`neo-arthur`) that links `libneo`. It runs as one process with one thread (initially; multi-threading for concurrent inference + I/O is a Phase 9 optimization).

```
neo-arthur (userland process)
├── Model layer: loads .neom via SYS_MODEL_LOAD, holds ModelHandle
├── Inference layer: constructs input tensors, calls SYS_INFER_RUN, reads output tensors
├── Intent parser: parses model output tokens → structured {action, args} intents
├── Action executor: maps intent → capability-checked syscall sequence
├── Conversation relay: reads user input from Handle(0), writes responses to Handle(0)
└── Context manager: maintains conversation history as a rolling tensor buffer
```

### 4B. The NL→intent→capability-checked-action loop

```
LOOP:
    1. READ: sys_recv(conversation_chan, &mut input_buf)
       → user's natural-language command (from neo-shell relay)

    2. TOKENIZE: convert input bytes → token IDs (Phase 8.3 tokenizer)
       → input tensor for the model

    3. INFER: sys_infer_run(model_handle, [input_tensor], &mut [output_tensor], 0)
       → model emits tokens autoregressively (Phase 8.1 decode loop)

    4. DETECT: scan output tokens for intent markers
       → if pure text response: goto STEP 5a
       → if tool-call/intent detected: goto STEP 5b

    5a. RESPOND (text-only):
        sys_send(conversation_chan, response_bytes)
        goto LOOP

    5b. ACT (intent + action):
        intent = parse_intent(output_tokens)  // e.g., {action: "read_file", path: "/neofs/config.neom"}
        result = execute_action(intent)       // capability-checked execution
        response = format_result(result)      // "I read the file: ..." or "Denied: no write cap for /neofs/secret"
        sys_send(conversation_chan, response_bytes)
        goto LOOP

execute_action(intent):
    match intent.action:
        "read_file"  → open_file(intent.path, READ)   → if EBADF/EPERM → return DENIED
        "write_file" → open_file(intent.path, WRITE)   → if EBADF/EPERM → return DENIED
        "list_dir"   → readdir(intent.path)            → if EBADF/EPERM → return DENIED
        "spawn"      → sys_spawn(intent.name, [])      → always succeeds (child gets no caps)
        "net_dial"   → sys_net_dial(intent.host, port) → if no net cap → return DENIED
        ...
```

### 4C. Intent format — model output structure

The model must be trained/fine-tuned to emit structured intents in its token stream. Two approaches:

**Approach A: Delimited tool-call tokens.** The model emits special tokens (e.g., `<tool>read_file</tool><arg>path=/neofs/config.neom</arg>`) that the intent parser recognizes. This is the Claude/GPT tool-calling pattern adapted to raw token streams.

**Approach B: JSON-in-context.** The model emits a JSON object as part of its text response. The intent parser extracts the JSON, validates its schema, and dispatches. Simpler to implement; less reliable (model may emit malformed JSON).

**Recommendation:** Approach A for v1. It's more robust — the token stream is easier to parse than free-form JSON, and the model can be trained with explicit tool-call tokens. The tokenizer (Phase 8.3) registers special tokens for each action type.

### 4D. Conversation context management

Arthur needs to maintain conversation history. In the kernel's tensor model:

- **Context buffer:** A mutable tensor holding the rolling token window (e.g., last 4096 tokens). This is a `TensorHandle` with `tensor_rights::USE | tensor_rights::WRITE`.
- **KV cache:** Managed by the kernel's residency governance (Phase 8.2). The KV cache IS governed tensors — exactly what Phase 7 was built for.
- **Context overflow:** When the token window exceeds the model's context length, Arthur truncates the oldest tokens. Simple sliding window; no summarization in v1.

### 4E. Model hot-swap (ADR-049)

Arthur supports live model replacement without process restart:
1. neo-shell (or user command) loads a new `.neom` via `SYS_MODEL_LOAD`
2. The new `ModelHandle` is granted to Arthur via `SYS_GRANT` on the conversation channel
3. Arthur receives the grant, swaps the model handle, and continues inference with the new model
4. The old model's refcount drops; kernel reclaims its weight tensors

This rides on existing capability + model infrastructure. No new syscalls needed.

---

## 5. Constitutional constraints — design compliance

| Constraint | How the design honors it |
|-----------|-------------------------|
| **Userland-only (6.6, 23.1)** | Arthur is `neo-arthur`, a userland ELF binary. The kernel knows nothing about "Arthur" — it only sees a process calling syscalls. Governance hooks (LogitPort, ANSPEC-002) are exposed but unused by default; Arthur's safety is capability-bounded, not kernel-filtered. |
| **Model-agnostic kernel (23.3)** | The kernel loads `.neom` files and executes tensor-op graphs. It doesn't know the graph represents an LLM, a classifier, or a diffusion model. Arthur's "personality" is entirely in the model file it loads. |
| **No telemetry (23.2)** | Arthur runs on-device. Inference stays local. No network syscalls are used for telemetry — only for user-requested actions (web fetch, etc.). The design has no telemetry surface to accidentally enable. |
| **Friendly, practical character (29.1)** | This is a model-training concern, not an architecture concern. The capability-bounded action surface ensures Arthur is practical (can do useful things) while the denial path ("Denied: no cap") keeps it honest about its limits. |

---

## 6. Build slicing — Phase 8 arc 2 + Arthur sequence

Each slice is kill-switchable, independently verifiable, and gated on real evidence.

### Slice 1: neo-shell (minimal REPL)
**Goal:** A userland process that reads lines from serial stdin, echoes to stdout, spawns children, and relays IPC.
**New code:** `userland/neo-shell/` — new binary crate (~400 LOC)
**Kernel changes:** None (uses existing syscalls: spawn, chan_create, send, recv, write, wait, exit)
**Input dependency:** Serial input driver (read from COM1). May need a new syscall or extension of the UART driver to support userland reads. **OPEN DECISION:** Does the serial driver need a `SYS_READ` path for fd=0 (stdin), or does neo-shell read from a kernel-managed input channel?
**Gate:** Boot → see shell prompt → type "exit" → shell exits → init continues. Serial I/O round-trip verified.
**Kill switch:** If serial stdin doesn't work, fall back to IPC-only mode where neo-init sends commands over a pre-established channel.

### Slice 2: Capability-bounded action surface (libneo extension)
**Goal:** Type-safe wrappers in libneo for Arthur's action catalog, with capability rights checking at the wrapper level.
**New code:** `userland/libneo/src/action.rs` — action enum, execute_action() dispatcher (~300 LOC)
**Kernel changes:** Possibly extend `SYS_WRITE` to accept FileHandle (currently fd=1 only). Small, contained change.
**Gate:** Unit test in neo-init: spawn a process with a read-only file cap; verify it can read but not write. Test each action type's grant/deny path.
**Kill switch:** If SYS_WRITE extension is risky, Arthur uses tensor-based file I/O (create tensor → copy data → sys_tensor_create with data ptr).

### Slice 3: neo-arthur skeleton (inference relay)
**Goal:** Arthur process that loads a model, receives text via IPC, runs inference, sends response back. NO action execution yet — pure conversation relay.
**New code:** `userland/neo-arthur/` — new binary crate (~600 LOC)
**Dependencies:** Phase 8.1-8.5 complete (decode loop, KV cache, tokenizer, quantization, real-model serving)
**Gate:** Boot → shell spawns arthur → type "hello" → Arthur responds with model-generated text. End-to-end NL conversation verified.
**Kill switch:** If inference isn't ready, Arthur echoes input back (dummy mode) to verify the IPC relay + shell integration.

### Slice 4: Intent parsing + action execution
**Goal:** Arthur parses tool-call tokens from model output and executes capability-bounded actions.
**New code:** Intent parser in neo-arthur (~400 LOC), action executor wired to libneo action surface
**Dependencies:** Slice 2 (action surface), Slice 3 (inference relay)
**Gate:** Boot → ask Arthur "read /neofs/config.neom" → Arthur reads file and reports contents. Ask Arthur "write /neofs/secret.txt" → Arthur reports "Denied: no write capability." Both paths verified.
**Kill switch:** If intent parsing is unreliable, fall back to JSON-in-context (Approach B) with manual validation.

### Slice 5: On-demand capability escalation
**Goal:** Arthur requests capabilities via IPC; neo-shell grants them after user confirmation.
**New code:** Grant request/response protocol on conversation channel (~200 LOC in both shell and Arthur)
**Dependencies:** Slice 4
**Gate:** Boot → ask Arthur to write a file → shell prompts "Allow arthur to write /neofs/report.txt? [y/N]" → user types "y" → shell grants cap via SYS_GRANT → Arthur succeeds.
**Kill switch:** Pre-grant all needed caps at spawn (Mechanism 1 from Section 3E). Less secure but functional.

---

## 7. Open decisions for PM/PL ratification

### OD-1: Serial stdin vs IPC-only shell input
**Question:** How does neo-shell receive user input? The UART driver currently only supports output (`write_bytes`). Do we add a `SYS_READ` path on fd=0 (serial stdin), or does neo-init relay keyboard/input events over IPC to neo-shell?
**Options:**
- A: Add serial read to the UART driver, expose as fd=0 in libneo. Simple, works for headless + QEMU. (~50 LOC kernel change)
- B: IPC-only — neo-init reads from input source and forwards lines to neo-shell over a channel. No kernel changes but adds indirection.
**Recommendation:** A. Serial stdin is the simplest path to "type at the machine." The UART 16550 already has a receive FIFO; exposing it as a readable fd is mechanically straightforward.

### OD-2: Arthur's file write surface
**Question:** `SYS_WRITE` currently only supports fd=1 (serial stdout). Arthur needs to write files. Do we extend SYS_WRITE to accept FileHandle, or add a new syscall?
**Options:**
- A: Extend `SYS_WRITE` to dispatch on handle type (fd=1 → serial; FileHandle → VFS write). Minimal ABI change.
- B: Add `SYS_FILE_WRITE(handle, buf, len)` as a new syscall. Clean separation, no risk to existing SYS_WRITE callers.
**Recommendation:** A. The VFS layer already supports write on OpenFile; the syscall just needs a dispatch case. Low risk, one place to change.

### OD-3: Intent format — delimited tokens vs JSON-in-context
**Question:** How does Arthur's model emit structured action intents?
**Options:**
- A: Special tool-call tokens in the tokenizer vocabulary (Approach A above). More robust parsing, requires tokenizer training data with tool tokens.
- B: JSON objects in the text output (Approach B). Easier to implement immediately; less reliable parsing.
**Recommendation:** A for v1 if the tokenizer supports custom tokens. B as interim if Phase 8.3 tokenizer doesn't support special tokens initially.

### OD-4: Capability escalation — pre-grant vs on-demand
**Question:** Does Arthur get a broad capability bundle at spawn, or request caps on-demand through the conversation channel?
**Options:**
- A: Pre-grant broad caps at spawn. Simpler, but Arthur has more authority than needed.
- B: On-demand grants via IPC with user confirmation. Demonstrates the security spine; gives user visibility.
**Recommendation:** B for v1. It's the design that proves Article 29.3 works. The conversation is the authorization surface — the user sees what Arthur wants to do and approves or denies it.

### OD-5: neo-shell as Arthur's parent vs dedicated supervisor
**Question:** Does neo-shell directly manage Arthur (spawn, grant, relay), or is there a separate supervisor process?
**Options:**
- A: neo-shell is the supervisor. It spawns Arthur, manages caps, and relays conversation. Simple, but conflates shell and supervisor concerns.
- B: Dedicated `neo-supervisor` process. neo-shell spawns the supervisor; supervisor manages Arthur. Cleaner separation, more processes.
**Recommendation:** A for v1. neo-shell IS the supervisor for Arthur. If we later add multi-agent support (multiple assistants), we'd extract the supervisor. Rule 3 (smallest sufficient thing) says don't pre-build for multi-agent.

### OD-6: Conversation context — tensor buffer vs ring buffer in userspace
**Question:** How does Arthur maintain conversation history? As a kernel-managed mutable tensor, or as a userspace `Vec<u32>` that's re-encoded into tensors each inference call?
**Options:**
- A: Mutable tensor via `SYS_TENSOR_CREATE(mutable=true)` + `SYS_TENSOR_WRITE`. Kernel tracks the context buffer; efficient for KV cache integration.
- B: Userspace `Vec<u32>`. Re-create input tensor each turn. Simpler, no kernel tensor state to manage.
**Recommendation:** B for v1. The context window is small enough (a few thousand token IDs) that recreating the tensor each turn is negligible overhead. A becomes relevant at scale (long contexts, frequent partial updates).

### OD-7: Arthur's model loading — ambient vs capability-gated
**Question:** Can Arthur load any `.neom` file it can reach on disk, or does it need explicit authorization to load models?
**Options:**
- A: Ambient — `SYS_MODEL_LOAD` doesn't check a special capability; Arthur loads any model it has filesystem read access to.
- B: Gated — add a new capability right (`model_rights::LOAD`) that must be explicitly granted. Restricts which models Arthur can activate.
**Recommendation:** A for v1. The filesystem capability already bounds what Arthur can reach. Adding a separate model-load cap is defense-in-depth but not necessary if the FS cap is properly scoped. Revisit if multi-tenant model isolation becomes a concern.

---

## 8. Risk assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| Phase 8.1-8.5 inference arc delays Arthur's conversation capability | HIGH | BLOCKS | Slice 3 has a dummy-mode kill-switch (echo relay) to verify IPC + shell integration without inference |
| Serial stdin driver work is more complex than expected | MEDIUM | DELAYS Slice 1 | IPC-only fallback (OD-1 Option B) requires zero kernel changes |
| Intent parsing from model output is unreliable | MEDIUM | DEGRADES UX | JSON-in-context fallback (OD-3 Option B); manual validation before execution |
| SYS_WRITE extension introduces regressions | LOW | KERNEL BUG | Isolated to one dispatch case; existing neo-init test scenarios cover SYS_WRITE(fd=1) as regression guard |
| Capability grant protocol on conversation channel creates message ambiguity | MEDIUM | COMPLEXITY | Use message type tags: first byte distinguishes NL text from cap-grant request/response |

---

## 9. Summary

The path from inference-complete to usable Arthur is **additive layers on proven substrate** — no rework, no kernel model embedding, no telemetry surface. The four gaps are: (1) a minimal shell to invoke Arthur, (2) a capability-bounded action surface for Arthur to act safely, (3) the Arthur process itself (NL→inference→response relay), and (4) intent parsing + action execution. Each is independently buildable and kill-switchable. The security spine — "model proposes, capability disposes" — is structural: Arthur can only reach what neo-shell grants it, enforced by the same Phase-2 capability system that governs every other process. The constitutional constraints (userland-only, model-agnostic kernel, no telemetry) are honored by construction: Arthur is a userland ELF binary calling existing syscalls through capability-bounded handles.

---

## 10. PM/PL ratification (2026-06-16)

PM verified the load-bearing EXISTS/MISSING claims against the actual tree (the ones that drive *kernel* changes), then PL ratified the open decisions. This section is authoritative; where it differs from §2–§7 above, this section governs.

### 10.1 PM factual correction — OD-2 is MOOT (file write already exists)

§3C ("Note on file write") and **OD-2** assert that `SYS_WRITE` only supports fd=1 and that Arthur needs a new syscall or a `SYS_WRITE` extension to write files. **This is wrong.** `sys_write_impl` (`kernel/src/arch/x86_64/syscall.rs:2168`) already dispatches non-stdout fds to `sys_file_write_impl` (`syscall.rs:2406`), which performs a **capability-checked `FileRights::WRITE` VFS write** today. So:

- **OD-2 collapses** — there is no decision to make; the `write_file` action works on the existing ABI.
- **Slice 2 loses its only kernel change.** §6 Slice 2's "Possibly extend `SYS_WRITE` to accept FileHandle" and its kill-switch (tensor-based file I/O) are deleted as unnecessary; Slice 2 becomes pure-userland `libneo` wrappers over existing syscalls.
- The §3C catalog row for `write_file` already maps to `SYS_OPEN`+`SYS_WRITE(FileHandle)`+`SYS_CLOSE` with no kernel work.

This is exactly the error the "ground every claim in real code, no memory" rule exists to catch — and it makes the plan *simpler*, not harder.

### 10.2 Ratified open decisions (PL, 2026-06-16)

| OD | Ratified | Rationale |
|----|----------|-----------|
| **OD-1** input | **A — serial stdin** (`SYS_READ`/fd=0 path on the 16550 RX FIFO, ~50 LOC) | simplest "type at the machine"; works headless + in QEMU. **The arc's one and only kernel change.** |
| **OD-2** file write | **MOOT** | already exists (§10.1); no decision, no kernel change. |
| **OD-3** intent format | **A — delimited tool-call tokens** (B as interim if 8.2's tokenizer can't register special tokens yet) | token-stream parse is more robust than free-form JSON. |
| **OD-4** cap escalation | **B — on-demand grant via IPC with user confirmation** | this is the build that *proves* Article 29.3; the conversation channel becomes the authorization surface — the security spine made visible. |
| **OD-5** supervisor | **A — neo-shell IS Arthur's supervisor** | Rule-3 smallest-sufficient; extract a dedicated supervisor only if multi-agent later lands. |
| **OD-6** context store | **B — userspace `Vec<u32>`, re-encode per turn** | a few thousand token-ids; kernel-tensor context is a scale optimization for later. |
| **OD-7** model load | **A — ambient, bounded by the FS capability** | the filesystem cap already bounds reachable models; a separate `model_rights::LOAD` is defense-in-depth not needed at v1. |

### 10.3 Locked build slicing + sequencing

The 5-slice arc (§6) is ratified **with these amendments**:
- **Slice 1 (neo-shell):** carries the OD-1 serial-stdin `SYS_READ`/fd=0 kernel addition — **the arc's only kernel-touching slice** (⇒ pre-merge QEMU gate when built).
- **Slice 2 (action surface):** **pure userland** — `libneo` action wrappers over existing syscalls; **no kernel change** (OD-2 moot). Kill-switch is the per-action deny path, not a syscall fallback.
- **Slices 3–5:** unchanged; Slice 3 (the Arthur process) depends on **8.5** (real-model generation).
- **Intent format:** OD-3=A (special tokens), so the 8.2 BPE tokenizer's special-token support is a dependency for Slice 4 — flag it when 8.2's surface is final.

**Sequencing (PL, 2026-06-16): HOLD arc-2 until 8.5 lands.** Although Slices 1–2 are inference-independent and *could* run now in parallel with 8.5, the PL elected to keep the coder (Janus) parked and run arc-2 **sequentially after the inference arc closes**, so the whole Phase-8 AI surface is final before the userland arc opens (and the post-8.5 Rule-4 consolidation of `ai/ops` settles first). Arc-2 is **ratified and ready to dispatch the moment 8.5 merges** — Slice 1 first.

**Status:** §1–§9 design ACCEPTED with the §10.1 correction; all 7 ODs ratified; build slicing locked; dispatch deferred to 8.5 close.
