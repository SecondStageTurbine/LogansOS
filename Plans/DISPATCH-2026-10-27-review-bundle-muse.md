<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE for Muse (Gwen), 2026-10-27: security relays #108, #111, #101, #105

**MODEL:** Muse (Gwen). **Why:** these are security reviews of Claude-authored code that controls who may access
what: permission revocation, the file-open permission, and the disk write guard that keeps the Zenbook's Windows
disk unwritten. Under CLAUDE.md they go to Muse.

**How.** Muse can't push. For each relay, read its dispatch file and the files it lists at the reviewed SHA, from the
repo (not a summary). Produce one RETURN per relay. The PL pastes or uploads each back, and the PM saves it at the
path shown. Work them in this order: the first two cover the permission model that the browser sandbox and logout
depend on.

Repository: SecondStageTurbine/LogansOS. Read each dispatch on `main` at `Plans/<file>`.

1. **Relay #108: security review of K1a-0, closing two revoke escapes in the capability graph (#166)**, reviewed SHA `149d4a19`.
   - Checklist and file list: `Plans/DISPATCH-2026-10-27-k1a0-cap-graph-review-muse.md` on `main`.
   - RETURN saved as `Plans/RETURN-2026-10-27-k1a0-cap-graph-review-muse.md`.

2. **Relay #111: security review of K1a, the root-directory capability (#168)**, reviewed SHA `2922bf0c`.
   - Checklist and file list: `Plans/DISPATCH-2026-10-27-k1a-rootdircap-review-muse.md` on `main`.
   - RETURN saved as `Plans/RETURN-2026-10-27-k1a-rootdircap-review-muse.md`.

3. **Relay #101: security review of OT-87 R3, the write guard's partition rule and parser move (#160)**, reviewed SHA `7c771c0b`.
   - Checklist and file list: `Plans/DISPATCH-2026-10-27-ot87-r3-guard-review-muse.md` on `main`.
   - RETURN saved as `Plans/RETURN-2026-10-27-ot87-r3-guard-review-muse.md`.

4. **Relay #105: security review of OT-87 R4.1, device identity and the install claim (#163)**, reviewed SHA `cec200af`.
   - Checklist and file list: `Plans/DISPATCH-2026-10-27-ot87-r4.1-claim-review-muse.md` on `main`.
   - RETURN saved as `Plans/RETURN-2026-10-27-ot87-r4.1-claim-review-muse.md`.

Each RETURN gives PASS or FAIL per checklist item, the enforcing `file:line`, a concrete sequence for any FAIL, and an
overall verdict.
