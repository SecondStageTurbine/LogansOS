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
RELAY 241 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-recheck-241-pr13.md | Section "Relay 241" (#13 narrow re-check; RETURN `Plans/RETURN-2026-11-01-pr13-recheck-chad.md`)
RELAY 242 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-recheck-242-pr31.md | Section "Relay 242" (#31 re-check; RETURN `Plans/RETURN-2026-11-01-pr31-recheck-dexter.md`)
RELAY 243 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-recheck-243-244.md | Section 243 (#16 re-check; RETURN `Plans/RETURN-2026-11-01-pr16-recheck-janus.md`)
RELAY 244 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-recheck-243-244.md | Section 244 (#6 re-check; RETURN `Plans/RETURN-2026-11-01-pr6-recheck-janus.md`)
RELAY 245 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-rechecks-245-250.md | Section 245 (#13 re-check 2; RETURN `Plans/RETURN-2026-11-01-pr13-recheck2-chad.md`)
RELAY 246 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-rechecks-245-250.md | Section 246 (#29 re-check; RETURN `Plans/RETURN-2026-11-01-pr29-recheck-jarvis.md`)
RELAY 247 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-01-rechecks-245-250.md | Section 247 (#30 + #23 re-check; RETURN `Plans/RETURN-2026-11-01-pr30-pr23-recheck-jarvis.md`)
RELAY 248 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-01-rechecks-245-250.md | Section 248 (#12 rev 3 re-check; RETURN `Plans/RETURN-2026-11-01-pr12-recheck-jarvis.md`)
RELAY 249 | chad | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-01-rechecks-245-250.md | Section 249 (#9 + #20 re-check; RETURN `Plans/RETURN-2026-11-01-pr9-pr20-recheck-chad.md`)
RELAY 250 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-rechecks-245-250.md | Section 250 (L2d images THINK; RETURN `Plans/RETURN-2026-11-01-l2d-images-dexter.md`)
RELAY 251 | chad | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-01-review-251-pr41.md | Section "Relay 251" (#41 design read; RETURN `Plans/RETURN-2026-11-01-pr41-review-chad.md`)
RELAY 252 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-relays-252-254.md | Section 252 (#13 re-check 3; RETURN `Plans/RETURN-2026-11-01-pr13-recheck3-chad.md`)
RELAY 253 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-relays-252-254.md | Section 253 (#16 item 1; RETURN `Plans/RETURN-2026-11-01-pr16-recheck2-janus.md`)
RELAY 254 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-relays-252-254.md | Section 254 (#41 revision; RETURN appended to `Plans/RETURN-2026-11-01-l2d-images-dexter.md`)
RELAY 255 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-01-browser-reviews-255-256.md | Section 255 (#2 neo-crt review; RETURN `Plans/RETURN-2026-11-01-pr2-neo-crt-review-jarvis.md`)
RELAY 256 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-11-01-browser-reviews-255-256.md | Section 256 (#4 Stylo review; RETURN `Plans/RETURN-2026-11-01-pr4-stylo-review-chad.md`)
RELAY 257 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-recheck-257-pr6.md | Section "Relay 257" (#6 items 2/4/5; RETURN `Plans/RETURN-2026-11-01-pr6-recheck2-janus.md`)
RELAY 258 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-01-review-258-pr43.md | Section "Relay 258" (#43 RDRAND review, priority; RETURN `Plans/RETURN-2026-11-01-pr43-rdrand-review-chad.md`)
RELAY 259 | chad | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-01-relays-259-260.md | Section 259 (#41 re-check; RETURN `Plans/RETURN-2026-11-01-pr41-recheck-chad.md`)
RELAY 260 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-relays-259-260.md | Section 260 (#45 quick-tier review; RETURN `Plans/RETURN-2026-11-01-pr45-quick-tier-review-dexter.md`)
RELAY 261 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-relays-261-263.md | Section 261 (#15 re-check; RETURN `Plans/RETURN-2026-11-01-pr15-recheck-chad.md`)
RELAY 262 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-01-relays-261-263.md | Section 262 (#16 re-check 3; RETURN `Plans/RETURN-2026-11-01-pr16-recheck3-janus.md`)
RELAY 263 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-01-relays-261-263.md | Section 263 (#46 pipe step 1 review; RETURN `Plans/RETURN-2026-11-01-pr46-pipe-review-dexter.md`)
RELAY 264 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-01-review-264-pr42.md | Section "Relay 264" (#42 term core review; RETURN `Plans/RETURN-2026-11-01-pr42-term-core-review-jarvis.md`)
RELAY 265 | chad | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-02-relays-265-268.md | Section 265 (#4 re-check; RETURN `Plans/RETURN-2026-11-02-pr4-recheck-chad.md`)
RELAY 266 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-02-relays-265-268.md | Section 266 (#2 + #29 re-checks; RETURN `Plans/RETURN-2026-11-02-pr2-pr29-recheck-jarvis.md`)
RELAY 267 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-11-02-relays-265-268.md | Section 267 (#17 + #18 FAT review; RETURN `Plans/RETURN-2026-11-02-pr17-pr18-fat-review-chad.md`)
RELAY 268 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-02-relays-268-270.md | Section 268 (#52 WHPX/XSAVE review; RETURN `Plans/RETURN-2026-11-02-pr52-whpx-review-chad.md`)
RELAY 269 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-02-relays-268-270.md | Section 269 (#53 + #54 keyboard review; RETURN `Plans/RETURN-2026-11-02-pr53-pr54-keyboard-review-jarvis.md`)
RELAY 270 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relays-268-270.md | Section 270 (#51 time-zone THINK; RETURN `Plans/RETURN-2026-11-02-pr51-tz-review-dexter.md`)
RELAY 271 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relay-271.md | Section "Relay 271" (#45 re-check + #47 review; RETURN `Plans/RETURN-2026-11-02-pr45-pr47-review-dexter.md`)
