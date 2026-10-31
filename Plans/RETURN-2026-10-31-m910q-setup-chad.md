<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN - Relay 224 - M910q setup - Chad

Dispatch: `Plans/DISPATCH-2026-10-31-m910q-setup-chad.md`, Section 224.
Branch: `codex/m910q-setup-chad`, created from `origin/main` at `30176170`.
GitHub CLI authenticated as `SecondStageTurbine`. Read the workflow and ops script from fetched `origin/main` before running setup. No secret values are included here.

1. **Codex login:** Set `CODEX_AUTH_JSON` and `CODEX_CONFIG_TOML` on `SecondStageTurbine/LogansOS-private` from `C:\Users\<user>\.codex\auth.json` and `config.toml`, respectively, using subprocess stdin to `gh secret set`. Both succeeded; repository secret names/timestamps confirmed.
2. **DeepSeek:** `Get-Command dsh` resolves to `C:\Users\<user>\AppData\Roaming\npm\dsh.ps1`, invoking the npm package `@deepseek-ai/dsh`; `dsh --version` is `0.2.0-rc.2`. Package repository: https://github.com/deepseek-ai/deepseek-harness . Matching release: https://github.com/deepseek-ai/deepseek-harness/releases/tag/dsh-v0.2.0-rc.2 . Its release API reports no downloadable assets. No Linux binary download was found; skipped `DSH_INSTALL_CMD` under the dispatch's explicit no-Linux-download exception. The upstream README documents npm/npx installation, which is an alternative to a standalone binary and was not substituted for the requested Linux binary download. Set `DSH_CONFIG` from `C:\Users\<user>\.dsh\profiles\headless\cordis.patch.yml`; no Windows drive paths were found in this patch. Set `DSH_CONFIG_PATH` to `/home/logans/.dsh/profiles/headless/cordis.patch.yml`, matching dsh's default `~/.dsh` home on Linux. Configuration keys include `id`, `name`, `config`, `provider`, `model`, `reasoningEffort`, `mode`, and `defaultPreset`. The separate `.dsh/.credentials.yaml` was not copied as the headless profile configuration.
3. **Runner labels:** Waited without interrupting the existing QEMU Integration Tests (shard 2/4), job `124586641457`, CI run `41217251826`. The runner became idle at approximately `2026-10-31T17:33:33Z`. Replaced runner ID `55` (`m910q-1`) custom labels with only `m910q-ops` using PUT, then confirmed with GET. GitHub's read-only `self-hosted`, `Linux`, and `X64` labels remain.
4. **First setup:** Dispatched and watched [run 41225142776](https://github.com/SecondStageTurbine/LogansOS-private/actions/runs/41225142776). It installed rustup, Node v22.11.0, and Codex CLI 0.166.0, wrote the supplied secrets to their target files, and generated both deploy keys. It failed with exit 255 because SSH could not resolve `github-private`; all worker clones failed and the relay runners were not started.
5. **Deploy keys:** Registered the first run's public keys as `m910q-relay` with write access (`read_only: false`), confirmed through the repository keys API: private repository key ID `168314621`; public repository key ID `168314623`. Only the public halves were extracted to temporary files for `gh repo deploy-key add`; private keys were never copied.
6. **Second setup and status:** Dispatched and watched [second setup 41225217824](https://github.com/SecondStageTurbine/LogansOS-private/actions/runs/41225217824), then [status 41225246519](https://github.com/SecondStageTurbine/LogansOS-private/actions/runs/41225246519). Both failed with exit 255. The private SSH alias still did not resolve after key registration. Codex and DeepSeek are NOT running, private push access did not authenticate, and the queue head is absent because the relay repository could not be cloned. Step 6 did not pass.
7. **PC relay / live flag:** Left this PC's relay runner running and issued no stop command. Did not set `RELAY_BOX_LIVE=1` because step 6 failed.

## Step 6 status output

The following is the status command's output, with Actions timestamps and step labels removed. The environment/header containing masked secrets is omitted. The script exits at the private SSH failure, before public push access and disk/memory output.

```text
== relay runners
codex: NOT running
deepseek: NOT running
== cron
(not armed)
== relay log (last 25)
(no log yet)
== watchdog (last 5)
== queue head
== tools
codex-cli 0.166.0
dsh missing
== push access
private: ssh: Could not resolve hostname github-private: Temporary failure in name resolution
##[error]Process completed with exit code 255.
```

## Observed blocker

In `scripts/m910q/ops.sh`, setup creates `~/.ssh/config.relay` with both host aliases, then touches `~/.ssh/config` and attempts `sed -i '1i Include config.relay'` if the Include is absent. GNU sed's address `1` never matches an empty file, so this operation leaves a fresh empty config empty. A local reproduction, `printf '' | sed '1i Include config.relay' | wc -c`, returned `0`. The observed hostname-resolution failure is consistent with the alias file never being included; confirming the remote config contents would require an additional diagnostic operation. The deploy-key registrations succeeded but cannot repair alias resolution. The script also uses `set -euo pipefail` around the status SSH pipeline, so an SSH failure ends status immediately.

No workflow/script changes or additional remote repair actions were made: this RETURN records the exact requested setup sequence and its failed verification. The M910q must not be considered live on this evidence. DeepSeek additionally lacks an installed executable; its separately stored account credentials were not transferred as profile configuration.
