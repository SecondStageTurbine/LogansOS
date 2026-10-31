<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Arthur: The Destination

*This is the purpose document. The [Bible](bible.md) explains **how** the system works; the [Strategic Honesty Ledger](STRATEGIC-HONESTY-LEDGER.md) records **what** has been proven. This explains **why** any of it exists — and it should be read before either of them. Hand it to a new contributor first: the Bible is the constitution, this is the reason there is anything to govern.*

*On its nature: this document states the **destination**, not a description of what ships today. Where it names a capability, it grounds that capability in the kernel primitive that makes it possible and marks honestly what is built, designed, or still ahead. For the audited line between proven, structural, and aspired, see the Ledger. For where Arthur stands right now, see the last section.*

---

LogansOS began with a simple idea.

What if the kind of trusted personal intelligence imagined in science fiction actually existed?

Not a cloud service.
Not a chatbot in a browser.
Not an assistant that requires trusting a corporation.

A personal intelligence that belongs to its owner.

Local.
Private.
Capable.
Auditable.
Bounded.

An intelligence that can help, act, remember, and assist — without requiring unlimited authority over the machine it inhabits.

**Arthur is that vision.**

The operating system exists to make Arthur trustworthy.

Every major LogansOS subsystem exists because it answers a question science fiction never had to explain:

> Who owns the model?
> Who owns the memory?
> Who grants authority?
> Who can revoke authority?
> How do we know what the assistant actually did?

The answer is the operating system.

- The **capability system** provides authority.
- The **inference subsystem** provides intelligence.
- The **audit trail** provides accountability.
- The **kernel** provides enforcement.

Arthur is not the operating system.

**Arthur is the reason the operating system exists.**

*(Constitutionally, Arthur is Article 29: an integral, on-device, capability-bounded assistant — in userland, on a model-agnostic kernel. The Bible defines the boundary; this document defines the point of it.)*

---

## The vision is ownership, not capability

It is tempting to define Arthur against the assistants that exist today — more private than one, more bounded than another. That framing is a trap, because it ties the vision to a moving target. If a lab ships a smarter model next year, the vision must not change. If a vendor ships a slicker assistant next year, the vision must not change.

So the vision is not:

> "Our AI is smarter."

The vision is:

> **"Our AI belongs to you."**

A smarter model is a feature someone else can ship next quarter. An intelligence that is *yours* — that runs on your hardware, answers only to your grants, keeps your data because there is no wire for it to leave on, and can prove what it did — is a different kind of thing entirely. It is not obsoleted by a better model. It gets *better* when you load a better model into it, because the model is yours to choose.

That is the durable vision. It survives every competitor's roadmap, because it competes on an axis they are not even on: not *how capable* the intelligence is, but *whose it is.*

---

## Model proposes, capability disposes

Every assistant you have used works one way:

> **The model decides.**

It decides what to answer, what tool to call, what file to open, what command to run. Its judgment is the only thing standing between its output and your system. We make it safer by making the model *better behaved* — by training, by guardrails, by hoping.

Arthur works a different way:

> **The model proposes. Authority is separately enforced.**

Arthur's intelligence emits an *intent* — "read this file," "send this message," "run this task." That intent is a **proposal**, not an action. Before anything happens, the kernel's capability system checks whether Arthur was granted the authority for it. If it was, the action proceeds. If it wasn't, the action is refused — and Arthur tells you it was refused.

The model never holds the keys. It asks; a separate system that does not run on hope decides.

This is the whole product in four words, and it is understandable to an engineer and to someone who has never opened a terminal:

> **Model proposes, capability disposes.**

What that buys the person using it: you can hand Arthur the authority to do *one* thing and know — not trust, *know* — that it cannot do anything else. You can take that authority back in the middle of a task. And you can see exactly what it used. Delegation you can trust, because the bounds are enforced, not promised.

---

## What "belongs to you" actually means — mechanically

"Belongs to you" is not a slogan. It is a set of properties the operating system enforces. Each is real because a specific kernel primitive makes it real:

