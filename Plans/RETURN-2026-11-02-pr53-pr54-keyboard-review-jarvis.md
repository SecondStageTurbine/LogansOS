<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — PR #53/#54 keyboard review

**Reviewer:** Jarvis  
**Reviewed #53:** `ebf82c642d0c183170cce078b78fcc0054b42959`  
**Reviewed #54:** `9c0af14c8d8b38f0f0f4274280520cc04229e79c`  
**Base:** `origin/main` at review time  
**Branch:** `codex/pr53-pr54-review-jarvis` (RETURN only)

- **PASS — US keymap and shared map.** `tools/xtask/src/keymap_tests.rs:21-87` gives hand-written expected values for all 48 printable US usages and exercises plain, left/right Shift, Caps Lock, and Caps+Shift. `:90-105` checks printable ASCII coverage. The PS/2 KAT feeds set-1 bytes through the decoder into the same table (`:933-1024`); the USB legacy decoder includes that table (`kernel/src/drivers/usb/hid_logic.rs:168-185`), and Home and Arthur include the same source (`userland/neo-atrium/src/keys.rs:30-33`, `userland/neo-arthur/src/showcase_logic.rs:104-108`).
- **PASS — keypad on the configured PS/2 path.** The i8042 config enables translation and the decoder explicitly consumes set 1 (`kernel/src/drivers/ps2/i8042_logic.rs:6-15, 121-159`). The table maps keypad keys and E0 keypad Enter and `/` (`:121-159, 261-269`); the KAT checks keypad digits/operators, Num Lock states, E0 Enter and `/`, Num Lock off navigation, and fake-shift-wrapped Home (`tools/xtask/src/keymap_tests.rs:108-188, 969-1012`). Raw set-2 bytes are not decoder inputs: the controller translates them to set 1 by design.
- **PASS — lock-screen routing unchanged.** Both keyboard drivers still submit their decoded key events through the existing focus route (`kernel/src/drivers/ps2/i8042.rs:475-496`, `kernel/src/drivers/usb/hid_keyboard.rs:286-316`). With the lock-route feature, `focus::input_route` selects the WM lock focus while locked, so a client does not receive those events (`kernel/src/atrium/focus.rs:824-844`).
- **FAIL — Linux atkbd trap-mining findings are missing.** `kernel/src/drivers/ps2/i8042_logic.rs:17-21` says the field traps are in a “FINDINGS note,” but neither PR adds that note, and no such note exists in the tree. Concrete check: search the reviewed tree for `FINDINGS note`, `atkbd`, or a keyboard trap-mining findings file; the only keyboard `atkbd` hit is this dangling reference. Add the cited Linux-principle findings and map each trap to this decoder before treating this item as closed.

Static source review only; no tests were run.
