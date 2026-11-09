<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PR #62 password message review

Reviewed head: `6c1bb542b88b2997dc203cc588730db121eb944d`

- PASS — The wrong-password text is the same generic message at sign-in and on the lock screen. It does not distinguish a bad password from any other account detail. `userland/neo-init/src/desktop_session/login.rs:689-712`; `userland/neo-init/src/desktop_session/lock.rs:394-396,432-445,538-549`.
- PASS — Rate limiting and lockout behavior are unchanged: the same persisted attempt counter, pre-check increment, `delay_secs` calculation, and wait/drop-input path remain in use. `userland/neo-init/src/desktop_session/login.rs:254-322,720-726`; `userland/neo-init/src/desktop_session/lock.rs:321-340,358-373,411-445`.
- PASS — The sign-in entry is wiped on Enter before the next prompt; the lock field is wiped before password verification. Sign-in retains focus, and the lock remains engaged on refusal; only successful verification unlocks it. The new note is rendered as the last row beside the field on both screens. `userland/neo-init/src/desktop_session/login.rs:512-541,705-713`; `userland/neo-init/src/desktop_session/lock.rs:401-420,538-549`.
