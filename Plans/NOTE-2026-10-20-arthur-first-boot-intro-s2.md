<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: Arthur's first-boot introduction on the desktop — the wording (step 1, for PL approval)

**Status:** wording only, no code. **Author:** S2 session (Claude). **Date:** 2026-10-20.
**Register:** `docs/VISION-arthur-persona.md`: economy, no exclamations or emoji, address sparingly, always
offer the exit. Example 10 is the model.
**Rule applied to every line:** say only what is true of the 0.1 desktop as it is on `main` today. Each claim
below cites the code that makes it true. Where it is not yet true, the line is changed or the gap is put to the
PL (§3).

## 1. The introduction (model ready)

There are two variants. The PL picks one (§3 Q5).

### Variant A: with the name (Bible 29.1)

```
Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.
The household side comes later; for now, I look after this computer for you.
I work entirely on this machine, and I don't send what you tell me anywhere.
Ask in your own words: "What does a CPU do?", for instance.
"list /" shows you the files on this machine.
Esc sets me aside and leaves Home to you. I'll keep until you need me.
```

- **Article 29.1** spells the name "Assistant for Real-time Tasks, Home automation, & User Requests". Line 1
  reads it as part of the greeting, a phrase a beginner can read past, not a slogan.
- **No home automation is promised.** Nothing in 0.1 does it: no driver, protocol or code in `kernel/` or
  `userland/`. So line 2 names it as what Arthur is *for*, says plainly that it comes later, and none of the
  examples touches it.
- **"Household", not "home",** in line 2. The desktop already calls its start view **Home** (line 6), so "the
  home part" would read as that view.
- **Length.** Line 1 is 87 characters, over the 80 bound for new lines in §2. Variant A, if chosen, raises that
  bound to 90. The 100 limit for wrapped lines is unchanged.
- **USB-only.** On a USB-only machine, the extra line below makes seven lines, one over the brief's six. If the
  PL wants six, lines 2 and 3 merge in the build: *"For now I look after this computer, and what you
  tell me stays on it."* That depends on the Q2 answer.

### Variant B: without the name

Five lines, ASCII only. The desktop text surface draws bytes, and the console greeting already uses `--` for
the same reason.

```
Good day. I'm Arthur. I look after this computer for you.
I work entirely on this machine, and I don't send what you tell me anywhere.
Ask in your own words: "What does a CPU do?", for instance.
"list /" shows you the files on this machine.
Esc sets me aside and leaves Home to you. I'll keep until you need me.
```

On a USB-only machine (no NeoFS, so D5 applies), one more line goes after line 2:

```
On this machine I can't keep anything once it's switched off, not yet.
```

**Why each line is true in 0.1:**

| Line | Claim | Grounds |
|---|---|---|
| 1 | an assistant who looks after this computer | example 10's establishing line. "Good day" replaces "Good afternoon": the kernel has no wall clock (`CLOCK_REALTIME` returns ticks since boot, `syscall.rs:5541-5546`), so a time of day could be wrong. |
| 2 | runs entirely on this machine; what you tell him goes nowhere | the model runs in-process on the local CPU (`respond_to_input`, the boot-module model of DECISION D1). Arthur is spawned holding only his session channel, two surface slots and his memory directory (`desktop_session.rs:1635-1654`), and his code makes no network call (no `SYS_NET_DIAL` in `neo-arthur`). **Caveat, §3 Q2:** this is true of Arthur's behaviour, not yet enforced by a capability. |
| 3 | a plain question is answered | with the model ready, any line that is not a Lens command goes to the model (`main.rs` desktop path, `model_ready` branch). The 0.5B answers a short factual question like this one. The example avoids anything that tempts it to describe capabilities it lacks. |
| 4 | `list /` shows the files | `showcase_logic::wants_root_lens` accepts `list /` (also `ls /`, `list directory /`, `list the directory /`) and blooms the read-only Lens on `/` through Arthur's LaunchCap. **It needs no model** (DS-1c), so it also stands in the no-model variants. |
| 5 | Esc sets him aside, and he stays | DS-1a: Esc in Arthur sends `OP_YIELD_FOCUS`, Home holds the live Arthur, and the next intent or his Spine chip resumes him (`THINK-2026-10-18-ds1-home-spine-shell.md:13`). This is the exit (rule 9): Home is the owner's, and Arthur waits. |
| USB-only | nothing survives switch-off | DECISION-2026-10-19 D5: on a USB-only machine, saving is honestly off in 0.1. The intro shows every session there (no persisted flag), so it should say why it repeats. |

## 2. Variants

The first and last lines stay. Lines 2–4 change with the model's state. The model-state sentences are reused
**verbatim** from B1b (`neo-arthur/src/model_state_logic.rs:45-57`), so the screen never disagrees with the
reply Arthur gives later.

