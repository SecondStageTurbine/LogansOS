# Bible Article 6.6 — Inference-governance hooks (LogitPort)

**Type:** Bible amendment draft (Option C from `HANDOFF-2026-05-05-build3-sub-commit-1a-close.md` §10.2)
**To:** PM — for direct commit; not a coder dispatch.
**From:** Claude (design partner)
**Date:** 2026-05-06
**Authority required:** Project Lead (Bible Article 27)
**Composes with:** Articles 6.1–6.5 (existing AI subsystem chapter), 12.5 (AI models as security boundary), 23.1 (no system AI persona), 23.3 (no embedded specific model)
**Substrate:** ANSPEC-002 §7 (LogitPort capability surface); ALTIMIT research artifact §G Recommendation 9 + §J "the Morganna pattern"

---

## 0. Why Bible-level

ANSPEC-002 §7 already specifies LogitPort technically — semantic-rights bits, attachment mechanics, anti-scope, the alignment with Articles 23.1/23.3/12.5. That's the right place for the technical shape and is already on disk.

What ANSPEC-002 cannot do is establish the *constitutional* commitment: that the kernel will expose inference-governance hooks but will not implement governance policy. That's a Bible-level shape because:

1. It is a permanent direction-setter — like 23.1 ("no chatbot persona"), it's a clause that future work measures itself against, not a technical surface that evolves as Phases land.
2. It tells future maintainers *why* LogitPort exists without 6.6 being a slow path to an embedded safety/alignment subsystem. The text below makes that distinction explicit and resists exactly the failure mode the research artifact §J names ("the Morganna pattern" — the OS that embedded a "mother program" is the one that crashed The World).
3. Without 6.6, a future PR that adds a kernel-internal "default safety policy" (a policy module that runs on every inference, not a hook for userspace to opt in) has no Bible clause to lose against. With 6.6, it does.

The text below is short by design. Article 6 is structural-assertion-shaped (5–10 lines per clause); 6.6 fits that shape.

---

## 1. The proposed text

To be inserted into `docs/bible.md` between Article 6.5 and the start of Article 7. The exact insertion point is after the line `6.5 When no accelerator is present, the kernel falls back to a CPU inference backend. CPU inference is correct and slow; this is acceptable. We do not ship a system that requires a GPU to boot.`

```
6.6 The kernel exposes inference-governance hooks but does not implement governance policy. Specifically, the AI subsystem provides capability-gated mechanisms — LogitPort being the canonical example (ANSPEC-002 §7) — by which a holding process can observe, veto, or reweight the logit distribution of an inference context before sampling. The mechanism is the kernel's contribution; the policy is the application's. The kernel does not ship a default safety filter, alignment layer, or content classifier. It ships the primitives by which userspace builds those things, capability-gated like every other kernel object.

This clause exists because the alternative — a kernel-resident governance policy that runs on every inference by default — is the failure mode Articles 23.1 and 23.3 already foreclose, in a less-obvious form. A "default safety filter the kernel applies on every inference" is a system AI persona by another name and an embedded specific model by another name; both are forbidden. 6.6 is the positive version of that prohibition: the kernel exposes the seam on which governance can be built; the kernel does not stand on that seam itself.
```

That's the whole clause. Two paragraphs. The first establishes the mechanism-not-policy direction; the second names why, by reference to existing Bible clauses.

---

## 2. Why the wording is the way it is

A few specific choices worth flagging so PM can push back if any feel wrong:

**"LogitPort being the canonical example."** Naming LogitPort by name in the Bible does carry a small risk: the name is ANSPEC-002's invention and could in principle be renamed. But the Bible already names ".neom" and "neo-init" by name (Articles 6.2, 5.4, 25.2), so naming a kernel-internal object is consistent. Renaming LogitPort later would require this clause's amendment; that's acceptable — the rename should be deliberate.

The alternative wording "the AI subsystem provides capability-gated mechanisms by which a holding process can observe, veto, or reweight inference behavior" is more general but loses the cross-reference to ANSPEC-002 §7. The current wording does both: names LogitPort as the canonical instance and points readers to where it's specified.

**"Capability-gated."** Load-bearing word. It connects 6.6 to Article 12.1 (capability-based security from Phase 2) without restating it. Anyone reading 6.6 who wonders "but couldn't an unprivileged process attach a LogitPort and snoop on inferences" finds the answer in "capability-gated" — they go look at ANSPEC-002 §7.4 and see `inference_context_rights::ATTACH_LOGIT_PORT` is required.

