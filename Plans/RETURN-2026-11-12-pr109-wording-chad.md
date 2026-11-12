<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 402 — Chad's Spanish wording read of S6b (#109)

**Dispatch:** `Plans/DISPATCH-2026-11-12-relays-400-402.md`, Section 402.
**Reviewed head:** `3855d53db`, `claude/setup-s6b-lang`.
**RETURN branch:** `codex/pr109-wording-chad`.
**Verdict: PASS.** The S6b Spanish is natural, neutral Latin American Spanish in the same short,
informal-*tú* imperative style as S6a. I would change no line.

## Wording review

- **Language page — PASS.** `userland/i18n-core/src/lib.rs:197-201` reads naturally: “Elige el
  idioma de la configuración y de Inicio” and “Por ahora, Arthur responde en inglés.” “Inicio”
  consistently names Home, and the second row accurately says that Arthur's replies remain in
  English.
- **Network page — PASS.** `userland/i18n-core/src/lib.rs:204-219` uses idiomatic, concise
  labels: “Red”, “Continuar sin red”, “Cable: sin enlace”, “Cable: conectado” and “Concesión”.
  “Concesión” is the ordinary Spanish term for a DHCP lease; “Cable” and “enlace” preserve the
  wired/link-layer meaning without adding ambiguity.
- **Lease-time forms — PASS.** `userland/i18n-core/src/lib.rs:220-224` correctly gives
  “queda menos de un minuto”, “quedan {} min” and “quedan {} h {} min”. The plural verb is
  appropriate for the displayed minute range, including the abbreviated “1 min” form.
- **Intro/date additions — PASS.** The S6b-marked intro wording in
  `userland/i18n-core/src/lib.rs:230-244` and `:248-268` preserves the meaning and register;
  the translated example question remains clear while command words and names stay unchanged.
  The Spanish date pattern at `:302-304` is idiomatic and correctly orders the fields as
  “miércoles 21 de octubre de 2026”.
- **Clock readout — PASS.** `userland/i18n-core/src/lib.rs:359-361` gives the natural
  parenthetical “ (zona sin definir)”, including the required leading space and the same
  “sin definir” wording used elsewhere for an unset zone.

## Requested change list

None. No Spanish line in the reviewed S6b additions or the included Network-page rows needs a
wording change.
