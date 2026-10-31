<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay #136, NVMe spec rows (#181)

Reviewed `ee629498`; code citations refer to that squash. Worktree: `E:\AI Automation\workers\chad`. RAG healthy. Requested GPT-6.1-Sol/high; session model/effort not independently verifiable.

Sources: [NVMe Base 2.0, May 13 2021](https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-2_0-2021.06.02-Ratified-4.pdf) (copyright NVM Express, Inc.); [PCIe Transport 1.3, July 30 2025](https://nvmexpress.org/wp-content/uploads/NVM-Express-NVMe-over-PCIe-Transport-Specification-Revision-1.3-2025.08.01-Ratified.pdf). The web PDF reader reached both; direct HTTP downloads returned 403. Exact layouts were separately read from the [raw SPDK header at 65b7c388](https://raw.githubusercontent.com/spdk/spdk/65b7c388/include/spdk/nvme_spec.h), BSD-3-Clause, not a summarizing fetch.

Each S-row's specification reference is to the linked Base PDF unless explicitly marked PCIe. FAILs concern the PR's generalizations; they do not establish a wrong command encoding.

- **S1 PASS:** ?4.3.5?4.3.6, Figures 127?132. Raw SPDK `nvme_spec.h:3205` and `:3208`, 128-byte prefix assertion `:3210`. `kernel/src/drivers/block/nvme_event_logic.rs:53` copies 104..128 unchanged.
- **S2 FAIL:** ?5.17.2.3, Figure 277 permits UUID-only identification. Input: NGUID=EUI64=0, valid type-3 descriptor. `kernel/src/drivers/block/claim_logic.rs:63` conservatively refuses destructive identity; that policy is valid, the claimed compliance requirement is false.
- **S3 PASS:** ?5.2, Figure 143; opcode table in ?5. Raw SPDK `:1777`, `:4380`. `nvme_event_logic.rs:75` masks/shifts the matching fields.
- **S4 PASS:** ?5.2 Figure 146; ?5.16.1.6 (Changed Namespace List). `nvme_event_logic.rs:33`, `:38` and `:39` name the constants.
- **S5 PASS:** ?5.27.1.8 Figure 326. Raw SPDK `:954`. `kernel/src/drivers/block/nvme.rs:778` writes FID 0Bh and CDW11=100h.
- **S6 PASS for namespace notices:** ?5.2 and Figure 146 specify log acknowledgement. `nvme.rs:820` bumps the medium, reads the named log, then re-posts. Immediate events are a separate class, not a universal log-ack rule.
- **S7 PASS:** ?5.16 Figure 197. Raw SPDK `:1172`. `nvme_event_logic.rs:126` produces 03ff0004h for log 04h; CDW11 stays zero.
- **S8 PASS:** ?3.7.2 explicitly preserves AQA/ASQ/ACQ on Controller Reset. `nvme_event_logic.rs:144` checks readiness and CC as well.
- **S9 FAIL as ?every register?; named registers PASS:** ?3.7.2 excepts CMBMSC on FLR. Input: nondefault CMBMSC followed by FLR. PCIe ?2.3 Controller Level Reset and Base property reset columns support the CC/AQA/ASQ/ACQ checks at `nvme_event_logic.rs:156`.
- **S10 FAIL as unconditional:** ?3.1.3.6 Figure 47 qualifies NSSRO by CAP.NSSRS; firmware activation and power application clear it. Input: subsystem reset activating firmware. `nvme_event_logic.rs:144` still rejects RDY=0/changed CC, so the documented register checks do not depend solely on NSSRO.
- **S11 PASS:** ?3.1.3.5?.6, .8?.10. Raw SPDK register structure and `union spdk_nvme_csts_register:174` agree. Driver constants `nvme.rs:29` and `read_regs` at `:797` match.
- **S12 FAIL as general rule:** ?5.27.1.28/?8.12: Write Protection persists despite SV=0. Input: FID84h, write-protect=1, reset. The actual AEC command at `nvme.rs:780` has SV clear and changes no saved setting (?4.2, Figure 314); narrow the Zenbook note to AEC.
- **S13 PASS:** ?5.22 Figures 294?295. Raw SPDK opcode `:1784`. `nvme.rs:365` builds one controller-list entry and SEL=0/1.

- **PASS ? implementation uses the reviewed encodings.** `nvme_event_logic.rs:53`, `:75`, `:126`, `:144`; `nvme.rs:778`, `:820`. Unknown events conservatively change the medium; error completions, unexpected CIDs, log-read failures and missing AERs declare it dead. Exactly one request is outstanding. The all-zero identifier rule is explicit policy, not a spec mandate. A reset followed by another agent restoring exactly the old queue registers remains the PR's declared limit.
- **PASS ? raw byte evidence.** Downloaded the pinned header directly (135,921 bytes), read the structures, enums and bitfields above. No offset was accepted solely from a search excerpt.

Validation on `87458b7e`: `cargo test -p install-claim-logic-tests nvme -- --nocapture`: **12 PASS**. QEMU and hardware not rerun. Requested corrections: qualify S2/S9/S10/S12 and the reset table; retain the conservative driver policy.
