# PL-ROADMAP-MARKER: Windows personality

- Recorded by: PM
- Date: 2026-05-25
- Authority: Project Lead roadmap instruction
- Status: ROADMAP TARGET RECORDED
- Related: `docs/ADR-023-linux-personality-layer.md`, `docs/bible.md` Article 5.6 and Article 28.2

---

## 1. PL Instruction

PL instructed PM to add Windows personality to the NeoPlutarch roadmap.

The motivation is strategic: Windows remains the dominant desktop software ecosystem, and long-term NeoPlutarch adoption is helped by a path to run mainstream Windows applications, not only Linux-native applications and Proton-compatible games.

## 2. Roadmap Ruling

Windows personality is now a long-horizon strategic roadmap target.

This marker does not replace ADR-023's Linux-first compatibility path. The intended sequence is:

1. Deliver the native NeoPlutarch substrate: networking, filesystem, graphics/input/audio foundations, and AI-native phases.
2. Deliver the Linux personality first, per ADR-023.
3. Use Linux personality plus Vulkan/graphics/audio/input support to reach Windows-game compatibility through Wine/Proton.
4. Open a future Windows-personality ADR when the substrate is mature enough.

The Windows personality must reuse the Article 5.6 personality-subsystem mechanism: opt-in per process, kernel-resident syscall translation, isolated from the native ABI, and incapable of changing native ABI semantics.

## 3. Compatibility Boundary Rule

PL further clarified that supporting both Linux and Windows software must not make NeoPlutarch bloated.

PM records the rule as:

> Compatibility at the boundary, NeoPlutarch in the core.

Operational meaning:

- Native kernel services stay NeoPlutarch-shaped: scheduler, memory, VFS, networking, graphics/accelerator substrate, and capability/security model.
- Linux and Windows behavior lives in personality dispatch, personality-owned shims, or personality-owned userland services.
- Multiple personalities may run at the same time on the same kernel, but each process has exactly one personality at a time.
- Shared core hooks are allowed only when they are general primitives useful to native NeoPlutarch code too.
- No native subsystem should grow scattered `if Linux` / `if Windows` branches.
- Novelty is not a veto. If no prior OS has done this exact combination cleanly, NeoPlutarch may invent it, but the invention must strengthen the boundary rather than dissolve it.

This rule is now recorded in `docs/bible.md` Article 5.7.

## 4. Placement

Roadmap placement is post-Linux-personality and post-graphics-foundation. PM shorthand: Phase 10+/11+ unless future phase planning chooses a different label.

The future specification placeholder is:

- `WINSPEC-001` - Windows Personality Specification

That spec is not opened now. It becomes appropriate only after Phase 8 Linux personality planning and the Phase 9+ graphics/audio/input compatibility work have concrete shape.

## 5. Expected Scope Of A Future ADR

A future Windows-personality ADR should address at least:

- PE/COFF image loading and process startup
- NT syscall dispatch model and process/thread personality binding
- Windows object namespace and handle semantics
- Win32 subsystem boundary: what lives in kernel personality vs userland compatibility services
- registry-compatible configuration surface
- service/process model
- filesystem worldview and drive-letter/path mapping
- networking translation onto NeoPlutarch's Phase 4 networking API
- input, audio, display, and graphics integration
- DirectX path: likely DirectX-to-Vulkan translation rather than native DirectX in the kernel
- conformance with Bible Article 5.7's compatibility boundary rule
- compatibility test tiers and representative application/game set
- legal/source discipline for learning from Wine, ReactOS, public Microsoft documentation, and clean-room behavior tests

## 6. Non-Goals For Current Phases

This marker does not authorize:

- Phase 2, 3, or 4 implementation work
- native ABI pollution with Win32/NT semantics
- replacing the Linux personality path
- importing Windows APIs into libneo
- copying implementation code from Wine, ReactOS, or Windows
- claiming Chrome/Firefox/Office/game compatibility before the relevant substrate exists

## 7. Immediate Documentation Actions

PM records this marker and updates roadmap-facing documentation so future readers do not rediscover the decision:

- `docs/bible.md` gets Article 5.7, the compatibility boundary rule.
- `README.md` roadmap gets a future-compatibility marker.
- `docs/design.md` vision roadmap gets a future Windows-personality note.
- `docs/ADR-023-linux-personality-layer.md` gets a post-ratification roadmap note clarifying that ADR-023 remains Linux-first, while Windows personality is now a later strategic target.

## 8. References

- `docs/bible.md` Article 5.6 - personality-subsystem mechanism.
- `docs/bible.md` Article 5.7 - compatibility boundary rule.
- `docs/bible.md` Article 28.2 - useful-software accessibility goal.
- `docs/ADR-023-linux-personality-layer.md` - Linux personality and reusable mechanism.
- `Plans/PM-DISPOSITION-2026-05-21-adr-023-and-bible-v1.7.0-ratification.md` - historical Linux-only ratification posture, now superseded only as to "not anticipated" for Windows personality.
