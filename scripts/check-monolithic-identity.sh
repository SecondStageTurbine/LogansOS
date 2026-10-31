#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# check-monolithic-identity.sh — CI grep-guard against microkernel-identity drift.
#
# This script fails if any first-party doc asserts that NeoPlutarch/LogansOS is
# a "microkernel." It allow-lists the legitimate contexts where "microkernel"
# appears correctly: HongMeng, seL4, Redox, Fuchsia, Asterinas, Theseus,
# "not a microkernel," "microkernel IPC overhead," "microkernel-style,"
# "microkernel transition," "microkernel OS," etc. — i.e., references to OTHER
# systems being microkernels, or our explicit rejection of the model.
#
# The drift this catches: calling NeoPlutarch/LogansOS/"our" a "microkernel"
# when we are monolithic (Bible Article 4.1, ADR-001). This has recurred 3x.
#
# Usage: scripts/check-monolithic-identity.sh
# Run from the repo root. Exit 0 = clean, exit 1 = drift detected.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Files to check: first-party docs and plans (not archived historical records).
CHECK_DIRS=("docs" "Plans")

# Pattern: look for lines that combine a first-party identifier with "microkernel".
# This catches: "our microkernel", "NeoPlutarch is a microkernel", "capability-secured microkernel"
# when used as a positive descriptor of ourselves.
#
# The heuristic: flag any line in docs/ or Plans/ that contains "microkernel" AND
# one of our identifiers (NeoPlutarch, LogansOS, our, we are, we're, this project).
# Then filter out the allow-listed legitimate contexts.

DRIFT_FOUND=0

for dir in "${CHECK_DIRS[@]}"; do
    if [ ! -d "$REPO_ROOT/$dir" ]; then
        continue
    fi

    # Find all .md files (exclude archive/ subdirectories)
    while IFS= read -r -d '' file; do
        # Skip archived documents — they're historical record
        if [[ "$file" == *"/archive/"* ]]; then
            continue
        fi

        # Skip the authoritative docs that are already correct
        if [[ "$file" == *"bible.md"* ]] || [[ "$file" == *"ADR-001"* ]]; then
            continue
        fi

        # Skip meta-docs whose SUBJECT is the drift itself — they must quote the
        # wrong phrasing to describe correcting it (the correction-sweep dispatch).
        if [[ "$file" == *"monolithic-identity-correction-sweep"* ]]; then
            continue
        fi

        # Check for drift: lines containing "microkernel" that also reference ourselves positively
        while IFS= read -r line; do
            lineno=$(echo "$line" | cut -d: -f1)
            content=$(echo "$line" | cut -d: -f2-)

            # Allow-list: legitimate uses of "microkernel" in our docs
            # 1. "not a microkernel" / "Not a research microkernel" — explicit
            #    rejection (correct). The original regex `(not|no )a ?microkernel`
            #    could never match "not a microkernel" (no space handling) — fixed
            #    to allow optional qualifier words between the article and the noun.
            if echo "$content" | grep -qiE "\bno[t]?\s+(a|an)\s+(\w+[- ]?)?micro-?kernel"; then
                continue
            fi

            # 1b. Rejection context — "A microkernel approach ... is rejected".
            if echo "$content" | grep -qiE "micro-?kernel.*\breject|\breject.*micro-?kernel"; then
                continue
            fi

            # 1c. Plural "microkernels" — a survey of OTHER systems, never an
            #     identity claim about us.
            if echo "$content" | grep -qiE "micro-?kernels\b"; then
                continue
            fi

            # 2. "microkernel IPC overhead" — referencing the concept, not our identity
            if echo "$content" | grep -qiE "microkernel (IPC|overhead|split|transition|orthodoxy|family|message passing)"; then
                continue
            fi

            # 3. "microkernel-style" / "microkernel approach" — hypothetical rejection
            if echo "$content" | grep -qiE "microkernel-(style|adjacent|ish)"; then
                continue
            fi

            # 4. References to OTHER systems being microkernels (HongMeng, seL4, Redox, Fuchsia, etc.)
            if echo "$content" | grep -qiE "(HongMeng|HM|seL4|Redox|Fuchsia|Zircon|HarmonyOS|Pneuma|Nanvix|Asterinas)"; then
                # Check: is "our" or "NeoPlutarch" being called a microkernel on this line?
                if echo "$content" | grep -qiE "(NeoPlutarch|LogansOS)\s+(is|are|as)\s+.*microkernel"; then
                    : # drift — fall through
                else
                    continue
                fi
            fi

            # 5. "strongest shipping microkernel OS" — referring to HarmonyOS, not us
            if echo "$content" | grep -qiE "(strongest|shipping)\s+.*microkernel\s+OS"; then
                continue
            fi

            # 6. "dominant microkernel overhead" — referencing HongMeng's architecture
            if echo "$content" | grep -qiE "dominant\s+microkernel\s+overhead"; then
                continue
            fi

            # 7. "microkernel boundary" — referring to HongMeng's isolation model
            if echo "$content" | grep -qiE "microkernel\s+boundary"; then
                continue
            fi

            # 8. "HongMeng's microkernel service-placement model" — correct reference
            if echo "$content" | grep -qiE "(HongMeng|HM)'s.*microkernel"; then
                continue
            fi

            # 9. "clean microkernel orthodoxy" — referencing HongMeng's paging
            if echo "$content" | grep -qiE "clean\s+microkernel\s+orthodoxy"; then
                continue
            fi

            # 10. "small seL4-class TEE/microkernel core" — correct reference
            if echo "$content" | grep -qiE "seL4-class.*microkernel"; then
                continue
            fi

            # 11. "monolithic architecture ≠ microkernel service placement" — correct contrast
            if echo "$content" | grep -qiE "(monolithic|our).*≠.*microkernel\|(monolithic|our).*(architecture|kernel).*≠.*microkernel"; then
                continue
            fi

            # If we got here, this is a potential drift hit
            echo "DRIFT: $file:$lineno"
            echo "  $content"
            DRIFT_FOUND=1

        done < <(grep -niE "microkernel" "$file" 2>/dev/null || true)

    done < <(find "$REPO_ROOT/$dir" -name "*.md" -print0 2>/dev/null)
done

if [ "$DRIFT_FOUND" -eq 1 ]; then
    echo ""
    echo "FAIL: microkernel-identity drift detected."
    echo "NeoPlutarch is a MONOLITHIC kernel (Bible Article 4.1, ADR-001)."
    echo "Fix the flagged lines to say 'monolithic' or reframe the reference."
    exit 1
fi

echo "PASS: no microkernel-identity drift detected."
exit 0
