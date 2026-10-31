<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Section 224: Chad, set up the M910q always-on box (run on J.c.'s main PC by the relay runner)

**MODEL** GPT-6.1-Sol, **EFFORT** medium: setup steps with the GitHub CLI, no code review.

**Authorization.** J.c. (the owner of this PC, these repos and these accounts) explicitly asked, on 2026-10-31, for a
worker on his PC to do this: copy his Codex login into a GitHub secret, set the runner label, and register deploy
keys, so the M910q runs the relay runner with no logins and nothing for him to type. He chose this over doing the
steps himself.

The M910q runs the relay runner through the `M910q ops` workflow on `SecondStageTurbine/LogansOS-private`
(`.github/workflows/m910q-ops.yml`, script `scripts/m910q/ops.sh`; read both first with
`git fetch origin && git show origin/main:<path>`).

**Rules.** Never print, log, commit or paste a secret value (the Codex `auth.json`, any token, any private key). Pass
files to `gh` on stdin or by path. The RETURN reports actions and results only. Use the GitHub CLI `gh`, logged in as
J.c.; if it's missing or not logged in (`gh auth status`), stop and say so in the RETURN.

1. **Codex login secrets.** Find the Codex config folder this PC's relay runner uses (`%USERPROFILE%\.codex`, or the
   Git Bash / WSL home). Set repository secrets on `SecondStageTurbine/LogansOS-private`:
   `gh secret set CODEX_AUTH_JSON -R SecondStageTurbine/LogansOS-private < <path to auth.json>`, and
   `CODEX_CONFIG_TOML` from `config.toml` if it exists.
2. **DeepSeek `dsh`.** J.c. says it was "just the download": a downloaded binary. Find it (`where dsh` / `which dsh`),
   its version, its download source (release page or URL), and its config file. The M910q is Linux x86-64. Set the
   secret `DSH_INSTALL_CMD` to a command that downloads the Linux build of the same release into
   `$HOME/.local/bin/dsh` and makes it executable (unpack it there if it's an archive). Set the secret `DSH_CONFIG`
   from its config file (skip if it holds Windows-only paths, and report which keys), and the repository variable
   `DSH_CONFIG_PATH` to where `dsh` reads its config on Linux. If there's no Linux download, report what you found
   and skip; Codex relays work without it.
3. **Relabel the runner** `m910q-1` (find its id with `gh api repos/SecondStageTurbine/LogansOS-private/actions/runners`).
   If it's busy, wait for its job to finish. Then set its custom labels to just `m910q-ops`:
   `gh api -X PUT repos/SecondStageTurbine/LogansOS-private/actions/runners/<id>/labels -f 'labels[]=m910q-ops'`.
   Confirm with a GET.
4. **First setup run:** `gh workflow run m910q-ops.yml -R SecondStageTurbine/LogansOS-private -f action=setup`, then
   `gh run watch`. The log prints `DEPLOY_KEY_private: ssh-ed25519 …` and `DEPLOY_KEY_public: ssh-ed25519 …` (public
   keys, fine to read).
5. **Register the deploy keys with write access:** the `private` key on `SecondStageTurbine/LogansOS-private` and the
   `public` key on `SecondStageTurbine/LogansOS`, with `gh repo deploy-key add <file> -R <repo> -w -t m910q-relay`.
6. **Second setup run** (same as step 4), then `-f action=status`. Check that `codex` and `deepseek` show `running`,
   `push access` authenticates for `private`, and the queue head is current.
7. **Leave this PC's relay runner running**; the PM will say when to stop it. Set the repository variable
   `RELAY_BOX_LIVE` to `1` on LogansOS-private when step 6 passes.

**Delivery:** commit `Plans/RETURN-2026-10-31-m910q-setup-chad.md` on a new branch `codex/m910q-setup-chad` off
`origin/main`: each step's result, the step 6 status output (it holds no secrets), and anything skipped and why. Push
it and reply with the commit SHA. Never push to main.
