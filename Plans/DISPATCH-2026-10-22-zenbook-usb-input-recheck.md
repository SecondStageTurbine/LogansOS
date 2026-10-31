<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: narrow re-check of the Zenbook USB input review findings (#93) → Chad

**MODEL: GPT-6.1-Sol — EFFORT: medium.** A narrow re-check of the reviewer's own four findings (CLAUDE.md
routing table).

**PR:** #93, `claude/zenbook-usb-input`. **Your RETURN:** `Plans/RETURN-2026-10-22-zenbook-usb-input-review-chad.md`
at `b0f9c314` (BOUNCE at `6f09e826`). **What changed:** `Plans/NOTE-2026-10-22-zenbook-usb-input.md` §8.

```
git fetch origin
git show origin/claude/zenbook-usb-input:Plans/DISPATCH-2026-10-22-zenbook-usb-input-recheck.md
git diff 6f09e826 origin/claude/zenbook-usb-input -- kernel tools
```

WORKTREE: `E:\AI Automation\workers\chad` (yours alone; never another coder's folder, never the repo root)
BRANCH: `codex/zenbook-usb-input-recheck-chad` (`git fetch origin && git checkout -B codex/zenbook-usb-input-recheck-chad origin/claude/zenbook-usb-input`)

## Check (pass/fail each, with `file:line`; a concrete input for any fail)

1. **Controller choice.** With the second controller's protocol chain absent or truncated:
   - a connected low/full/high-speed device on it still makes it `usb2-device-connected`
     (`port_holds_usb2_device`, `xhci_silicon.rs::probe`);
   - with nothing connected, it is chosen over a controller known to be SuperSpeed-only, with the reason
     `AMBIGUOUS-protocols-unknown` on screen.
   Is there still an input where a known SuperSpeed-only controller is chosen while another readable
   controller could host a USB 2 device?
2. **Multi-TT.** `select_multi_tt`: MTT is set only for a high-speed hub with bDeviceProtocol 2, and only after
   SET_INTERFACE(0, 1) succeeds; otherwise the slot context leaves MTT clear. Is that the right condition?
3. **Hub-child retries.** `ADDRESSED` is filled at the end of `address_and_describe` (before the offer) and of
   `enumerate_hub`; `service_hubs` uses it, not PORT_ENABLE. Does an enabled child whose Enable Slot or
   Address Device fails now get its three attempts? Does a detach clear the right entries
   (`remove_subtree`, `route_is_under`)?
4. **Kill-switch.** `EXTRA_GLYPHS` and its lookup need `fb-progress-console` and `xhci-silicon`. The
   author's `.text` comparison against `main` now includes `fb-progress-console`,
   `fb-progress-console,lapic-timer` and `product,fb-progress-console,lapic-timer` (identical).

Optional: `cargo test -p xhci-silicon-logic-tests` (25), `cargo xtask neo-test usb_xhci_select` and
`usb_hotplug` at `NEO_SMP=4`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='xHCI hub multi TT slot context'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-22-zenbook-usb-input-recheck-chad.md` to your branch and push it: a verdict
(MERGE / MERGE-WITH-NITS / BOUNCE), items 1–4 pass/fail with `file:line`, any new finding with a concrete
input. Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
