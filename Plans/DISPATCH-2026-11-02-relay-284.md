<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 284: Jarvis, design read of the setup-menu THINK (J.c.'s F11)

**MODEL** GPT-6-Luna, **EFFORT** high: a cross-family read of Dexter's (DeepSeek) design; a Claude seat builds it.
Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. `git fetch origin dexter/setup-menu-think && git
show origin/dexter/setup-menu-think:docs/THINK-2026-11-02-setup-menu.md` (head `a514ea63`). J.c.'s answers to §7 are in the
PR body (hide single-option pages; run on every Try boot; Install via restart for now).
1. The security model (§4.1, §4.4): the WM is the only settings writer; the clock confirm is authority-checked; nothing
   in the wizard can grant itself more than the session has; no client can spoof or draw over it.
2. Each screen's "works now / stub / new rung" claims in §3 match `main` (check the `file:line`s).
3. Try vs Install (§2.3, §2.4): what persists and what doesn't is unambiguous; nothing from a Try session leaks to disk.
4. The rungs (§5) each have an independent oracle and a kill-switch; the off path is today's boot unchanged.
5. Anything in §6 that's already true (i.e. a known way the design is wrong).
RETURN: `Plans/RETURN-2026-11-02-setup-menu-review-jarvis.md`, branch `codex/setup-menu-review-jarvis`.
