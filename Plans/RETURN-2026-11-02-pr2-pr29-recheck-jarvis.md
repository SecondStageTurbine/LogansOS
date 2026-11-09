<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 266 — PR #2 and #29 re-checks — Jarvis

Reviewed heads: #2 `8fd7df13fd483054e7b17a16e14f15b400f14509`;
#29 `8ca815fab650650e51dfd6e8c7eae66da33abe44`.
RETURN branch: `codex/pr2-pr29-recheck-jarvis`, from `origin/main`
at `bcfe69827`. Source references are at the respective reviewed head.
Read the relay 255 and 246 RETURNs and the latest seat notes:
[#2 PM, 2026-11-02 01:27:02 UTC](https://github.com/SecondStageTurbine/LogansOS/pull/2#issuecomment-6566884807)
and [#29 PM, 2026-11-02 04:51:50 UTC](https://github.com/SecondStageTurbine/LogansOS/pull/29#issuecomment-6569359777).

- **PASS — #2, relay 255: errno on write errors.**
  `userland/neo-crt/src/lib.rs:296` routes both standard streams through
  `write_result_for_c` (`:311`), which returns -1 and sets the error's errno.
  `errno_for_io_error` (`:327`) preserves positive raw OS codes; otherwise
  it maps error kinds (including BrokenPipe to EPIPE and WouldBlock to EAGAIN),
  falling back to EIO. The PAL produces positive raw codes from negative syscall
  results (`userland/neoplutarch-std-overlay/std/stdio_neoplutarch.rs:30`).
  The earlier blanket-EIO conversion is gone. Successful results do not set errno,
  and unsupported C descriptors still return -1/EBADF (`src/lib.rs:302`).
  `userland/cxx-runtime-smoke/src/main.rs:113` checks raw EPIPE/EAGAIN/EACCES/EBADF,
  std-made BrokenPipe/WriteZero, success, and the invalid-descriptor C entry point
  against hand-written numbers; `tools/xtask/src/main.rs:16596` requires check 18.

- **PASS — #29, relay 246 item 4: the retained dial authority cannot transfer or survive the broker.**
  Before receiving requests (`userland/fetch-broker/src/main.rs:64`),
  `request_root` derives USE|DERIVE|REVOKE without GRANT (`:113`), then closes
  the GRANT-bearing handle 1 and exits if closing fails (`:124`). The new
  feature-gated index-1 NetDial close is accepted at
  `kernel/src/arch/x86_64/syscall.rs:6922`; other NetDial closes are accepted
  at `:6934`. `kernel/src/sched/process.rs:3189` removes the handle under
  CAP_GRAPH and retires its edge, preserving the launcher's ancestor-revoke
  reach through reparenting (`:3154`). After initialization, the earlier
  derive-directly-from-handle-1 counterexample returns EBADF. Generic derivation
  cannot restore GRANT (`:3380`), and scoped derivation preserves the reduced
  ceiling (`:3443`). Spawn refuses inheritance without GRANT
  (`kernel/src/arch/x86_64/syscall.rs:1963`); channel grant refuses it at
  `:8095`, with installation rechecking it at `kernel/src/sched/process.rs:3874`.
  All remaining dial handles therefore stay in the broker's own table, which
  teardown drains (`:3166`). The broker's root-close/grant-refusal self-check
  is at `userland/fetch-broker/src/main.rs:130`; the oracle requires both
  refusals at `tools/xtask/src/main.rs:18810`. This relies on the trusted
  broker completing its startup attenuation before serving content; the
  launcher's generic delegation policy itself remains available.

- **PASS — #29, relay 246 new finding: missing REVOKE on the request root.**
  `userland/fetch-broker/src/main.rs:113` retains REVOKE, inherited by each
  request scope (`kernel/src/sched/process.rs:3443`). The check at `:3491`
  now permits cleanup; the revoke removes the scope and walks/removes its
  descendants (`:3494`, `:3500`, `:3522`), including a narrowed dial leaf.
  `revoke_scope` checks and logs the result (`userland/fetch-broker/src/main.rs:146`).
  It is called on leaf-derivation failure (`:201`), dial failure (`:212`),
  and connection close (`:243`); TLS failure closes the connection (`:233`),
  and exchange completion or failure closes it before returning (`:169`).
  Repeating the earlier fixture FETCH no longer accumulates scopes through
  EPERM. The oracle requires a successful revoke and rejects any logged
  revoke failure (`tools/xtask/src/main.rs:18816`, `:18822`).

- **PASS — anything new:** no additional finding in the changed paths; the
  index-1 close accepts only NetDial and remains gated by net-dial-public-only
  (`kernel/src/arch/x86_64/syscall.rs:6910`, `:6922`), preserving the ordinary
  stdout close refusal and feature-off behavior.

Validation: at the exact #29 head, `cargo test -p fetch-proto -p cap-graph-logic-tests`
passed all 29 protocol and 9 graph tests. #2's errno mapping and smoke were
reviewed in source; the target smoke was not run locally. QEMU and xorriso
are unavailable here. The PM comments report green gates, including
`cxx_runtime_smoke` for #2 and `fetch_broker` with the new oracle for #29;
those are external evidence, not local boot results. No implementation changes.
