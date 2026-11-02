<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Dispatch: Seat I, two more items from J.c.'s preview (before the time-zone build)

J.c. (2026-11-02, preview, accelerated mode). Feedback items in `Plans/FEEDBACK-2026-11-02-preview.md`.

## F10 first: key auto-repeat
Holding Backspace (or any key) doesn't repeat. The i8042 driver drops typematic repeats on purpose (your FINDINGS
note, trap 9: "held-set dedupe"), expecting software auto-repeat, which doesn't exist. Build it once, shared by the
PS/2 and USB paths: after a key is held for a delay (about 500 ms), repeat it at a rate (about 30/s) until its break;
modifiers, Caps/Num/Scroll Lock and the lock-screen keys don't repeat; a repeat goes to whoever has focus *now* (never
to a client that lost focus or behind the lock screen); release, focus change and lock all stop it. Host KATs on the
repeat state machine with a fake clock; a QEMU scenario holding Backspace that checks several characters are deleted.

## F9: tell the user a password was wrong
At login (and the lock screen), a wrong password is refused silently. Show "Incorrect password. Try again." under the
field, clear the field, keep focus there, and keep any existing rate limit or lockout behaviour. Say nothing about
*which* part was wrong (user vs password). Update the login/lock capture oracles for the new frame.

Then continue the time-zone build (THINK #51, merged): J.c. is still seeing the wrong time.
Usual rules: close-gate, gate, PR per item, `PM:` comment; kernel-touching parts get a Codex review the PM queues.
