<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-069: Arthur ships "yolo-within-guardrails" as the default posture

**Date:** 2026-07-03
**Status:** Accepted (PL-ratified posture decision, 2026-07-03; build dispatch
`Plans/DISPATCH-2026-07-03-arthur-yolo-within-guardrails-oracle.md`)
**Builds on:** OT-18 D1–D7 (front-loaded grants), OT-19 (reversibility),
OT-26 D1–D6 (constrained agentic decoding), the A2/A3 tiers
(`Plans/THINK-2026-07-02-arthur-agentic-a3-design.md`), Article 29.3.

## Decision

Arthur's agentic layer boots in a **yolo-within-scope** posture by default:
**reversible** actions (the A3 taxonomy's R and T tiers — snapshot-backed
`write_file`, trash-convertible `trash_file`/`move_file`) **inside held
grants execute with no per-action confirmation**, with the A2 undo net armed.
Friction surfaces in exactly the cases where it buys safety:

- **A missing cap** — the action still routes through the unchanged
  `execute_action()` cap-check and is **DENIED, loudly** (the reply names the
  exact missing resource+rights — the OT-18 D3 on-miss surface). Confirmation
  was never the guard; the capability floor is.
- **An irreversible act** — the **L tier** (irreversible-local: kill, spawn,
  model-load) and the **X tier** (irreversible-external: send) keep the A3
  dry-run + typed-confirm gate **in every posture**; the **U tier** still
  refuses outright. Yolo drops friction only where recovery exists; the
  A3-D4 hard rule (no silent destruction or send) is not weakened.

## The two-layer separation (why yolo is safe)

Posture is **layer 1** — confirmation friction in the NL layer. **Layer 2**
— the kernel capability check — is untouched by this build (`action.rs` not
in the diff). A confused or hijacked yolo-Arthur can *propose* anything but
can *cause* only what its held caps permit: `yolo_missing_cap = DENIED` is
proven with confirmation skipped, at both the host-KAT tier and the live
QEMU scenario. This is Article 29.3 doing the work confirmation never could.

## The three-posture selector (the kill-switch)

`/posture [yolo|cautious|off]` — a **deterministic user command intercepted
before model decode**. There is no posture production in the constrained
grammar, so the model structurally cannot switch its own posture (the OT-26
D1 argument applied to policy). A posture switch stales any pending
preview/typed-confirm phrase (the stale-confirm discipline extended).

- **yolo** — the shipped default (above).
- **cautious** — the pre-yolo A2/A3 behavior **byte-for-byte**: the cautious
  arm is literally the unchanged code path, and the equality is pinned by
  host KATs whose expected bytes are copied from the pre-yolo tests, plus
  the live QEMU cautious segment reproducing the full A2/A3 transcript.
- **off** — the agentic layer is **inert**: no model decode, no executor
  call, a deterministic notice reply. Note the honest scope of this
  equality: inside an `agentic-actions` build the pre-agentic chat tier is
  not linked, so *bit-for-bit pre-agentic* remains the **compile-time**
  collapse already proven at A1 (`agentic-actions` off ≡ chat-only);
  runtime `off` proves **structural inertness** (decoder + executor
  unreachable — the property the pre-agentic layer had by construction).

## Reply markers

Replies produced on the yolo fast path (direct R/T execution, denials
reached without confirmation, and L/X previews staged under yolo) carry a
trailing ` posture=yolo` marker. Cautious replies are marker-free (the
byte-equality requires it). The marker is what makes the QEMU needles
segment-unambiguous — the discriminator discipline applied to a policy
build.

## The safety-contract matrix (ISC-5 — the falsifiable enumeration)

| Row | Proof |
|-----|-------|
| `yolo_reversible_granted = executes_silently` | host KAT + QEMU needle (`write_file … posture=yolo`, `trash_file … posture=yolo`) |
| `yolo_missing_cap = DENIED` | host KAT (executor invoked, denial returned) + QEMU needle (`Denied: write_file requires file /forbidden rights=0x2 posture=yolo`) |
| `yolo_irreversible_local = still_gated` | host KAT + QEMU needle (`phrase=confirm kill 4242 posture=yolo`) |
| `yolo_irreversible_external = still_gated` | host KAT (untyped confirm no-op; typed executes) + QEMU needle (`phrase=confirm send … posture=yolo`) |
| `yolo_wrong_reversible = undo_recovers` | host KAT + QEMU undo/restore needles |
| `posture_cautious = a3_equiv` | host KAT byte-equality corpus + the QEMU cautious segment reproducing every pre-yolo needle |
| `posture_off = inert` | host KAT (zero decoder/executor calls) + QEMU off-notice needle |

Negative controls verified during the build: (1) making the yolo arm bypass
the executor turns the missing-cap and silent-write KATs red; (2) adding
`ExternalSend` to the yolo fast path turns the X-gate KATs red.

## Invariants

**No new invariant.** The posture rides I-15 (file caps), the OT-18 grant
graph, and the existing confirmation machinery; it is layer-1 policy in
userland (`conversation_logic.rs`), pure CPU at the sampling boundary — no
ISR/DMA/governance-lock contact (I-14 posture inherited from A1/A2/A3).
No new on-disk or on-wire format (posture is session state, never
serialized), so no new byte oracle is owed; the AST grammar is unchanged
(pinned by the not-model-emittable KAT).
