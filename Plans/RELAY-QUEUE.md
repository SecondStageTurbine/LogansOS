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
RELAY 272 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-02-relays-272-273.md | Section 272 (#42 item 1 re-check; RETURN `Plans/RETURN-2026-11-02-pr42-recheck-jarvis.md`)
RELAY 273 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relays-272-273.md | Section 273 (#56 pipe step 2 design; RETURN `Plans/RETURN-2026-11-02-pr56-pipe2-review-dexter.md`)
RELAY 274 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-02-relays-274-276.md | Section 274 (#57 re-check; RETURN `Plans/RETURN-2026-11-02-pr57-recheck-chad.md`)
RELAY 275 | chad | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-02-relays-274-276.md | Section 275 (#17 item 1 re-check; RETURN `Plans/RETURN-2026-11-02-pr17-recheck-chad.md`)
RELAY 276 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relays-274-276.md | Section 276 (#51 r2 re-check; RETURN `Plans/RETURN-2026-11-02-pr51-recheck-dexter.md`)
RELAY 277 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-02-relays-277-281.md | Section 277 (#16 item 1; RETURN `Plans/RETURN-2026-11-02-pr16-recheck4-janus.md`)
RELAY 278 | chad | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-02-relays-277-281.md | Section 278 (#18 item 2; RETURN `Plans/RETURN-2026-11-02-pr18-recheck-chad.md`)
RELAY 279 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-02-relays-277-281.md | Section 279 (#15 items 3/6; RETURN `Plans/RETURN-2026-11-02-pr15-recheck2-chad.md`)
RELAY 280 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-02-relays-277-281.md | Section 280 (#57 re-check 2; RETURN `Plans/RETURN-2026-11-02-pr57-recheck2-chad.md`)
RELAY 281 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relays-277-281.md | Section 281 (#56 F1/F2; RETURN `Plans/RETURN-2026-11-02-pr56-recheck-dexter.md`)
RELAY 282 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-setup-menu-think-dexter.md | Section "Relay 282" (setup menu THINK; RETURN `Plans/RETURN-2026-11-02-setup-menu-think-dexter.md`)
RELAY 283 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relay-283.md | Section "Relay 283" (#45/#47 re-check; RETURN `Plans/RETURN-2026-11-02-pr45-pr47-recheck-dexter.md`)
RELAY 284 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-02-relay-284.md | Section "Relay 284" (setup-menu THINK design read; RETURN `Plans/RETURN-2026-11-02-setup-menu-review-jarvis.md`)
RELAY 285 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relay-285.md | Section "Relay 285" (setup-menu THINK revision; RETURN appended to `Plans/RETURN-2026-11-02-setup-menu-think-dexter.md`)
RELAY 286 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-02-relays-286-289.md | Section 286 (#59 tz slice 1; RETURN `Plans/RETURN-2026-11-02-pr59-tz-review-dexter.md`)
RELAY 287 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-02-relays-286-289.md | Section 287 (#60 clock/RTC; RETURN `Plans/RETURN-2026-11-02-pr60-clock-review-janus.md`)
RELAY 288 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-02-relays-286-289.md | Section 288 (#61 key repeat; RETURN `Plans/RETURN-2026-11-02-pr61-repeat-review-jarvis.md`)
RELAY 289 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-02-relays-286-289.md | Section 289 (#62 password message; RETURN `Plans/RETURN-2026-11-02-pr62-password-review-jarvis.md`)
RELAY 290 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-03-relays-290-295.md | Section 290 (#59 re-check; RETURN `Plans/RETURN-2026-11-03-pr59-recheck-dexter.md`)
RELAY 291 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-290-295.md | Section 291 (#60 re-check; RETURN `Plans/RETURN-2026-11-03-pr60-recheck-janus.md`)
RELAY 292 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-03-relays-290-295.md | Section 292 (#61 re-check; RETURN `Plans/RETURN-2026-11-03-pr61-recheck-jarvis.md`)
RELAY 293 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-290-295.md | Section 293 (#15 re-check; RETURN `Plans/RETURN-2026-11-03-pr15-recheck3-chad.md`)
RELAY 294 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-03-relays-290-295.md | Section 294 (#7 review; RETURN `Plans/RETURN-2026-11-03-pr7-quiesce-review-janus.md`)
RELAY 295 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-03-relays-290-295.md | Section 295 (#58 review; RETURN `Plans/RETURN-2026-11-03-pr58-drag-review-jarvis.md`)
RELAY 296 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relay-296.md | Section 296 (#60 restart-hint re-check; RETURN `Plans/RETURN-2026-11-03-pr60-recheck2-janus.md`)
RELAY 297 | janus | GPT-6-Luna | high | Plans/DISPATCH-2026-11-03-relays-297-299.md | Section 297 (#65 review; RETURN `Plans/RETURN-2026-11-03-pr65-review-janus.md`)
RELAY 298 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-03-relays-297-299.md | Section 298 (#66 review; RETURN `Plans/RETURN-2026-11-03-pr66-review-jarvis.md`)
RELAY 299 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-03-relays-297-299.md | Section 299 (#67 review; RETURN `Plans/RETURN-2026-11-03-pr67-review-chad.md`)
RELAY 300 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-03-relay-300.md | Section 300 (NeoFS find_cached_block loop; RETURN `Plans/RETURN-2026-11-03-neofs-cache-loop-review-jarvis.md`)
RELAY 301 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relay-301.md | Section 301 (#69 NeoFS loop re-check, items 3/4; RETURN `Plans/RETURN-2026-11-03-pr69-recheck-jarvis.md`)
RELAY 302 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-302-305.md | Section 302 (#7 re-check; RETURN `Plans/RETURN-2026-11-03-pr7-recheck-janus.md`)
RELAY 303 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-03-relays-302-305.md | Section 303 (#56 pipe cross-CPU review; RETURN `Plans/RETURN-2026-11-03-pr56-pipe-review-chad.md`)
RELAY 304 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-03-relays-302-305.md | Section 304 (#68 F8 review; RETURN `Plans/RETURN-2026-11-03-pr68-f8-review-jarvis.md`)
RELAY 305 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-302-305.md | Section 305 (#64 setup-menu THINK re-check; RETURN `Plans/RETURN-2026-11-03-pr64-recheck-jarvis.md`)
RELAY 306 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-306-308.md | Section 306 (#68 F8 re-check; RETURN `Plans/RETURN-2026-11-03-pr68-recheck-jarvis.md`)
RELAY 307 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-306-308.md | Section 307 (#66 SWGL re-check; RETURN `Plans/RETURN-2026-11-03-pr66-recheck-jarvis.md`)
RELAY 308 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-306-308.md | Section 308 (#67 pthreads re-check; RETURN `Plans/RETURN-2026-11-03-pr67-recheck-chad.md`)
RELAY 309 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-03-relays-309-310.md | Section 309 (#8 PowerControl cap + SYS_POWER; RETURN `Plans/RETURN-2026-11-03-pr8-power-cap-review-janus.md`)
RELAY 310 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-03-relays-309-310.md | Section 310 (#14 power menu; RETURN `Plans/RETURN-2026-11-03-pr14-power-menu-review-jarvis.md`)
RELAY 311 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relay-311.md | Section 311 (#8 re-check; RETURN `Plans/RETURN-2026-11-03-pr8-recheck-janus.md`)
RELAY 312 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-312-314.md | Section 312 (#58 drag re-check; RETURN `Plans/RETURN-2026-11-03-pr58-recheck-jarvis.md`)
RELAY 313 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-03-relays-312-314.md | Section 313 (#56 pipe item-5 re-check; RETURN `Plans/RETURN-2026-11-03-pr56-recheck-chad.md`)
RELAY 314 | janus | GPT-6-Luna | high | Plans/DISPATCH-2026-11-03-relays-312-314.md | Section 314 (#70 setup menu S1 review; RETURN `Plans/RETURN-2026-11-03-pr70-setup-s1-review-janus.md`)
RELAY 315 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 315 (#70 re-check; RETURN `Plans/RETURN-2026-11-08-pr70-recheck-janus.md`)
RELAY 316 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 316 (#58 re-check 2; RETURN `Plans/RETURN-2026-11-08-pr58-recheck2-jarvis.md`)
RELAY 317 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 317 (#14 Enter-only check; RETURN `Plans/RETURN-2026-11-08-pr14-check-jarvis.md`)
RELAY 318 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 318 (#75 destructor cap; RETURN `Plans/RETURN-2026-11-08-pr75-review-chad.md`)
RELAY 319 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 319 (#73 NeoFS index THINK; RETURN `Plans/RETURN-2026-11-08-pr73-design-dexter.md`)
RELAY 320 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 320 (#74 Raven THINK; RETURN `Plans/RETURN-2026-11-08-pr74-design-dexter.md`)
RELAY 321 | janus | GPT-6-Luna | high | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 321 (#76 file-viewer survey; RETURN `Plans/RETURN-2026-11-08-pr76-survey-review-janus.md`)
RELAY 322 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relay-322.md | Section 322 (#72 stdio-at-spawn design read; RETURN `Plans/RETURN-2026-11-08-pr72-design-dexter.md`)
RELAY 323 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relay-323-quant-drift-dexter.md | Build the quantization-drift check (Q4_K_M vs full precision; bars A and B fixed in the dispatch); RETURN `Plans/RETURN-2026-11-08-quant-drift-dexter.md`
RELAY 324 | jarvis | GPT-5.6-Luna | medium | Plans/DISPATCH-2026-11-08-relays-315-321.md | Section 324 (#14 add-on: test commit `3d55c856`, head `ec3de81c`; RETURN `Plans/RETURN-2026-11-08-pr14-check2-jarvis.md`)
RELAY 325 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-325-328.md | Section 325 (#72 re-check at `e49233bf`; RETURN `Plans/RETURN-2026-11-08-pr72-recheck-dexter.md`)
RELAY 326 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-325-328.md | Section 326 (#73 re-check at `89127f21`; RETURN `Plans/RETURN-2026-11-08-pr73-recheck-dexter.md`)
RELAY 327 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-325-328.md | Section 327 (#74 re-check at `8ccd6751`; RETURN `Plans/RETURN-2026-11-08-pr74-recheck-dexter.md`)
RELAY 328 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-11-08-relays-325-328.md | Section 328 (#71 first review at `50ca36e2`; RETURN `Plans/RETURN-2026-11-08-pr71-review-chad.md`)
RELAY 329 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-08-relays-329-332.md | Section 329 (browser rungs #77–#80 bundle; RETURN `Plans/RETURN-2026-11-08-pr77-80-review-janus.md`)
RELAY 330 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-08-relays-329-332.md | Section 330 (#81 Servo layout L0–L4; RETURN `Plans/RETURN-2026-11-08-pr81-review-jarvis.md`)
RELAY 331 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-11-08-relays-329-332.md | Section 331 (#82 NeoFS cache index; RETURN `Plans/RETURN-2026-11-08-pr82-review-chad.md`)
RELAY 332 | jarvis | GPT-5.6-Luna | medium | Plans/DISPATCH-2026-11-08-relays-329-332.md | Section 332 (#83 batch-plan stderr; RETURN `Plans/RETURN-2026-11-08-pr83-review-jarvis.md`)
RELAY 333 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-333-334-quant-drift.md | Section 333 (quant drift: Q8_0 control C, per-position logits, divergence gaps; RETURN `Plans/RETURN-2026-11-08-quant-drift-control-dexter.md`)
RELAY 334 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-08-relays-333-334-quant-drift.md | Section 334 (quant-drift harness review, after 333; RETURN `Plans/RETURN-2026-11-08-quant-drift-review-jarvis.md`)
RELAY 335 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 335 (#14 = Sections 317 + 324, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr14-check-dexter.md`)
RELAY 336 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 336 (#70 + #58 re-checks = Sections 315 + 316, rerouted; RETURNs `Plans/RETURN-2026-11-08-pr70-recheck-dexter.md`, `...pr58-recheck-dexter.md`)
RELAY 337 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 337 (#83 = Section 332, rerouted; RETURN `Plans/RETURN-2026-11-08-pr83-review-dexter.md`)
RELAY 338 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 338 (#73/#82 item-5 re-check; RETURN `Plans/RETURN-2026-11-08-pr73-recheck2-dexter.md`)
RELAY 339 | prometheus | Qwen | - | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 339 (#76 survey = Section 321, rerouted; RETURN `Plans/RETURN-2026-11-08-pr76-survey-review-prometheus.md`)
RELAY 340 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 340 (#71 = Section 328, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr71-review-dexter.md`)
RELAY 341 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 341 (#82 = Section 331, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr82-review-dexter.md`)
RELAY 342 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-342-343-quant-drift.md | Section 342 (quant drift: dequantized-f32 test of the 37 bar-A gaps; Q4/Q5/Q6/Q8 ladder vs F16; RETURN `Plans/RETURN-2026-11-08-quant-drift-ladder-dexter.md`)
RELAY 343 | prometheus | Qwen | - | Plans/DISPATCH-2026-11-08-relays-342-343-quant-drift.md | Section 343 (quant-drift harness review, rerouted from Jarvis 334; RETURN `Plans/RETURN-2026-11-08-quant-drift-review-prometheus.md`)
RELAY 344 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 344 (#75 = Section 318, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr75-review-dexter.md`)
RELAY 345 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-08-relays-329-332.md | Section 345 (#85 L5 first page in a window; RETURN `Plans/RETURN-2026-11-08-pr85-review-jarvis.md`)
RELAY 346 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 346 (#77 = Section 329's #77 part, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr77-review-dexter.md`)
RELAY 347 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 347 (#86 CLOCK_REALTIME from 1970; RETURN `Plans/RETURN-2026-11-08-pr86-review-dexter.md`)
RELAY 348 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 348 (#78 = Section 329's #78 part, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr78-review-dexter.md`)
RELAY 349 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 349 (#10 SpiderMonkey THINK design read; RETURN `Plans/RETURN-2026-11-08-pr10-design-dexter.md`)
RELAY 350 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 350 (#79 = Section 329's #79 part, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr79-review-dexter.md`)
RELAY 351 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 351 (#88 neo-crt for SpiderMonkey; RETURN `Plans/RETURN-2026-11-08-pr88-review-dexter.md`)
RELAY 352 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 352 (#80 = Section 329's #80 part, rerouted from Codex; RETURN `Plans/RETURN-2026-11-08-pr80-review-dexter.md`)
RELAY 353 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 353 (#89 C3c-1 product-build THINK; RETURN `Plans/RETURN-2026-11-08-pr89-design-dexter.md`)
RELAY 354 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md | Section 354 (#87 SpiderMonkey vendor + generator; RETURN `Plans/RETURN-2026-11-08-pr87-review-dexter.md`)
RELAY 355 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-355-358.md | Section 355 (#10 re-check at `65da764c`; RETURN `Plans/RETURN-2026-11-08-pr10-recheck-dexter.md`)
RELAY 356 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-355-358.md | Section 356 (#88 re-check at `849d2325`; RETURN `Plans/RETURN-2026-11-08-pr88-recheck-dexter.md`)
RELAY 357 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-08-relays-355-358.md | Section 357 (#90 Arthur to Q8_0; RETURN `Plans/RETURN-2026-11-08-pr90-review-chad.md`)
RELAY 358 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-355-358.md | Section 358 (#84 fix: the .neom run's corpus window; RETURN `Plans/RETURN-2026-11-08-quant-drift-fix-dexter.md`)
RELAY 359 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 359 (#85 item 4 re-check at `0757ab02`; RETURN `Plans/RETURN-2026-11-08-pr85-recheck-jarvis.md`)
RELAY 360 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 360 (#81 re-check at `5f70ce34`; RETURN `Plans/RETURN-2026-11-08-pr81-recheck-jarvis.md`)
RELAY 361 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 361 (#87 re-check at `4f7c4e75`; RETURN `Plans/RETURN-2026-11-08-pr87-recheck-dexter.md`)
RELAY 362 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 362 (#89 re-check at `f52503e7`; RETURN `Plans/RETURN-2026-11-08-pr89-recheck-dexter.md`)
RELAY 363 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 363 (#10 item 3 re-check at `10258cc1`; RETURN `Plans/RETURN-2026-11-08-pr10-recheck2-dexter.md`)
RELAY 364 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 364 (#84 corpus-window re-check at `eb28b7fb`; RETURN `Plans/RETURN-2026-11-08-quant-drift-recheck-jarvis.md`)
RELAY 365 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 365 (#87 generator commits after 361, at `3b85ae50`; RETURN `Plans/RETURN-2026-11-08-pr87-recheck2-dexter.md`)
RELAY 366 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 366 (#72 stdio-at-spawn code review at `576cd862`; RETURN `Plans/RETURN-2026-11-09-pr72-review-janus.md`)
RELAY 367 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-08-relays-359-362.md | Section 367 (#91 THINK follow-up at `c0be27dd`; RETURN `Plans/RETURN-2026-11-09-pr91-design-dexter.md`)
RELAY 368 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 368 (#72 F1/F2 re-check at `fc964d65`; RETURN `Plans/RETURN-2026-11-09-pr72-recheck-janus.md`)
RELAY 369 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 369 (#92 setup S3 time zone at `46370757`; RETURN `Plans/RETURN-2026-11-09-pr92-review-chad.md`)
RELAY 370 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 370 (#93 SpiderMonkey on target, docs; RETURN `Plans/RETURN-2026-11-09-pr93-design-dexter.md`)
RELAY 371 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 371 (#72 F1 only at `70fa3e7e`; RETURN `Plans/RETURN-2026-11-09-pr72-recheck2-janus.md`)
RELAY 372 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 372 (#94 quant drift on Q8_0; RETURN `Plans/RETURN-2026-11-09-pr94-review-jarvis.md`)
RELAY 373 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 373 (#92 confirm re-check at `60c1e1e5`; RETURN `Plans/RETURN-2026-11-09-pr92-recheck-chad.md`)
RELAY 374 | jarvis | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 374 (#94 item-1 re-check at `572fb6ff`; RETURN `Plans/RETURN-2026-11-09-pr94-recheck-jarvis.md`)
RELAY 375 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 375 (#95 notes on #82; RETURN `Plans/RETURN-2026-11-09-pr95-notes-dexter.md`)
RELAY 376 | chad | GPT-6-Luna | high | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 376 (#96 stable size measurement; RETURN `Plans/RETURN-2026-11-09-pr96-review-chad.md`)
RELAY 377 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 377 (#92 all-or-nothing confirm at `5dd0e4e9`; RETURN `Plans/RETURN-2026-11-09-pr92-recheck2-chad.md`)
RELAY 378 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 378 (#96 ceilings re-baselined at `0eea4614`; RETURN `Plans/RETURN-2026-11-09-pr96-recheck-chad.md`)
RELAY 379 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 379 (#92 reconcile-from-store at `1609c752`; RETURN `Plans/RETURN-2026-11-09-pr92-recheck3-chad.md`)
RELAY 380 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md | Section 380 (#92 narrow re-check of the PM rulings at `0945e6e5`; RETURN `Plans/RETURN-2026-11-09-pr92-recheck4-chad.md`)
RELAY 381 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-09-relay-381-ap-preemption-diagnosis.md | Diagnosis: preempted user thread starves ~20 s beside non-yielding spinners on NEO_SMP=4 (#98 evidence); RETURN `Plans/RETURN-2026-11-09-ap-preemption-diagnosis-dexter.md`
RELAY 382 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-09-relays-382-384-setup-reviews.md | Section 382 (#100 setup S4 at `67be4ba2`; RETURN `Plans/RETURN-2026-11-09-pr100-review-janus.md`)
RELAY 383 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-09-relays-382-384-setup-reviews.md | Section 383 (#99 setup S5 at `8e407c15`; RETURN `Plans/RETURN-2026-11-09-pr99-review-jarvis.md`)
RELAY 384 | chad | GPT-5.6-Luna | medium | Plans/DISPATCH-2026-11-09-relays-382-384-setup-reviews.md | Section 384 (#101 setup S6a at `b33788db`; RETURN `Plans/RETURN-2026-11-09-pr101-review-chad.md`)
RELAY 385 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-10-relay-385-ot100-think-read.md | OT-100 THINK read (#103 at `a94a5e60`; RETURN `Plans/RETURN-2026-11-10-pr103-ot100-think-read-dexter.md`)
RELAY 386 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-10-relay-386-pr101-recheck.md | #101 narrow re-check at `24b59a34`; RETURN `Plans/RETURN-2026-11-10-pr101-recheck-chad.md`
RELAY 387 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md | Section 387 (#100 re-check at `8414cc5f`; RETURN `Plans/RETURN-2026-11-10-pr100-recheck-janus.md`)
RELAY 388 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md | Section 388 (#103 OT-100 THINK re-check at `9a2c97dd`; RETURN `Plans/RETURN-2026-11-10-pr103-recheck-dexter.md`)
RELAY 389 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md | Section 387, re-run (#100 re-check at `8414cc5f`; relay 387 ran 11-10 03:13 but pushed no RETURN); RETURN `Plans/RETURN-2026-11-10-pr100-recheck-janus.md` on `codex/pr100-recheck-janus`
RELAY 390 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-11-relay-390-pr101-s5-strings.md | #101 narrow check of the 5 new S5 strings at `3d994252`; RETURN `Plans/RETURN-2026-11-11-pr101-recheck2-chad.md`
RELAY 391 | dexter | DeepSeek | high | Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md | Section 387, rerouted to Dexter (#100 re-check at `8414cc5f`; Janus relays 387 and 389 both ran but pushed no RETURN); RETURN `Plans/RETURN-2026-11-12-pr100-recheck-dexter.md` on `dexter/pr100-recheck`
RELAY 392 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-12-relay-392-pr98-terminal-review.md | Section 392 (#98 terminal, cross-family code review at `76aa36d7`); RETURN `Plans/RETURN-2026-11-12-pr98-review-jarvis.md` on `codex/pr98-review-jarvis`
RELAY 393 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-12-relay-393-pr104-usb-write-review.md | Section 393 (#104 OT-100 R2, cross-family code review at `2cf42002`); RETURN `Plans/RETURN-2026-11-12-pr104-review-janus.md` on `codex/pr104-review-janus`
RELAY 394 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-12-relay-394-pr104-recheck.md | #104 OT-100 R2 re-check of F1-F3 at `c7235a49`; RETURN `Plans/RETURN-2026-11-12-pr104-recheck-janus.md` on `codex/pr104-recheck-janus`
RELAY 395 | janus | GPT-6-Astra | medium | Plans/DISPATCH-2026-11-12-relays-395-397.md | Section 395 (#104 OT-100 R2 third check, F1a/F1b at `16bb8f56`); RETURN `Plans/RETURN-2026-11-12-pr104-recheck2-janus.md` on `codex/pr104-recheck2-janus`
RELAY 396 | chad | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-12-relays-395-397.md | Section 396 (#106 F0 NeoFS dir ops, AAG cross-family lens at `e2b5daff`); RETURN `Plans/RETURN-2026-11-12-pr106-review-chad.md` on `codex/pr106-review-chad`
RELAY 397 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-12-relays-395-397.md | Section 397 (#107 OT-61 C3c-1 product build at `6c5bc8b7`); RETURN `Plans/RETURN-2026-11-12-pr107-review-jarvis.md` on `codex/pr107-review-jarvis`
RELAY 398 | janus | GPT-6-Astra | medium | Plans/DISPATCH-2026-11-12-relays-398-399-rechecks.md | Section 398 (#104 OT-100 R2 round 4 at `b6ee1046`); RETURN `Plans/RETURN-2026-11-12-pr104-recheck3-janus.md` on `codex/pr104-recheck3-janus`
RELAY 399 | chad | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-12-relays-398-399-rechecks.md | Section 399 (#106 F0 re-check of F1/F2 at `e7a50407`); RETURN `Plans/RETURN-2026-11-12-pr106-recheck-chad.md` on `codex/pr106-recheck-chad`
RELAY 400 | janus | GPT-6.1-Sol | high | Plans/DISPATCH-2026-11-12-relays-400-402.md | Section 400 (#108 A3p, AAG cross-family lens at `259feac2`); RETURN `Plans/RETURN-2026-11-12-pr108-review-janus.md` on `codex/pr108-review-janus`
RELAY 401 | jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-12-relays-400-402.md | Section 401 (#109 S6b code review at `3855d53d`); RETURN `Plans/RETURN-2026-11-12-pr109-review-jarvis.md` on `codex/pr109-review-jarvis`
RELAY 402 | chad | GPT-5.6-Luna | medium | Plans/DISPATCH-2026-11-12-relays-400-402.md | Section 402 (#109 S6b Spanish wording at `3855d53d`); RETURN `Plans/RETURN-2026-11-12-pr109-wording-chad.md` on `codex/pr109-wording-chad`
RELAY 403 | chad | GPT-6-Astra | medium | Plans/DISPATCH-2026-11-12-relay-403-pr106-recheck2.md | #106 F0 re-check of F2 at `09414118` (round 3 → Astra); RETURN `Plans/RETURN-2026-11-12-pr106-recheck2-chad.md` on `codex/pr106-recheck2-chad`
RELAY 404 | jarvis | GPT-6-Luna | medium | Plans/DISPATCH-2026-11-12-relay-404-pr107-recheck.md | #107 workflows re-check (item 2) at `f323962c`; RETURN `Plans/RETURN-2026-11-12-pr107-recheck-jarvis.md` on `codex/pr107-recheck-jarvis`
RELAY 405 | chad | GPT-6-Astra | medium | Plans/DISPATCH-2026-11-12-relays-405-406.md | Section 405 (#106 F0 superblock-pin re-check at `fbd75e50`); RETURN `Plans/RETURN-2026-11-12-pr106-recheck3-chad.md` on `codex/pr106-recheck3-chad`
RELAY 406 | janus | GPT-6.1-Sol | medium | Plans/DISPATCH-2026-11-12-relays-405-406.md | Section 406 (#108 A3p CP-61 re-check at `a6b3bcf3`); RETURN `Plans/RETURN-2026-11-12-pr108-recheck-janus.md` on `codex/pr108-recheck-janus`
RELAY 407 | chad | GPT-6-Astra | medium | Plans/DISPATCH-2026-11-12-relays-407-408.md | Section 407 (#106 F0 items 1+8 at `7557b87e`); RETURN `Plans/RETURN-2026-11-12-pr106-recheck4-chad.md` on `codex/pr106-recheck4-chad`
RELAY 408 | janus | GPT-6-Astra | medium | Plans/DISPATCH-2026-11-12-relays-407-408.md | Section 408 (#108 A3p items 1/3/4 at `561b1a1d`); RETURN `Plans/RETURN-2026-11-12-pr108-recheck2-janus.md` on `codex/pr108-recheck2-janus`
RELAY 409 | Jarvis | GPT-6-Luna | high | Plans/DISPATCH-2026-11-12-relay-409-c3c2-think-read.md | design read of the OT-61 C3c-2 THINK (CP table, check 21 + control, CP-111 join under ENGINE_STATE, the JS_ShutDown gap, OOM/crash containment)
