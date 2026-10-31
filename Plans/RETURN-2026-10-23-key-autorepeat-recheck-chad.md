# Return: key auto-repeat item 11 re-check

**Verdict: CLOSED.** Reviewed `d59bab14a496574ad68e7cb1886b8e196e55b61f` against the three-item dispatch.

| # | Result | Evidence |
|---|---|---|
| 1. Item 11 closed | PASS | `kernel/src/drivers/ps2/i8042.rs:517-535` gates `drain_events_stamped` on `neo-test-ps2-key-repeat`, samples one `now_ns`, passes that same value to `on_event`/`poll`, and stamps emitted events with it. `kernel/src/test_harness.rs:25101-25109` checks each repeat k against `t_press + DELAY_NS + k * INTERVAL_NS` with a strict `<` failure and no slack. A press at 0 ms followed by a first repeat at 490 ms fails k=0; repeats at 500, 530, and 560 ms fail at k=1 because 530 ms is earlier than 533,333,333 ns. |
| 2. Honest claim | PASS | `Plans/NOTE-2026-10-23-key-autorepeat.md:81-85` says the bound is per repeat index and explicitly disclaims a 33 ms minimum adjacent gap, noting 30 or 40 ms tick gaps. `tools/atrium-logic-tests/src/key_repeat_kats.rs:241-260` asserts the indexed deadline and separately constrains 10 ms polling gaps to 30 or 40 ms. The claims match the clock's 10 ms quantization. |
| 3. Test-only / production logic | PASS | The stamping hook is gated by `#[cfg(feature = "neo-test-ps2-key-repeat")]` at `kernel/src/drivers/ps2/i8042.rs:517`; it is not available in production builds. `git diff b17cb2f2 d59bab14 -- kernel/src/atrium/key_repeat_logic.rs` is empty: the repeat state machine is unchanged. `key_repeat.rs:20-50` and the i8042/USB pump call sites now pass one timestamp through the wrapper; this is clock plumbing for consistent stamps. The i8042 pump holds `STATE` at `i8042.rs:446-453`, and the USB pump calls `drain` under `without_interrupts` and the device lock at `hid_keyboard.rs:359-367`, so the timer tick cannot advance between the old per-call reads on these paths. |

No build was needed for this review. No Zenbook disk was written.