- **It answers only to your grants.** Arthur reaches exactly what you granted — a derived capability it cannot exceed — and nothing else. *"Organize my photos and touch nothing else"* is an enforced boundary, not an instruction the model might disregard. *(The capability system — derive / grant / revoke with cascade — is built.)*

- **You can take authority back instantly.** Revoke a grant mid-task and the cascade removes every authority Arthur derived from it. Trust you can withdraw is trust you can afford to extend. *(Built; verified by construction — adversarial log, Probe B.)*

- **Your data stays because there is no wire.** Arthur's intelligence runs as a kernel operation — a syscall, not a network call. Privacy here is not a policy you are asked to trust; it is the *absence of a path* for your data to leave. *(On-device inference is constitutional; the syscall path is built; first real-model generation is the current milestone.)*

- **The mind is yours to choose.** Arthur is model-agnostic. Run the model you want, swap it for a better one when it arrives, inspect it. Arthur is never obsoleted by a competitor's model — it *adopts* it. *(The model importer is built; live hot-swap is designed — ADR-049.)*

- **You can see what it did.** Because every action is a capability-checked kernel operation, *"what did Arthur actually access?"* is answered by the kernel, not by trusting an application's self-report. *(The mechanism is built; Arthur's action surface is ratified design.)*

- **It cannot become an all-powerful process.** A misbehaving or crashing model cannot corrupt or take down the system that governs it — the assistant is contained by construction. *(Fault containment, invariant I-14, is silicon-proven.)*

Together these are the thing science fiction implied but never had to explain: an intelligence that is genuinely *part of the machine* — and an intelligence you never have to fear, because the machine bounds it.

---

## Arthur, by phase

The destination, unfolding. This is a product trajectory, not a committed schedule — and it is the honest category to keep it in (Phase 9 is researched-and-deferred; Phase 10 is not yet formally planned). The Ledger marks the line between this vision and what is proven.

**Phase 8 — A local mind that works with no cloud.**
Arthur exists and converses, generating from a real model entirely on-device, offline-capable, and takes its first capability-bounded actions — reading what you granted, refusing what you didn't, telling you which it did. *We are at this threshold now: the substrate is nearly ready, first real-model generation is the current milestone, and Arthur's userland is ratified design. The doorway — not yet through it.*

**Phase 9 — Useful, not a demo.**
A genuinely capable model runs at usable speed on ordinary hardware, and Arthur's hands widen: a richer capability-gated action surface, every action still bounded and revocable. Arthur stops feeling like a toy and starts being help.

**Phase 10 — Part of the machine, and trusted because you can see and bound exactly what it does.**
Arthur as an agent with delegated, revocable, audited authority: multi-step tasks scoped to what you granted, the right model for each task, the full *model-proposes-capability-disposes* loop as something you operate and watch. AltimitOS, realized — with the foundation underneath that makes the trust real.

---

## Where Arthur is today — the honest line

Arthur is not running yet. No one has talked to Arthur.

What exists is the foundation: the capability system, kernel-governed inference computing on real model weights, hardware-enforced memory isolation proven on silicon, and a ratified design for Arthur's userland. The intelligence reaches its first words at the current milestone — real-model generation — and the assistant is built on top of that after.

This document is the *destination*, stated plainly so it cannot quietly drift. For the audited boundary between what is proven, what is true by construction, and what is still a bet, read the **Strategic Honesty Ledger**. For how the machine works, read the **Bible**. This document exists to answer the one question neither of those does:

> **Why does any of this exist?**

Because someone wanted the kind of trusted personal intelligence that science fiction implied should exist — and found that the hard part was never the intelligence. The hard part was everything that makes an intelligence *trustworthy*: who owns it, what bounds it, how you grant and revoke its authority, how you know what it did.

That is what the operating system is.

We did not wander away from the dream. We spent the first chapter building the things the dream never had to explain.

Now we are close enough to write it down.