**"Default safety filter, alignment layer, or content classifier."** This list is intentionally specific. Naming concrete things the kernel does NOT ship makes the clause harder to misread as merely aspirational. A future PR proposing "let's add a small built-in toxicity classifier as a quality-of-life feature" has to argue against this list, not against an abstract principle.

**"Capability-gated like every other kernel object."** This is the consistency hook. It says: the new thing is not new in shape, only in surface. Tensors, models, processes, channels — all capability-gated. LogitPort is the same. No new security model is being introduced; the existing one extends.

**The second paragraph's argument structure.** The claim is: "kernel-default governance" reduces to "embedded persona" + "embedded model," both already forbidden. The argument is one move long: governance-running-by-default is a persona (it acts on the user's inference without being asked); it requires an embedded model (the policy itself is a model, however small). Both 23.1 and 23.3 already foreclose those. So 6.6's prohibition on default governance policy is not a new restriction — it's an explicit consequence of restrictions already in place.

**What the second paragraph deliberately does not do:** invoke the .hack canon ("Morganna pattern"). The research artifact at `docs/research/ALTIMIT-real-world-implementation.md` is informational, not normative; it's correct material for a rationale section but not for Bible text. The Bible should stand on its own constitutional logic, not on franchise reference. The rationale section §3 below cites the research artifact for the amendment record, but the article text doesn't.

---

## 3. Rationale section (for the commit body, not the article)

The amendment commit's body should include a rationale per Article 27. Suggested text:

```
Rationale (Article 27):

Article 6.6 specifies that the kernel exposes inference-governance hooks (the
canonical example being LogitPort, defined in ANSPEC-002 §7) but does not
itself implement governance policy. The clause is the positive form of two
existing prohibitions:

- Article 23.1 forbids a "system AI assistant" or "chatbot persona." A kernel
  that runs a default safety filter or alignment layer on every inference,
  without being asked, is a system persona by another name — it acts on
  behalf of the user without the user's invocation.
- Article 23.3 forbids embedding a specific model in the kernel. Any default
  governance policy the kernel runs *is* a model — possibly a small one,
  possibly a hand-coded heuristic, but a model in the relevant sense.

Together, 23.1 and 23.3 already foreclose default governance policy. Article
6.6 makes the consequence explicit: governance is exposed as mechanism
(LogitPort and future hooks), not implemented as kernel-resident policy.

This is the kernel's contribution to the inference-governance problem: a
capability-gated seam on which userspace can build policies. The policies
themselves — safety, alignment, audit, refusal — are application concerns.
The kernel is a substrate, not a participant.

ANSPEC-002 §7 is the technical specification of LogitPort; ANSPEC-002 §7.1
already records the Bible-alignment argument that this article makes
constitutional. The research artifact at docs/research/ALTIMIT-real-world-
implementation.md §G Recommendation 9 named "kernel-mediated inference
governance" as a frontier research direction; §J named "the Morganna
pattern" — the failure mode of an OS that embeds a governance/persona
substrate at its core. Article 6.6 is how the project resists that pattern
without giving up the substrate's value.

Implementation phasing: LogitPort is Phase 7+ research-frontier per
ANSPEC-002 §7. Article 6.6 is Bible-level forward-looking discipline; it
asserts what the kernel will do, not what it currently does. No code change
accompanies this amendment. Phase 5 ships Tensor/Model/InferenceContext
capability rights (ANSPEC-002 §3-§5) without LogitPort; Phase 7+ adds
LogitPort under this article's discipline.
```

Versioning: Bible v1.0.3 → v1.0.4. Footer's `Last amended` line updates to `2026-05-06 (Article 6.6 added, inference-governance-hooks clause)`.

---

## 4. The diff

Two files change:

### `docs/bible.md`

Insert the two paragraphs of §1 above between Article 6.5 and Article 7's `Article 7 — Memory Model` heading. Update the footer:

```
Version: 1.0.4
...
Last amended: 2026-05-06 (Article 6.6 added, inference-governance-hooks clause)
```

### `docs/specs/altimitneo-ai-capability-semantics-spec.md` (ANSPEC-002)

ANSPEC-002 §7.1 currently reads: "Bible alignment: Article 23.1 (no system AI assistant/chatbot persona) is preserved because LogitPort is a mechanism, not a persona; Article 23.3 (no embedded specific model in the kernel) is preserved because LogitPort observes a model loaded by userspace, not a kernel-resident one; Article 12.5 (AI models are a security boundary) composes — LogitPort is one of the mechanisms by which the kernel enforces the boundary."

