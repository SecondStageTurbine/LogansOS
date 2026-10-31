#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Summarize a `cargo neo-test` log for the QEMU capacity trial (qemu-trial.yml).

Reads the xtask log (stdin or a path) and, optionally, the resource sampler's
peaks file. Prints, as GitHub-flavoured markdown:

- one row per QEMU attempt: scenario, attempt, seconds, budget, headroom
  (budget / seconds) and result (exit status or TIMEOUT);
- each scenario's verdict (PASS, PASS after retry, or FAIL);
- the tightest headroom, the timeouts, and the scenarios under 1.5x headroom
  (the 2026-08-02 decision measured 1.5-2.1x on the main box for the
  tightest scenario);
- the sampler's peaks (QEMU RSS, memory in use, disk).

It only reads what xtask already prints:
  [xtask] neo-test: `<label>` qemu elapsed=<s>s status=<status> budget=<s>s
  [xtask] neo-test: `<label>` qemu elapsed=<s>s TIMEOUT budget=<s>s
  [xtask] neo-test: `<scenario>` PASS
  [xtask] neo-test: `<scenario>` ... PASS after retry attempt <n>/<m>
  [xtask] neo-test meta: verifying runner catches `<scenario>` as FAIL
  [xtask] neo-test meta: deliberate failure was detected

The meta-test boots a scenario that must FAIL (`expect_pass_but_fail`, to
prove the runner catches a failure): its verdict is "expected FAIL", and it
counts as a failure only if the meta-test's own success line is missing.
"""

import re
import sys

ATTEMPT = re.compile(
    r"neo-test: `(?P<label>[^`]+)` qemu elapsed=(?P<secs>[0-9.]+)s "
    r"(?:status=(?P<status>.*?)|(?P<timeout>TIMEOUT)) budget=(?P<budget>[0-9]+)s"
)
LAUNCH = re.compile(
    r"neo-test: launching QEMU for `(?P<label>[^`]+)` \(serial: [^)]*?"
    r"(?:\.attempt(?P<attempt>[0-9]+))?(?:\.boot(?P<boot>[0-9]+))?\.serial\.log\)"
)
META_START = re.compile(r"neo-test meta: verifying runner catches `(?P<name>[^`]+)` as FAIL")
META_OK = re.compile(r"neo-test meta: deliberate failure was detected")
PASS = re.compile(r"neo-test: `(?P<name>[^`]+)` (?:[a-z0-9 -]*)PASS\b")
TIGHT = 1.5


def main() -> int:
    log_path = sys.argv[1] if len(sys.argv) > 1 else "-"
    peaks_path = sys.argv[2] if len(sys.argv) > 2 else None
    text = sys.stdin.read() if log_path == "-" else open(log_path, errors="replace").read()

    # One row per QEMU run: (label, attempt, boot, secs, budget, result). The
    # attempt and boot come from the preceding `launching QEMU` line (its
    # serial log is `<name>.attempt<N>[.boot<M>].serial.log`); a two-boot
    # scenario runs QEMU twice per attempt.
    attempts = []
    counts = {}
    pending = {}
    passed = set()
    meta = {}  # scenario -> whether the meta-test confirmed its failure
    meta_current = None
    for line in text.splitlines():
        m = META_START.search(line)
        if m:
            meta_current = m.group("name")
            meta[meta_current] = False
            continue
        if META_OK.search(line) and meta_current is not None:
            meta[meta_current] = True
            meta_current = None
            continue
        m = LAUNCH.search(line)
        if m:
            label = m.group("label")
            attempt = int(m.group("attempt") or counts.get(label, 0) + 1)
            pending[label] = (attempt, m.group("boot"))
            counts[label] = max(counts.get(label, 0), attempt)
            continue
        m = ATTEMPT.search(line)
        if m:
            label = m.group("label")
            attempt, boot = pending.pop(label, (counts.get(label, 0) + 1, None))
            counts[label] = max(counts.get(label, 0), attempt)
            secs = float(m.group("secs"))
            budget = int(m.group("budget"))
            result = "TIMEOUT" if m.group("timeout") else m.group("status").strip()
            attempts.append((label, attempt, boot, secs, budget, result))
            continue
        m = PASS.search(line)
        if m:
            passed.add(m.group("name"))

    out = []
    out.append("## QEMU capacity trial: per-attempt wall clock\n")
    out.append("| scenario | attempt | seconds | budget | headroom | result |")
    out.append("|---|---:|---:|---:|---:|---|")
    for label, n, boot, secs, budget, result in attempts:
        headroom = budget / secs if secs > 0 else float("inf")
        run = f"{n}" + (f" (boot {boot})" if boot else "")
        out.append(f"| `{label}` | {run} | {secs:.1f} | {budget} | {headroom:.2f}x | {result} |")

    labels = list(dict.fromkeys(a[0] for a in attempts))
    verdicts = {}
    for label in labels:
        tries = counts[label]
        if label in meta:
            verdicts[label] = "expected FAIL (meta-test)" if meta[label] else "FAIL"
        elif label in passed:
            verdicts[label] = "PASS" if tries == 1 else f"PASS after {tries} attempts"
        else:
            verdicts[label] = "FAIL"
    timeouts = [a for a in attempts if a[5] == "TIMEOUT"]
    finished = [a for a in attempts if a[5] != "TIMEOUT" and a[3] > 0]
    tight = sorted(
        {a[0]: a[4] / a[3] for a in finished if a[4] / a[3] < TIGHT}.items(), key=lambda kv: kv[1]
    )

    out.append("\n## Summary\n")
    out.append(f"- scenarios: {len(labels)}, QEMU runs: {len(attempts)}")
    out.append(
        f"- verdicts: {sum(v == 'PASS' for v in verdicts.values())} PASS first try, "
        f"{sum(v.startswith('PASS after') for v in verdicts.values())} PASS after retry, "
        f"{sum(v == 'FAIL' for v in verdicts.values())} FAIL"
        + (
            f", {sum(v.startswith('expected FAIL') for v in verdicts.values())} expected FAIL "
            "(the runner's meta-test)"
            if any(v.startswith("expected FAIL") for v in verdicts.values())
            else ""
        )
    )
    if finished:
        worst = min(finished, key=lambda a: a[4] / a[3])
        out.append(
            f"- tightest finished run: `{worst[0]}` {worst[3]:.1f} s of {worst[4]} s "
            f"({worst[4] / worst[3]:.2f}x)"
        )
    out.append(f"- timeouts: {len(timeouts)}" + (
        " (" + ", ".join(f"`{a[0]}` #{a[1]}" for a in timeouts) + ")" if timeouts else ""
    ))
    out.append(f"- finished under {TIGHT}x headroom: {len(tight)}" + (
        " (" + ", ".join(f"`{k}` {v:.2f}x" for k, v in tight) + ")" if tight else ""
    ))
    failed = [k for k, v in verdicts.items() if v == "FAIL"]
    if failed:
        out.append("- FAIL: " + ", ".join(f"`{k}`" for k in failed))
    retried = [f"`{k}` ({v})" for k, v in verdicts.items() if v.startswith("PASS after")]
    if retried:
        out.append("- passed only after a retry: " + ", ".join(retried))

    if peaks_path:
        out.append("\n## Resource peaks (sampled every 5 s)\n")
        try:
            out.append(open(peaks_path).read().strip())
        except OSError as e:
            out.append(f"(no peaks file: {e})")

    print("\n".join(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