**Model absent:**
```
Good day. I'm Arthur. I look after this computer for you.
I can hear you, but my model isn't on this system yet. Boot the LogansOS USB that came with it.
"list /" still shows you the files on this machine.
Esc sets me aside and leaves Home to you. I'll keep until you need me.
```

**Model damaged:**
```
Good day. I'm Arthur. I look after this computer for you.
My model file is damaged, so I won't use it. Re-create the USB from the original image.
"list /" still shows you the files on this machine.
Esc sets me aside and leaves Home to you. I'll keep until you need me.
```

**Model loaded but failed** (B1b's fourth state, for completeness):
```
Good day. I'm Arthur. I look after this computer for you.
My model didn't load, so I'm running without it. The details are in /model.
"list /" still shows you the files on this machine.
Esc sets me aside and leaves Home to you. I'll keep until you need me.
```

**Too little memory.** This is B2's state; it is not in code yet. The base THINK's Q4 line, with one change:
```
Good day. I'm Arthur. I look after this computer for you.
This computer has N GB of memory; my smallest brain needs M GB.
I can't think yet, but "list /" still shows you the files on this machine.
Esc sets me aside and leaves Home to you. I'll keep until you need me.
```
The change: THINK Q4 says *"I can still take notes"*. In 0.1, without a model there is no note-taking surface,
and on USB-only nothing persists (D5). So the line offers the one thing that does work without a model, the
Lens. `N` and `M` come from B2's census, rounded to whole GB. If B2 is not in 0.1, this variant ships unused.

**The lengths.** The two reused B1b lines are long (95 and 87 characters). The build wraps them; it does not
rewrite them. The persona KAT bound is therefore ≤ 100 characters per line, with every new line ≤ 80.

## 3. Questions for the PL

1. **The exit (rule 9).** The PM's brief says "help" or the shell. **Neither exists on the 0.1 desktop.**
   - Typed to Arthur, "help" goes to the model like any other line.
   - The `neo>` shell is a different boot mode (`neo-init` runs `desktop_session::run()` instead of it), so the
     desktop cannot reach it.

   The honest exit today is **Esc → Home**, plus `list /` to look at the files yourself. The wording above
   uses that.

   Should 0.1 instead gain a real deterministic path on the desktop, such as a `help` card from Home or a shell
   window? That is a scope decision, not a wording one.
2. **"Nothing you tell me leaves this machine": a guarantee or a description?**
   - The persona's last line says Arthur speaks about what the capability model *guarantees*.
   - In 0.1 the `desktop` kernel feature includes `product`, and therefore `boot-eth0` (USB Ethernet + DHCP).
   - `net-dial-cap` is **off**, so `SYS_NET_DIAL` is ambient: any process may dial.
   - Arthur never does, and holds no network handle, but no capability forbids it.

   Options:
   - **(a) recommended:** turn on `net-dial-cap` in the `desktop` feature set (Arthur holds no `NetDial` cap,
     so he then *cannot* dial), and use the stronger line *"I work entirely on this machine; nothing you tell
     me leaves it."*
   - **(b)** ship line 2 as written above, a true description of his behaviour, and leave the guarantee for
     later.
3. **"Good day" for "Good afternoon".** Forced by the missing wall clock. Keep example 10's line for when an
   RTC driver lands?
4. **The USB-only line.** Include it (recommended: it explains why the intro repeats every session), or drop it
   to stay at five lines?
5. **Variant A (with the name) or variant B (without).**
   - A makes the name mean something on day one and is honest about the household part.
   - B is shorter by one line and says only what 0.1 does.
   - Recommendation: **B for 0.1**, and A once something on the household side exists, so the expansion stops
     needing a caveat. A is fully written above if the PL prefers it now.

## 4. For step 2 (after approval): where it goes

**Recommendation: on Home, not in Arthur's window.**
- **Reasoning.** On session start the user sees Home: the intent field on the WM's own focus (DS-1). Arthur is
  not spawned until the first intent. An intro inside Arthur's window would appear only *after* the user had
  already typed to someone they had not met.
- **Shape.** On Home, a short card under the intent field, drawn by the WM with no model needed.
- **Lifetime.** It shows at session start when no persisted "introduced" flag exists, which on USB-only means
  every session. It goes away at the first keystroke into the field, Esc, or a click elsewhere, and it never
  takes focus.
- **Kill-switch.** `desktop-intro` off means today's Home, bit for bit.
- **Discriminator.**
  - a desktop QEMU scenario whose screendump fixture shows the card, digest-checked like the other Home
    captures;
  - a host KAT asserting the persona rules on every variant: no `!`, no emoji, ASCII only, the line-length
    bounds above, and the four model-state lines exactly B1b's.
- **Model state.** The card picks its variant from the model state the WM learns at boot, or shows the
  model-ready text until Arthur reports otherwise. That is a step-2 design point.