That sentence remains correct, but with 6.6 in place it should also reference 6.6. Suggested one-line addition at the end of the existing paragraph:

> "Article 6.6 — added 2026-05-06 — establishes the constitutional shape under which LogitPort sits: the kernel exposes governance hooks; userspace builds policy."

That's a single-sentence ANSPEC-002 update; it does not require a new spec version (the spec doesn't *change* substantively, it just gains a cross-reference to a now-existing Bible clause). Optional: bump ANSPEC-002 to v0.1.1 with `Last revised: 2026-05-06` if PM prefers explicit version-bumping for cross-reference updates. My recommendation is to leave ANSPEC-002 at v0.1.0 — the cross-reference is descriptive, not a spec change.

### Nothing else changes

- ADR-013 is unaffected. (LogitPort is Phase 7+; ADR-013 is Phase 2.)
- ANSPEC-001 is unaffected. (Article 6.6 names hooks the AI subsystem provides; the AI subsystem's structural shape per ANSPEC-001 doesn't change.)
- No ADR is needed to introduce the article. Article 27 amendment process is sufficient.
- No code is touched. This is doc-only, governance-class work.

---

## 5. Recommended commit shape

Single commit on `main`. Suggested commit message:

```
docs: Bible amendment — Article 6.6 inference-governance-hooks clause

Article 6.6 specifies that the kernel exposes inference-governance hooks
(LogitPort being the canonical example, per ANSPEC-002 §7) but does not
implement governance policy. The clause is the positive form of the
prohibitions in Articles 23.1 and 23.3: a kernel-default governance policy
is a system persona by another name and an embedded specific model by
another name; both are already forbidden. 6.6 makes the consequence
explicit and gives future PRs a Bible clause to argue against if they
propose a built-in safety filter or alignment layer.

ANSPEC-002 §7.1 gains a one-sentence cross-reference to 6.6.

Bible: v1.0.3 → v1.0.4. ANSPEC-002 unchanged at v0.1.0 (cross-reference
is descriptive, not a spec change).

[full rationale per Article 27 below]

Rationale (Article 27):
[paste §3 of the dispatch here]
```

---

## 6. Two questions for PM before commit

**Q1.** Does the second paragraph of the article text feel necessary, or should 6.6 be just the first paragraph (the positive direction-setter), with the prohibition-grounding moved entirely into the rationale section?

- **Keep both paragraphs (current draft):** the article is self-contained; readers see *why* 6.6 exists without leaving the Bible.
- **First paragraph only:** the article matches the terse style of 6.1–6.5 better; the rationale is in the commit body where Article 27 amendments are documented anyway.

My recommendation: **keep both paragraphs.** Article 23 itself is unusually explanatory ("we will not ship... we will not collect... we will not embed... we will not block on..."), so 6.6 having a "why" paragraph is consistent. But I see the case for the terser version; reasonable PM call either way.

**Q2.** Does PM want to expand 6.6 to name *additional* hooks beyond LogitPort (e.g., "trace ports," "inference-completion observers"), or is naming LogitPort alone sufficient?

- **LogitPort only (current draft):** Bible names what's in ANSPEC-002 today; future hooks slot in under 6.6's general direction without article amendment.
- **Enumerate a wider class:** "LogitPort, trace observers, completion auditors, and similar capability-gated mechanisms..." — names a class, future-proofs the article, but commits to a vocabulary that doesn't exist yet in any spec.

My recommendation: **LogitPort only.** It's the only such hook with a specification today; future hooks land under 6.6's general direction without needing the article re-amended. If a future hook is structurally different from LogitPort in a way that makes the 6.6 framing wrong, *that's* the moment to amend 6.6 — premature enumeration risks committing to a class boundary that the actual second hook doesn't fit cleanly.

---

## 7. If PM wants the terser version

Trimmed Article 6.6 — first paragraph only, no second paragraph:

```
6.6 The kernel exposes inference-governance hooks but does not implement governance policy. Specifically, the AI subsystem provides capability-gated mechanisms — LogitPort being the canonical example (ANSPEC-002 §7) — by which a holding process can observe, veto, or reweight the logit distribution of an inference context before sampling. The mechanism is the kernel's contribution; the policy is the application's. The kernel does not ship a default safety filter, alignment layer, or content classifier. It ships the primitives by which userspace builds those things, capability-gated like every other kernel object.
```

Six sentences. Tighter. The 23.1/23.3 grounding lives entirely in the commit's rationale section. PM picks.

---

**End of draft.**

*This is a Bible amendment, not a coder dispatch. PM commits directly to main when ready, after answering Q1 and Q2 above.*
