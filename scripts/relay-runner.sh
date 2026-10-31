#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Relay runner (J.c.'s PC, Git Bash): polls Plans/RELAY-QUEUE.md on origin/main and
# runs each new relay in that coder's worker folder: Windows Codex CLI, or DeepSeek
# headless (`dsh`) when the model is DeepSeek, or OpenCode with the local Qwen
# (`opencode run -m strata/...`) when the model is Qwen.
# The coder pushes its own RETURN branch; the PM reads it from GitHub. Nothing here
# pushes to main. Start once:  nohup bash relay-runner.sh >/dev/null 2>&1 &
set -u
WORKERS="${WORKERS:-/e/AI Automation/workers}"
REPO="${REPO:-$WORKERS/relay-runner-repo}"          # a plain clone used only to read the queue
STATE="$WORKERS/.relay-runner"; mkdir -p "$STATE"; touch "$STATE/done"
LOG="$STATE/log"; INTERVAL="${INTERVAL:-1200}"
# ONLY=codex|deepseek|qwen runs one engine's relays, so one runner per engine can run side by
# side (unset = all engines). A per-coder lock keeps two runs out of one worker folder.
ONLY="${ONLY:-}"
engine() { case "$1" in Qwen*|qwen*|strata/*) echo qwen ;; DeepSeek*|deepseek*) echo deepseek ;; *) echo codex ;; esac; }

# Model names as the PM writes them -> Codex model slugs (checked against the PL's
# ~/.codex/models_cache.json, 2026-10-30). Unlisted names: lowercased, spaces to dashes.
slug() {
  case "$1" in
    GPT-6.1-Sol) echo "gpt-6.1-sol" ;; GPT-6-Luna) echo "gpt-6-luna" ;;
    GPT-5.6-Luna) echo "gpt-5.6-luna" ;; GPT-6-Astra) echo "gpt-6-astra" ;;
    "Daybreak Blue") echo "gpt-daybreak-blue-latest" ;;
    *) echo "$1" | tr '[:upper:]' '[:lower:]' | tr ' ' '-' ;;
  esac
}

log() { echo "$(date -u +%FT%TZ) [${ONLY:-all}] $*" >> "$LOG"; }

[ -d "$REPO/.git" ] || git clone -q https://github.com/SecondStageTurbine/LogansOS.git "$REPO"

while true; do
  if git -C "$REPO" fetch -q origin main 2>>"$LOG"; then
    # Queue lines: RELAY <id> | <coder> | <model> | <effort> | <dispatch path> | <section>
    git -C "$REPO" show origin/main:Plans/RELAY-QUEUE.md 2>/dev/null | grep '^RELAY ' |
    while IFS='|' read -r head coder model effort dispatch section; do
      id=$(echo "$head" | awk '{print $2}'); coder=$(echo $coder); model=$(echo $model)
      effort=$(echo $effort); dispatch=$(echo $dispatch); section=$(echo $section)
      grep -qx "$id" "$STATE/done" && continue
      [ -n "$ONLY" ] && [ "$(engine "$model")" != "$ONLY" ] && continue
      dir="$WORKERS/$coder"; [ -d "$dir" ] || { log "relay $id: no folder $dir"; continue; }
      lock="$STATE/lock-$coder"
      mkdir "$lock" 2>/dev/null || { log "relay $id: $coder busy, next cycle"; continue; }
      prompt="You are $coder. Relay #$id. Run: git fetch origin && git show origin/main:$dispatch . Do $section of that dispatch exactly as written: check out the branch it names, write the RETURN file it names, commit and push it. Reply with the RETURN commit SHA."
      log "relay $id: start ($coder, $model/$effort)"
      case "$model" in
        Qwen*|qwen*|strata/*)
          # OpenCode against the local Qwen server (provider "strata"). One server slot, so runs serialize.
          qm="$model"; case "$qm" in strata/*) ;; *) qm="strata/qwen3.8-flash-next-iq3_s" ;; esac
          ( cd "$dir" && opencode run -m "$qm" "$prompt" < /dev/null ) > "$STATE/relay-$id.out" 2>&1
          rc=$?
          tail -20 "$STATE/relay-$id.out" > "$STATE/relay-$id.last" ;;
        DeepSeek*|deepseek*)
          # dsh's headless profile fixes the model and effort (deepseek-flash, high).
          ( cd "$dir" && printf '%s' "$prompt" | dsh headless --json - ) > "$STATE/relay-$id.out" 2>&1
          rc=$?
          grep '"type":"final"' "$STATE/relay-$id.out" | tail -1 > "$STATE/relay-$id.last" ;;
        *)
          # </dev/null: codex exec reads stdin, which here is the queue itself.
          # approval_policy=never in the workspace sandbox (--approve-for-me needs Daybreak,
          # which is unavailable for most models). The PL config sets daybreak = true, which
          # non-Daybreak models reject, so turn it off for them. danger-full-access: the
          # workspace sandbox makes .git read-only and hides the GitHub login (PL approved).
          db=false; case "$(slug "$model")" in *daybreak*) db=true ;; esac
          ( cd "$dir" && codex exec -m "$(slug "$model")" -c daybreak=$db -c model_reasoning_effort="$effort" \
              -s danger-full-access -c approval_policy="never" \
              -o "$STATE/relay-$id.last" "$prompt" < /dev/null ) >> "$STATE/relay-$id.out" 2>&1
          rc=$? ;;
      esac
      rmdir "$lock" 2>/dev/null
      log "relay $id: exit $rc"
      # Done only if the reply names a commit SHA: coders exit 0 when they report a block.
      # ponytail: any hex run of 7-40 chars counts; check the RETURN branch if that misfires.
      [ $rc -eq 0 ] && grep -qE '\b[0-9a-f]{7,40}\b' "$STATE/relay-$id.last" && echo "$id" >> "$STATE/done"
    done
  else
    log "fetch failed"
  fi
  sleep "$INTERVAL"
done
