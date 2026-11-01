<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay queue (read by `scripts/relay-runner.sh` on J.c.'s PC)

The PM appends one line per relay. The runner runs each new id once, through Codex in that coder's worker folder;
the coder pushes its RETURN branch as usual. Format, one per line, starting `RELAY`:

`RELAY <id> | <coder folder> | <model> | <effort> | <dispatch path on main> | <which section of it>`

`<model>` is a Codex model name (for example `GPT-6.1-Sol`), `DeepSeek` for dexter or sage (run through
`dsh headless`), or `Qwen` for prometheus (OpenCode, `strata/qwen3.8-flash-next-iq3_s`); for those two the effort field
is ignored.

## Queue

RELAY 194 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-30-dexter-sdk-abi-and-size-budgets.md | Task 1 (the SDK-ABI refresh), then Task 2 (CI size budgets): open one PR per task and post the RETURN (dexter) comment on each
RELAY 195 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-30-b33-b34-reviews.md | Section 195
RELAY 196 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-30-b33-b34-reviews.md | Section 196
RELAY 197 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-30-b33-b34-reviews.md | Section 197
RELAY 198 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-30-b33-b34-reviews.md | Section 198
RELAY 199 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-10-30-b33-b34-reviews.md | Section 199
RELAY 200 | prometheus | Qwen | - | Plans/DISPATCH-2026-10-30-prometheus-browser-prep.md | Section 200
RELAY 201 | prometheus | Qwen | - | Plans/DISPATCH-2026-10-30-prometheus-browser-prep.md | Section 201
RELAY 202 | prometheus | Qwen | - | Plans/DISPATCH-2026-10-30-prometheus-browser-prep.md | Section 202
RELAY 203 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-b35-b36-reviews.md | Section 203
RELAY 204 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-b35-b36-reviews.md | Section 204
RELAY 205 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-10-31-b35-b36-reviews.md | Section 205
RELAY 206 | chad | GPT-5.6-Luna | medium | Plans/DISPATCH-2026-10-31-b35-b36-reviews.md | Section 206
RELAY 207 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-b36-bounces.md | Section 207
RELAY 208 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-b36-bounces.md | Section 208
RELAY 209 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-10-31-b38-b39-reviews.md | Section 209
RELAY 210 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-10-31-b38-b39-reviews.md | Section 210
RELAY 211 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-b38-b39-reviews.md | Section 211
RELAY 212 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-31-b38-b39-reviews.md | Section 212
RELAY 213 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-b38-bounces-and-user-copy-fault.md | Section 213
RELAY 214 | janus | GPT-6-Luna | high | Plans/DISPATCH-2026-10-31-b35-b36-reviews.md | Section 205 (re-queued from 205: answer it as Janus, RETURN `Plans/RETURN-2026-10-31-ot61-c2.3b-review-janus.md`)
RELAY 215 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 215
RELAY 216 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 216
RELAY 217 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 217
RELAY 218 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 218
RELAY 219 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-10-31-b42-review-chad.md | Section 219
RELAY 220 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 215 (re-queued: 215 never returned; RETURN `Plans/RETURN-2026-10-31-b38-recheck2-chad.md`)
RELAY 221 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 216 (re-queued: 216 never returned; RETURN `Plans/RETURN-2026-10-31-ot92-cache-recheck-janus.md`)
RELAY 222 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 217 (re-queued: 217 never returned; RETURN `Plans/RETURN-2026-10-31-l71-boot-time-recheck-jarvis.md`)
RELAY 223 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-rechecks-215-218.md | Section 218 (re-queued: 218 never returned; #247 is now 272bc85a and #253 23a8d98a; RETURN `Plans/RETURN-2026-10-31-k1b-l21-recheck-dexter.md`)
RELAY 224 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-10-31-m910q-setup-chad.md | Section 224
RELAY 226 | dexter | DeepSeek | high | Plans/DISPATCH-2026-10-31-seats-resume-on-public.md | Section "Relay 226" (Lane 2a, miniz_oxide; RETURN `Plans/RETURN-2026-10-31-l2a-miniz-oxide-dexter.md`)
RELAY 227 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md | Section 227 (#13 review; RETURN `Plans/RETURN-2026-11-01-pr13-user-copy-review-chad.md`)
RELAY 228 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md | Section 228 (#6 review; RETURN `Plans/RETURN-2026-11-01-pr6-stop-cpus-review-janus.md`)
RELAY 229 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md | Section 229 (#23 review; RETURN `Plans/RETURN-2026-11-01-pr23-msc-review-jarvis.md`)
RELAY 230 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md | Section 230 (Lane 2b; RETURN `Plans/RETURN-2026-11-01-l2b-image-decode-dexter.md`)
RELAY 231 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-01-reviews-231-233.md | Section 231 (#15 review; RETURN `Plans/RETURN-2026-11-01-pr15-client-table-review-chad.md`)
RELAY 232 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-01-reviews-231-233.md | Section 232 (#16 review; RETURN `Plans/RETURN-2026-11-01-pr16-boot-time-review-janus.md`)
RELAY 233 | jarvis | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-01-reviews-231-233.md | Section 233 (#29 review; RETURN `Plans/RETURN-2026-11-01-pr29-fetch-broker-review-jarvis.md`)
RELAY 234 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-l2c-decoder-fuzz-dexter.md | Section "Relay 234" (Lane 2c; RETURN `Plans/RETURN-2026-11-01-l2c-decoder-fuzz-dexter.md`)
RELAY 235 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-think-reviews-235-237.md | Section 235 (#31 tiered-gate THINK; RETURN `Plans/RETURN-2026-11-01-pr31-tiered-gate-review-dexter.md`)
RELAY 236 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-01-think-reviews-235-237.md | Section 236 (#12 terminal THINK; RETURN `Plans/RETURN-2026-11-01-pr12-terminal-think-review-jarvis.md`)
RELAY 237 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-11-01-think-reviews-235-237.md | Section 237 (#9 + #20 storage THINKs; RETURN `Plans/RETURN-2026-11-01-pr9-pr20-storage-thinks-review-chad.md`)
RELAY 238 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-01-review-238-pr30.md | Section "Relay 238" (#30 review; RETURN `Plans/RETURN-2026-11-01-pr30-msc-bind-review-jarvis.md`)
RELAY 239 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-l1d-coreutils-survey-dexter.md | Section "Relay 239" (L1d survey THINK; RETURN `Plans/RETURN-2026-11-01-l1d-coreutils-survey-dexter.md`)
RELAY 240 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-l1b-kibi-survey-dexter.md | Section "Relay 240" (L1b kibi survey THINK; RETURN `Plans/RETURN-2026-11-01-l1b-kibi-survey-dexter.md`)
