<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — the playbook lanes L1, L2 and L7 (J.c. order, 2026-10-30)

**J.c.:** lanes L1, L2 and L7 first, then L3 and L4 (`docs/THINK-2026-10-30-playbook-gap-map.md`). The browser has
its own seat (D). Seats A, B and C take these **after their current queues**. Each item is its own PR; the usual rules
apply (kill-switch, close-gate, `RETURN (seat X)`, and the prior-art survey for unfamiliar subsystems).

## Seat A, after the mount authority
- **L1.1, `cap-rooted-open` in image A.** First K1b: per-client RootDirCap scoping (Arthur and every client get only
  their own subtree; neo-init keeps `/`). Then add the feature to `PRODUCT_IMAGE_KERNEL_FEATURES` with a pairing
  assert. Scenario: the product boot, with Arthur's open of a path outside its subtree returning EPERM.
- **L1.2, `net-dial-cap` and `interface-caps` in image A.** Egress needs a granted cap; only the network broker or
  the WM holds it. Scenario: a client without the cap gets EPERM on SYS_NET_DIAL.

## Seat B, after #226's follow-ups and Dexter #187's
- **L1.3, Arthur's actions in image A, behind a WM permission prompt.** Turn on `arthur-grants`, the stakes/confirm
  path and `reversibility`. The WM shows "Arthur wants to <action> <object>: Allow once / Always / Deny"; "Always"
  writes a grant. **J.c. hasn't yet decided** whether consequential actions start read-only; build the prompt so
  both work, with read-only as the default until he says otherwise.
- **L2.1, the action log:** append-only and hash-chained on NeoFS under `.system/` (the WM-only store). It records
  every grant, prompt answer, action and undo, with the intent text. Byte oracle for the record format.
- **L2.2, the "What Arthur did" view** in the Lens, with undo where reversible; and **L2.3, the permission
  inspector**: grants and caps per app, with revoke through the cap graph.
- **L2.4, memory export:** a documented, versioned format (byte oracle). Export is a user action from the inspector.

## Seat C, after the #183/#189 follow-ups
- **L1.4, `power-control` in image A:** Shut down and Restart in the WM (using OT-90's S5 and reset paths), behind a
  confirm.
- **L7.1, a boot-time number on the Zenbook:** fb-console timestamps from firmware handoff to the login screen and
  to Arthur ready, recorded per run in the runbook's evidence. J.c. does the run.
- **L7.2, image size budgets in CI (moved to Dexter, 2026-10-30):** image A's kernel `.text` and ISO size checked against a committed budget file;
  a PR that grows them past the budget must raise the budget in the same PR, with a reason.
- **L7.3, NeoFS cache eviction (OT-92).** Then, as a design THINK first, **sleep/wake (S0ix) on the Zenbook**.

## Then: L3 (updates and rollback) and L4 (the app platform)
Those dispatches come once L1, L2 and L7 are in review. L3 needs J.c.'s signing-root decision first.
