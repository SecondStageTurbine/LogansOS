<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay #82, Jarvis stash rescue

Worktree audited: `E:\AI Automation\workers\jarvis` only.
No code review, build, or test run. RAG was not used.
Requested GPT-5.6-Luna was unavailable in this session; the current model
performed this inventory, as disclosed before the audit.

## State preserved

Current branch: `codex/boot-evidence-holds-review-jarvis`.
Current HEAD: `90fe0905bad7a2e6681616637059e6206791784a`.

Initial `git status --porcelain`:
```text
 M .gitignore
?? .ignore
```

Initial `git stash list`:
```text
stash@{0}: On codex/build-id-review-jarvis: relay76-preserve-preexisting-graft-ignore
stash@{1}: On codex/build-id-review-jarvis: relay74-preserve-preexisting-graft-ignore
stash@{2}: On codex/model-load-if-on-review-jarvis: relay60 preserve pre-existing Jarvis local changes and graft cache
```

No stash was dropped, popped, applied, cleared, or created. The main worktree
was not switched or edited. Its branch, HEAD, file-content hashes, status,
and stash list were checked unchanged before writing this RETURN in the
separate temporary worktree.

## Every stash

The following blocks are complete stdout from
`git stash show --stat --include-untracked 'stash@{N}'`; this Git supports
`--include-untracked`. Commands sometimes warn on stderr that the sandbox
cannot access `C:\Users\<user>/.config/git/ignore`; all inventory commands
succeeded and explicit stash contents were read from Git objects.

### stash@{0}: cache only

Label: `On codex/build-id-review-jarvis: relay76-preserve-preexisting-graft-ignore`.
SHA: `591a6651ec1bd84f6e753b9be1cec1c12f43093c`.

```text
 .gitignore | 3 +++
 .ignore    | 5 +++++
 2 files changed, 8 insertions(+)
```

Non-cache paths: none.

### stash@{1}: cache only

Label: `On codex/build-id-review-jarvis: relay74-preserve-preexisting-graft-ignore`.
SHA: `cfac65f3e15959118ad7d9f088e26ecea33036d1`.

```text
 .gitignore | 3 +++
 .ignore    | 5 +++++
 2 files changed, 8 insertions(+)
```

Non-cache paths: none.

### stash@{2}: has other files

Label: `On codex/model-load-if-on-review-jarvis: relay60 preserve pre-existing Jarvis local changes and graft cache`.
SHA: `f2dd94c5a0585b89e9b6469746b46ca8abe61fc3`.

```text
 .gitignore                                         |      3 +
 .ignore                                            |      5 +
 graft/.cache/ask-index.json                        |      1 +
 graft/.cache/extract.c3679fb5d24a63a0.json         |      1 +
 graft/.cache/fingerprint.c3679fb5d24a63a0.json     |      1 +
 .../01a14af1-c646-72a2-aeda-e6ed6bc4f738.json      |     10 +
 .../01a14f22-58c4-77e1-87b6-a288a247e573.json      |     10 +
 .../01a15082-954a-7ea3-8f5b-ad3da3a0db59.json      |     10 +
 .../01a1548a-acca-7112-b3cb-0753cd291677.json      |     10 +
 .../01a15539-8bfc-7a81-aebd-47be14c4d413.json      |     10 +
 .../01a1555a-6c0b-7743-a2f3-e64d5e618356.json      |     10 +
 .../01a15583-a9a4-7c20-9491-605a7eca1b77.json      |     10 +
 .../01a15724-3538-7131-b0cb-e891e851bc36.json      |     24 +
 .../01a15a45-3ab2-75b2-8d8a-bfe8f6982cce.json      |     14 +
 .../01a15f63-5b99-72f2-8459-a5c8437ea504.json      |     14 +
 .../01a15f91-1275-7d61-9f89-ccbcac63e22a.json      |     10 +
 .../01a16004-222e-7060-b620-1d057aaff79c.json      |     10 +
 .../01a1600f-6a20-7d30-ba43-9a5578a89155.json      |     10 +
 .../01a160e1-caab-7212-a798-a8e8767588d0.json      |     10 +
 .../01a160e8-2512-7133-8170-b71eba7fa5fa.json      |     14 +
 .../01a164d7-ead0-72a3-a705-f6017bd52313.json      |     10 +
 .../01a1653a-eb83-7c00-862d-15d948316ed6.json      |     10 +
 .../01a1659b-3498-7e40-96c1-edf707ed94b1.json      |     10 +
 .../01a168fe-301b-7013-bcad-925b9378c3f9.json      |     10 +
 .../01a1693c-84e5-7f80-8f12-4d908217bc32.json      |     10 +
 .../01a16a3e-ad26-7f81-9832-5d75a0d6dafd.json      |      9 +
 .../01a16c56-9973-7113-a980-8f5ae17d62b5.json      |      9 +
 .../01a173bb-a989-7470-a9ef-fd9b9d735542.json      |      9 +
 .../d33d139a-e847-4daf-b144-eca39f52cdac.json      |     10 +
 graft/.cache/stats.json                            |     15 +
 graft/.cache/telemetry-repo-id.json                |      3 +
 graft/.graph/wiring.json                           | 373113 ++++++++++++++++++
 graft/INDEX.md                                     |     15 +
 graft/fixtures/arthur-s2/title_window.md           |      6 +
 graft/fixtures/desktop-clock/clock.md              |     12 +
 graft/fixtures/desktop-di1b/move_digests.md        |     12 +
 graft/fixtures/desktop-di1b/resize_digests.md      |     17 +
 graft/fixtures/desktop-dl1/dock_digests.md         |     12 +
 graft/fixtures/desktop-dl1/dock_labels.md          |     13 +
 graft/fixtures/desktop-dm1/motion_digests.md       |     15 +
 graft/fixtures/desktop-ds1/field_text.md           |     15 +
 graft/fixtures/desktop-ds1/home_digests.md         |     15 +
 graft/fixtures/desktop-ds1/lens_content.md         |     16 +
 graft/fixtures/desktop-ds1/lens_digests.md         |     11 +
 graft/fixtures/desktop-ds1/lens_labels.md          |     22 +
 graft/fixtures/desktop-intro/intro.md              |     30 +
 graft/fixtures/desktop-s3/bar_label_rt.md          |      6 +
 graft/fixtures/desktop-s3/labels.md                |     28 +
 graft/fixtures/desktop-s3/scene_digests.md         |     11 +
 graft/fixtures/desktop-s3/title_label_rt.md        |      6 +
 graft/fixtures/qwen35_llama_cpp_hello_golden.md    |      4 +
 graft/kernel/build.md                              |      4 +
 graft/kernel/src/acpi/dmar.md                      |      4 +
 graft/kernel/src/acpi/dmar_logic.md                |     29 +
 graft/kernel/src/acpi/madt_logic.md                |     13 +
 graft/kernel/src/acpi/mod.md                       |     31 +
 graft/kernel/src/acpi/rtc_fadt.md                  |      4 +
 graft/kernel/src/acpi/rtc_fadt_logic.md            |      6 +
 graft/kernel/src/acpi/timers_logic.md              |     29 +
 graft/kernel/src/ai/admission.md                   |     25 +
 graft/kernel/src/ai/admission_policy.md            |      9 +
 graft/kernel/src/ai/arena.md                       |     39 +
 graft/kernel/src/ai/arena_logic.md                 |     10 +
 graft/kernel/src/ai/decode_logic.md                |     10 +
 graft/kernel/src/ai/evict.md                       |     23 +
 graft/kernel/src/ai/evict_policy.md                |     12 +
 graft/kernel/src/ai/exec.md                        |     12 +
 graft/kernel/src/ai/exec_dispatch.md               |     12 +
 graft/kernel/src/ai/exec_ops.md                    |     15 +
 graft/kernel/src/ai/forward_logic.md               |    120 +
 graft/kernel/src/ai/forward_perf.md                |     15 +
 graft/kernel/src/ai/generate.md                    |     10 +
 graft/kernel/src/ai/generate_logic.md              |     39 +
 graft/kernel/src/ai/gguf_dequant_logic.md          |     20 +
 graft/kernel/src/ai/gguf_logic.md                  |     64 +
 graft/kernel/src/ai/kv_cache.md                    |     23 +
 graft/kernel/src/ai/kv_layout.md                   |      8 +
 graft/kernel/src/ai/mod.md                         |     33 +
 graft/kernel/src/ai/model.md                       |     32 +
 graft/kernel/src/ai/model_logic.md                 |    115 +
 graft/kernel/src/ai/ops/gdn.md                     |     32 +
 graft/kernel/src/ai/ops/mod.md                     |     72 +
 graft/kernel/src/ai/ops/q4_k.md                    |     15 +
 graft/kernel/src/ai/ops/qk_matmul.md               |    104 +
 graft/kernel/src/ai/ops/soft_f32.md                |     41 +
 graft/kernel/src/ai/par_matmul.md                  |     42 +
 graft/kernel/src/ai/par_matmul_logic.md            |      6 +
 graft/kernel/src/ai/placement.md                   |     11 +
 graft/kernel/src/ai/placement_policy.md            |      6 +
 graft/kernel/src/ai/predictor.md                   |      4 +
 graft/kernel/src/ai/prefill_perf.md                |     36 +
 graft/kernel/src/ai/qwen_inventory.md              |     69 +
 graft/kernel/src/ai/reservation.md                 |     43 +
 graft/kernel/src/ai/reservation_logic.md           |      4 +
 graft/kernel/src/ai/residency_daemon.md            |     45 +
 graft/kernel/src/ai/sharing.md                     |     20 +
 graft/kernel/src/ai/sharing_logic.md               |      5 +
 graft/kernel/src/ai/tensor.md                      |     50 +
 graft/kernel/src/ai/tensor_logic.md                |     30 +
 graft/kernel/src/ai/tokenizer_logic.md             |     21 +
 graft/kernel/src/ai/watermark.md                   |      8 +
 graft/kernel/src/arch/mod.md                       |      8 +
 graft/kernel/src/arch/x86_64/apic.md               |     48 +
 graft/kernel/src/arch/x86_64/apic_logic.md         |     16 +
 graft/kernel/src/arch/x86_64/context.md            |     18 +
 graft/kernel/src/arch/x86_64/cpufreq.md            |     17 +
 graft/kernel/src/arch/x86_64/cpufreq_logic.md      |      8 +
 graft/kernel/src/arch/x86_64/fpu.md                |     37 +
 graft/kernel/src/arch/x86_64/gdt.md                |     33 +
 graft/kernel/src/arch/x86_64/idt.md                |    100 +
 graft/kernel/src/arch/x86_64/ioapic.md             |     16 +
 graft/kernel/src/arch/x86_64/ioapic_logic.md       |      9 +
 graft/kernel/src/arch/x86_64/lapic_timer.md        |     36 +
 graft/kernel/src/arch/x86_64/lapic_timer_logic.md  |     23 +
 graft/kernel/src/arch/x86_64/mod.md                |     61 +
 graft/kernel/src/arch/x86_64/per_cpu.md            |     25 +
 graft/kernel/src/arch/x86_64/pic.md                |     15 +
 graft/kernel/src/arch/x86_64/pit.md                |      7 +
 graft/kernel/src/arch/x86_64/refclock.md           |     26 +
 graft/kernel/src/arch/x86_64/rtc.md                |     17 +
 graft/kernel/src/arch/x86_64/rtc_logic.md          |     31 +
 graft/kernel/src/arch/x86_64/smp.md                |     17 +
 graft/kernel/src/arch/x86_64/syscall.md            |    416 +
 graft/kernel/src/arch/x86_64/timer.md              |      6 +
 graft/kernel/src/atrium/blur.md                    |     31 +
 graft/kernel/src/atrium/buffer.md                  |     29 +
 graft/kernel/src/atrium/focus.md                   |     60 +
 graft/kernel/src/atrium/glass.md                   |     11 +
 graft/kernel/src/atrium/input_logic.md             |     12 +
 graft/kernel/src/atrium/input_queue_logic.md       |     13 +
 graft/kernel/src/atrium/key_repeat.md              |     10 +
 graft/kernel/src/atrium/key_repeat_logic.md        |     15 +
 graft/kernel/src/atrium/logic.md                   |     40 +
 graft/kernel/src/atrium/mod.md                     |     20 +
 graft/kernel/src/atrium/pointer_route_logic.md     |     27 +
 graft/kernel/src/atrium/present_backend.md         |     33 +
 graft/kernel/src/atrium/sak_logic.md               |     12 +
 graft/kernel/src/boot_model_logic.md               |     25 +
 graft/kernel/src/boot_modules.md                   |     46 +
 graft/kernel/src/drivers/accel/accel_cpu.md        |     23 +
 graft/kernel/src/drivers/accel/accel_intel.md      |    245 +
 graft/kernel/src/drivers/accel/accel_intel_ids.md  |      8 +
 graft/kernel/src/drivers/accel/accel_sim.md        |     73 +
 graft/kernel/src/drivers/accel/accounting.md       |     30 +
 graft/kernel/src/drivers/accel/accounting_table.md |     16 +
 graft/kernel/src/drivers/accel/gpu_compute.md      |     16 +
 graft/kernel/src/drivers/accel/intel_gen9_4a.md    |    164 +
 .../src/drivers/accel/intel_gen9_execlist.md       |     81 +
 graft/kernel/src/drivers/accel/intel_gen9_ppgtt.md |     34 +
 graft/kernel/src/drivers/accel/mod.md              |     65 +
 graft/kernel/src/drivers/accel/recency_table.md    |     10 +
 graft/kernel/src/drivers/accel/virtio_accel.md     |    184 +
 graft/kernel/src/drivers/block/ahci.md             |    100 +
 graft/kernel/src/drivers/block/dma.md              |     22 +
 graft/kernel/src/drivers/block/mod.md              |     62 +
 graft/kernel/src/drivers/block/nvme.md             |    101 +
 graft/kernel/src/drivers/block/virtio_barrier.md   |      4 +
 graft/kernel/src/drivers/block/virtio_blk.md       |    189 +
 .../kernel/src/drivers/block/write_guard_logic.md  |     57 +
 graft/kernel/src/drivers/console/mod.md            |      3 +
 graft/kernel/src/drivers/console/serial.md         |     19 +
 graft/kernel/src/drivers/dma_arena.md              |     23 +
 graft/kernel/src/drivers/dma_arena_logic.md        |     10 +
 graft/kernel/src/drivers/fb/console.md             |     18 +
 graft/kernel/src/drivers/fb/console_logic.md       |     14 +
 graft/kernel/src/drivers/fb/font.md                |     10 +
 graft/kernel/src/drivers/fb/mod.md                 |     55 +
 graft/kernel/src/drivers/fb/progress.md            |     28 +
 graft/kernel/src/drivers/fb/progress_logic.md      |     17 +
 graft/kernel/src/drivers/graphics_arena.md         |     35 +
 graft/kernel/src/drivers/i2c_hid/dw_logic.md       |     81 +
 graft/kernel/src/drivers/i2c_hid/hid_logic.md      |     62 +
 graft/kernel/src/drivers/i2c_hid/mod.md            |      7 +
 graft/kernel/src/drivers/i2c_hid/pci_pm_logic.md   |     16 +
 graft/kernel/src/drivers/i2c_hid/quirk_logic.md    |     20 +
 graft/kernel/src/drivers/i2c_hid/touchpad.md       |    121 +
 graft/kernel/src/drivers/iommu/mod.md              |     45 +
 graft/kernel/src/drivers/iommu/vtd_logic.md        |     42 +
 graft/kernel/src/drivers/iommu/vtd_regs.md         |     38 +
 graft/kernel/src/drivers/mod.md                    |     14 +
 graft/kernel/src/drivers/net/mod.md                |     10 +
 graft/kernel/src/drivers/net/rtl8125.md            |    111 +
 graft/kernel/src/drivers/net/rtl8125_logic.md      |    170 +
 graft/kernel/src/drivers/net/virtio_net.md         |    120 +
 graft/kernel/src/drivers/pci.md                    |     88 +
 graft/kernel/src/drivers/ps2/i8042.md              |     59 +
 graft/kernel/src/drivers/ps2/i8042_logic.md        |     42 +
 graft/kernel/src/drivers/ps2/mod.md                |      4 +
 graft/kernel/src/drivers/usb/boot_eth0.md          |     20 +
 graft/kernel/src/drivers/usb/boot_eth0_logic.md    |      8 +
 graft/kernel/src/drivers/usb/cdc_ecm.md            |     69 +
 graft/kernel/src/drivers/usb/hid_keyboard.md       |     30 +
 graft/kernel/src/drivers/usb/hid_logic.md          |     26 +
 graft/kernel/src/drivers/usb/hid_mouse.md          |     32 +
 graft/kernel/src/drivers/usb/mod.md                |     42 +
 .../kernel/src/drivers/usb/mt7601u/eeprom_logic.md |     16 +
 graft/kernel/src/drivers/usb/mt7601u/fw_logic.md   |     15 +
 graft/kernel/src/drivers/usb/mt7601u/initvals.md   |      6 +
 .../kernel/src/drivers/usb/mt7601u/initvals_phy.md |      9 +
 graft/kernel/src/drivers/usb/mt7601u/key_logic.md  |      4 +
 .../src/drivers/usb/mt7601u/link_result_logic.md   |     12 +
 .../src/drivers/usb/mt7601u/linux_parity_logic.md  |     53 +
 graft/kernel/src/drivers/usb/mt7601u/mcu_logic.md  |     15 +
 graft/kernel/src/drivers/usb/mt7601u/mod.md        |    355 +
 graft/kernel/src/drivers/usb/mt7601u/phy.md        |     37 +
 .../src/drivers/usb/mt7601u/post_fw_logic.md       |      5 +
 graft/kernel/src/drivers/usb/mt7601u/regs.md       |    198 +
 graft/kernel/src/drivers/usb/mt7601u/rx_logic.md   |     16 +
 graft/kernel/src/drivers/usb/mt7601u/tx_logic.md   |     21 +
 .../kernel/src/drivers/usb/mt7601u/upload_logic.md |      9 +
 .../kernel/src/drivers/usb/mt7601u/wedge_logic.md  |     52 +
 .../src/drivers/usb/operation_budget_logic.md      |     19 +
 graft/kernel/src/drivers/usb/usb_parse.md          |     22 +
 graft/kernel/src/drivers/usb/xhci.md               |    173 +
 graft/kernel/src/drivers/usb/xhci_hotplug.md       |     18 +
 graft/kernel/src/drivers/usb/xhci_ring_logic.md    |     46 +
 graft/kernel/src/drivers/usb/xhci_silicon.md       |     39 +
 graft/kernel/src/drivers/usb/xhci_silicon_logic.md |     52 +
 graft/kernel/src/errno.md                          |     25 +
 graft/kernel/src/fs/ext4.md                        |     58 +
 graft/kernel/src/fs/fat32.md                       |     46 +
 graft/kernel/src/fs/mod.md                         |    164 +
 graft/kernel/src/fs/neofs/cache.md                 |     41 +
 graft/kernel/src/fs/neofs/format.md                |     49 +
 graft/kernel/src/fs/neofs/inode.md                 |    135 +
 graft/kernel/src/fs/neofs/mod.md                   |      7 +
 graft/kernel/src/fs/neofs/mount.md                 |     37 +
 graft/kernel/src/init/boot_frame.md                |      9 +
 graft/kernel/src/init/boot_frame_eth0.md           |     10 +
 graft/kernel/src/init/boot_logo.md                 |      6 +
 graft/kernel/src/init/mod.md                       |      5 +
 graft/kernel/src/ipc/mod.md                        |     32 +
 graft/kernel/src/loader/elf.md                     |     22 +
 graft/kernel/src/loader/mod.md                     |      3 +
 graft/kernel/src/log/block_sink.md                 |     67 +
 graft/kernel/src/log/mod.md                        |     10 +
 graft/kernel/src/main.md                           |     50 +
 graft/kernel/src/mm/frame.md                       |     47 +
 graft/kernel/src/mm/frame_reentry_guard.md         |      7 +
 graft/kernel/src/mm/heap.md                        |     29 +
 graft/kernel/src/mm/heap_policy.md                 |     10 +
 graft/kernel/src/mm/hhdm_requirements.md           |     18 +
 graft/kernel/src/mm/inference_stack.md             |     44 +
 graft/kernel/src/mm/inference_stack_logic.md       |     19 +
 graft/kernel/src/mm/mod.md                         |     53 +
 graft/kernel/src/mm/module_reclaim_logic.md        |     22 +
 graft/kernel/src/mm/paging.md                      |      7 +
 graft/kernel/src/mm/pmap.md                        |     57 +
 graft/kernel/src/mm/slab.md                        |     56 +
 graft/kernel/src/mm/tlb_shootdown.md               |     65 +
 graft/kernel/src/net/arp.md                        |     37 +
 graft/kernel/src/net/device.md                     |     21 +
 graft/kernel/src/net/dhcpv4.md                     |     43 +
 graft/kernel/src/net/dns.md                        |     60 +
 graft/kernel/src/net/happy_eyeballs.md             |     19 +
 graft/kernel/src/net/icmpv6.md                     |     19 +
 graft/kernel/src/net/identity.md                   |     14 +
 graft/kernel/src/net/identity_logic.md             |     13 +
 graft/kernel/src/net/interface.md                  |      5 +
 graft/kernel/src/net/ipv4.md                       |     32 +
 graft/kernel/src/net/ipv6.md                       |     27 +
 graft/kernel/src/net/mod.md                        |     21 +
 graft/kernel/src/net/ndp.md                        |     66 +
 graft/kernel/src/net/smoltcp_wrap.md               |     13 +
 graft/kernel/src/net/socket.md                     |     45 +
 graft/kernel/src/net/tcp.md                        |    154 +
 graft/kernel/src/net/tls_record.md                 |     33 +
 graft/kernel/src/net/wifi/mlme_logic.md            |     82 +
 graft/kernel/src/net/wifi/mod.md                   |      4 +
 graft/kernel/src/net/wifi/sae_connect.md           |     29 +
 graft/kernel/src/net/wpa/bip.md                    |     31 +
 graft/kernel/src/net/wpa/ccmp.md                   |     32 +
 graft/kernel/src/net/wpa/eapol.md                  |     38 +
 graft/kernel/src/net/wpa/kdf.md                    |     11 +
 graft/kernel/src/net/wpa/keywrap.md                |      6 +
 graft/kernel/src/net/wpa/mod.md                    |      9 +
 graft/kernel/src/net/wpa/sae.md                    |     26 +
 graft/kernel/src/net/wpa/sae_sm.md                 |     20 +
 graft/kernel/src/observability.md                  |     55 +
 graft/kernel/src/panic.md                          |      4 +
 graft/kernel/src/rng/drbg.md                       |     13 +
 graft/kernel/src/rng/entropy.md                    |      7 +
 graft/kernel/src/rng/kat.md                        |      7 +
 graft/kernel/src/rng/mod.md                        |     10 +
 graft/kernel/src/scenario/mod.md                   |      4 +
 graft/kernel/src/scenario/phase_indicator.md       |      7 +
 graft/kernel/src/sched/file_cap_logic.md           |     28 +
 graft/kernel/src/sched/futex.md                    |     25 +
 graft/kernel/src/sched/gpu_compute_cap_logic.md    |      3 +
 graft/kernel/src/sched/gpu_submit_cap_logic.md     |      3 +
 graft/kernel/src/sched/kthread.md                  |     51 +
 graft/kernel/src/sched/mod.md                      |     24 +
 graft/kernel/src/sched/net_dial_cap_logic.md       |      7 +
 graft/kernel/src/sched/process.md                  |    222 +
 graft/kernel/src/sched/scheduler.md                |    154 +
 graft/kernel/src/sched/thread.md                   |     25 +
 graft/kernel/src/sched/tick_sleep.md               |     21 +
 graft/kernel/src/sched/wait_any.md                 |     38 +
 graft/kernel/src/sched/wait_any_logic.md           |     12 +
 graft/kernel/src/sched/wake_trace.md               |     54 +
 graft/kernel/src/sync/irqsafe_mutex.md             |     15 +
 graft/kernel/src/sync/lock_owner.md                |     14 +
 graft/kernel/src/sync/mod.md                       |     18 +
 graft/kernel/src/sync/switch_mutex.md              |     23 +
 graft/kernel/src/test_harness_latency_oracle.md    |     12 +
 graft/kernel/src/user_fault_logic.md               |     14 +
 graft/kernel/src/watchdog.md                       |     64 +
 graft/kernel/src/watchdog_timeline_logic.md        |      7 +
 graft/scripts/desktop/clock_oracle.md              |      5 +
 graft/scripts/desktop/field_oracle.md              |      9 +
 graft/scripts/desktop/intro_oracle.md              |      9 +
 graft/scripts/desktop/label_oracle.md              |     11 +
 graft/scripts/desktop/lens_content_oracle.md       |      8 +
 graft/scripts/desktop/scene_oracle.md              |     37 +
 graft/scripts/exp_f32_accuracy_oracle.md           |     12 +
 graft/scripts/fonts/area_oracle.md                 |     25 +
 graft/scripts/fonts/freetype_oracle.md             |     25 +
 graft/scripts/fonts/harfbuzz_kern_oracle.md        |      6 +
 graft/third_party/acpi-loganfy/src/address.md      |     16 +
 graft/third_party/acpi-loganfy/src/aml/mod.md      |    104 +
 .../third_party/acpi-loganfy/src/aml/namespace.md  |     49 +
 graft/third_party/acpi-loganfy/src/aml/object.md   |     52 +
 .../third_party/acpi-loganfy/src/aml/op_region.md  |     14 +
 .../acpi-loganfy/src/aml/pci_routing.md            |      8 +
 graft/third_party/acpi-loganfy/src/aml/resource.md |     34 +
 graft/third_party/acpi-loganfy/src/lib.md          |     67 +
 .../acpi-loganfy/src/platform/interrupt.md         |     14 +
 graft/third_party/acpi-loganfy/src/platform/mod.md |     19 +
 .../third_party/acpi-loganfy/src/platform/numa.md  |      8 +
 graft/third_party/acpi-loganfy/src/platform/pci.md |      6 +
 graft/third_party/acpi-loganfy/src/registers.md    |     13 +
 graft/third_party/acpi-loganfy/src/rsdp.md         |     22 +
 graft/third_party/acpi-loganfy/src/sdt/bgrt.md     |     10 +
 graft/third_party/acpi-loganfy/src/sdt/facs.md     |      3 +
 graft/third_party/acpi-loganfy/src/sdt/fadt.md     |     54 +
 graft/third_party/acpi-loganfy/src/sdt/hpet.md     |      8 +
 graft/third_party/acpi-loganfy/src/sdt/madt.md     |     38 +
 graft/third_party/acpi-loganfy/src/sdt/mcfg.md     |      8 +
 graft/third_party/acpi-loganfy/src/sdt/mod.md      |     92 +
 graft/third_party/acpi-loganfy/src/sdt/slit.md     |      9 +
 graft/third_party/acpi-loganfy/src/sdt/spcr.md     |     23 +
 graft/third_party/acpi-loganfy/src/sdt/srat.md     |     20 +
 graft/tools/aarch64-virtio-barrier-lane/build.md   |      3 +
 .../tools/aarch64-virtio-barrier-lane/src/main.md  |     97 +
 graft/tools/acpi-aml-grade/extract/ingest_set.md   |      6 +
 graft/tools/acpi-aml-grade/extract/qemu_extract.md |     10 +
 graft/tools/acpi-aml-grade/src/acpi6_body.md       |     50 +
 .../tools/acpi-aml-grade/src/bin/acpi-aml-fuzz.md  |     22 +
 graft/tools/acpi-aml-grade/src/classify.md         |     29 +
 graft/tools/acpi-aml-grade/src/corpus.md           |     17 +
 graft/tools/acpi-aml-grade/src/fuzz.md             |     24 +
 graft/tools/acpi-aml-grade/src/gates.md            |     19 +
 graft/tools/acpi-aml-grade/src/grade_acpi6.md      |      8 +
 graft/tools/acpi-aml-grade/src/grade_aml016.md     |     32 +
 graft/tools/acpi-aml-grade/src/lib.md              |     14 +
 graft/tools/acpi-aml-grade/src/main.md             |      3 +
 graft/tools/acpi-aml-grade/src/probe.md            |     13 +
 graft/tools/acpi-aml-grade/src/ratchet.md          |     11 +
 graft/tools/acpi-aml-grade/src/recorder.md         |     36 +
 graft/tools/acpi-aml-grade/src/report.md           |     22 +
 graft/tools/acpi-aml-grade/src/sha256.md           |      7 +
 graft/tools/acpi-aml-grade/src/summary.md          |     17 +
 graft/tools/acpi-aml-grade/tests/corpus.md         |      9 +
 graft/tools/acpi-aml-grade/tests/fix1_kats.md      |     25 +
 graft/tools/acpi-aml-grade/tests/fix2_kats.md      |     32 +
 graft/tools/acpi-aml-grade/tests/fix3_kats.md      |     35 +
 .../acpi-aml-grade/tests/jarvis_review_probe.md    |      7 +
 graft/tools/acpi-aml-grade/tests/panic_census.md   |      7 +
 .../tools/acpi-aml-grade/tests/zenbook_touchpad.md |      6 +
 graft/tools/acpi-timer-logic-tests/src/lib.md      |     29 +
 graft/tools/action-logic-tests/src/lib.md          |     73 +
 graft/tools/ai-ops-tests/src/lib.md                |    697 +
 .../ai-ops-tests/tests/mt7601u_hostile_oracle.md   |    132 +
 graft/tools/apic-logic-tests/src/lib.md            |     12 +
 graft/tools/arthur-grant-logic-tests/src/lib.md    |    112 +
 graft/tools/arthur-logic-tests/src/lib.md          |    109 +
 .../arthur-logic-tests/src/showcase_s2_tests.md    |     35 +
 .../atrium-logic-tests/oracle_gen/blur_oracle.md   |     16 +
 .../atrium-logic-tests/oracle_gen/blur_weights.md  |      3 +
 graft/tools/atrium-logic-tests/src/handoff_kats.md |     15 +
 .../atrium-logic-tests/src/input_queue_kats.md     |     19 +
 .../atrium-logic-tests/src/key_repeat_kats.md      |     28 +
 graft/tools/atrium-logic-tests/src/lib.md          |    136 +
 .../atrium-logic-tests/src/pointer_route_kats.md   |     16 +
 graft/tools/atrium-logic-tests/src/sak_kats.md     |     16 +
 .../atrium-logic-tests/src/shared_pixels_kats.md   |      8 +
 graft/tools/bench-harness/src/main.md              |     63 +
 graft/tools/block-guard-logic-tests/src/lib.md     |     47 +
 graft/tools/boot-eth0-logic-tests/src/lib.md       |     10 +
 graft/tools/branding/generate.md                   |     12 +
 graft/tools/fb-progress-logic-tests/src/lib.md     |     16 +
 graft/tools/file-cap-logic-tests/src/lib.md        |     22 +
 graft/tools/frame-guard-logic-tests/src/lib.md     |      9 +
 graft/tools/heap-policy-logic-tests/src/lib.md     |     19 +
 graft/tools/i2c-hid-logic-tests/src/dw_tests.md    |     18 +
 graft/tools/i2c-hid-logic-tests/src/hid_tests.md   |     17 +
 graft/tools/i2c-hid-logic-tests/src/lib.md         |     11 +
 graft/tools/i2c-hid-logic-tests/src/model.md       |     18 +
 .../tools/i2c-hid-logic-tests/src/pci_pm_tests.md  |     19 +
 graft/tools/i2c-hid-logic-tests/src/quirk_tests.md |     10 +
 .../tools/inference-latency-bench/src/bin/layer.md |     11 +
 graft/tools/inference-latency-bench/src/main.md    |     10 +
 .../interface-identity-logic-tests/src/lib.md      |     18 +
 graft/tools/lapic-timer-logic-tests/src/lib.md     |     17 +
 graft/tools/model-logic-tests/src/lib.md           |     42 +
 graft/tools/module-reclaim-logic-tests/src/lib.md  |     46 +
 graft/tools/neo-extract-log/src/main.md            |     43 +
 graft/tools/neo-shell-logic-tests/src/lib.md       |     18 +
 graft/tools/neofs-inspect.md                       |     33 +
 graft/tools/neofs-mkfs/src/format.md               |    163 +
 graft/tools/neofs-mkfs/src/main.md                 |     17 +
 graft/tools/neomc/src/fixtures.md                  |     29 +
 graft/tools/neomc/src/format.md                    |     50 +
 graft/tools/neomc/src/gguf_import.md               |     94 +
 graft/tools/neomc/src/importer_stub.md             |     12 +
 graft/tools/neomc/src/inspect.md                   |     10 +
 graft/tools/neomc/src/lib.md                       |      9 +
 graft/tools/neomc/src/main.md                      |     13 +
 graft/tools/neomc/src/validator.md                 |     14 +
 graft/tools/neomc/src/writer.md                    |     26 +
 graft/tools/neomc/tests/isc_tests.md               |     32 +
 graft/tools/neomc/tests/kv_persist_kat_fixture.md  |      4 +
 graft/tools/observability-abi-tests/src/lib.md     |     35 +
 graft/tools/real-lm-oracle/inventory_ref.md        |     10 +
 graft/tools/rtc-logic-tests/src/lib.md             |     40 +
 graft/tools/rtl8125-logic-tests/src/lib.md         |     46 +
 graft/tools/slab-magazine-tests/src/lib.md         |     40 +
 graft/tools/swgl-atrium-oracle/src/lib.md          |     20 +
 graft/tools/tensor-logic-tests/src/lib.md          |      9 +
 graft/tools/virtio-accel-host/src/main.md          |     73 +
 graft/tools/xhci-silicon-logic-tests/src/lib.md    |     36 +
 graft/tools/xtask/src/i8042_tests.md               |     19 +
 graft/tools/xtask/src/main.md                      |    659 +
 graft/tools/xtask/src/ot61_c05_tests.md            |     44 +
 graft/tools/xtask/src/ot83_tests.md                |     11 +
 graft/userland/fault-probe/build.md                |      3 +
 graft/userland/fault-probe/src/bin/db.md           |      4 +
 graft/userland/fault-probe/src/bin/de.md           |      4 +
 graft/userland/fault-probe/src/bin/gp.md           |      4 +
 graft/userland/fault-probe/src/bin/mf.md           |      4 +
 graft/userland/fault-probe/src/bin/ok.md           |      4 +
 graft/userland/fault-probe/src/bin/ss.md           |      4 +
 graft/userland/fault-probe/src/bin/ud.md           |      4 +
 graft/userland/fault-probe/src/bin/xm.md           |      4 +
 graft/userland/fault-probe/src/lib.md              |      6 +
 graft/userland/libneo/src/action.md                |     65 +
 graft/userland/libneo/src/atrium_client.md         |      8 +
 graft/userland/libneo/src/atrium_configure.md      |     22 +
 graft/userland/libneo/src/atrium_desktop.md        |     33 +
 graft/userland/libneo/src/atrium_handoff.md        |     14 +
 graft/userland/libneo/src/atrium_launch.md         |     29 +
 graft/userland/libneo/src/atrium_protocol.md       |     80 +
 graft/userland/libneo/src/atrium_wm.md             |     29 +
 graft/userland/libneo/src/civil.md                 |     24 +
 graft/userland/libneo/src/grant_store.md           |     82 +
 graft/userland/libneo/src/lib.md                   |    415 +
 graft/userland/libneo/src/nsync.md                 |     32 +
 graft/userland/libneo/src/nthread.md               |     17 +
 graft/userland/libneo/src/tls.md                   |     40 +
 graft/userland/libneo/src/wait_abi.md              |      7 +
 graft/userland/neo-arthur/build.md                 |      3 +
 .../neo-arthur/src/agentic_action_logic.md         |    137 +
 graft/userland/neo-arthur/src/arthur_lib.md        |      3 +
 .../userland/neo-arthur/src/conversation_logic.md  |     98 +
 graft/userland/neo-arthur/src/intent_logic.md      |     35 +
 graft/userland/neo-arthur/src/main.md              |    323 +
 graft/userland/neo-arthur/src/memory_crypto.md     |     22 +
 graft/userland/neo-arthur/src/memory_encryption.md |     15 +
 graft/userland/neo-arthur/src/memory_logic.md      |     30 +
 graft/userland/neo-arthur/src/model_state_logic.md |     11 +
 .../userland/neo-arthur/src/reversibility_logic.md |     20 +
 graft/userland/neo-arthur/src/showcase_logic.md    |     45 +
 graft/userland/neo-arthur/src/showcase_surface.md  |     35 +
 graft/userland/neo-arthur/src/showcase_text.md     |     51 +
 graft/userland/neo-arthur/src/vocab_sidecar.md     |     32 +
 graft/userland/neo-atrium/src/backdrop.md          |     10 +
 graft/userland/neo-atrium/src/blur.md              |     12 +
 graft/userland/neo-atrium/src/clock.md             |      8 +
 graft/userland/neo-atrium/src/contrast.md          |      5 +
 graft/userland/neo-atrium/src/intro.md             |     21 +
 graft/userland/neo-atrium/src/keys.md              |     15 +
 graft/userland/neo-atrium/src/layout.md            |     84 +
 graft/userland/neo-atrium/src/lib.md               |     16 +
 graft/userland/neo-atrium/src/material.md          |     13 +
 graft/userland/neo-atrium/src/motion.md            |     13 +
 graft/userland/neo-atrium/src/pointer_grab.md      |      6 +
 graft/userland/neo-atrium/src/scene.md             |     78 +
 graft/userland/neo-atrium/src/shape.md             |     10 +
 graft/userland/neo-atrium/src/tests.md             |    102 +
 graft/userland/neo-atrium/src/wide.md              |      9 +
 graft/userland/neo-init/build.md                   |      3 +
 graft/userland/neo-init/src/arthur_compositor.md   |     21 +
 graft/userland/neo-init/src/desktop_session.md     |    264 +
 graft/userland/neo-init/src/dw1_desktop_wm.md      |     68 +
 graft/userland/neo-init/src/main.md                |    597 +
 graft/userland/neo-shell/build.md                  |      3 +
 graft/userland/neo-shell/src/main.md               |     81 +
 graft/userland/neo-shell/src/shell_logic.md        |     40 +
 graft/userland/neo-std-pal/a4_gate.md              |     26 +
 graft/userland/neo-std-pal/build.md                |      5 +
 graft/userland/neo-std-pal/src/lib.md              |     42 +
 graft/userland/neo-text/build.md                   |      6 +
 graft/userland/neo-text/src/canvas.md              |     20 +
 graft/userland/neo-text/src/flow.md                |     16 +
 graft/userland/neo-text/src/font.md                |     27 +
 graft/userland/neo-text/src/fonts.md               |      4 +
 graft/userland/neo-text/src/layout.md              |     10 +
 graft/userland/neo-text/src/lib.md                 |      9 +
 graft/userland/neo-text/src/raster.md              |     18 +
 graft/userland/neo-text/src/tests/exact_a_13.md    |      7 +
 graft/userland/neo-text/src/tests/exact_g_20.md    |      7 +
 graft/userland/neo-text/src/tests/exact_o_24.md    |      7 +
 graft/userland/neo-text/src/tests/exact_s_32.md    |      7 +
 .../userland/neo-text/src/tests/font_integrity.md  |     11 +
 graft/userland/neo-text/src/tests/ft_a_13.md       |      7 +
 graft/userland/neo-text/src/tests/ft_g_20.md       |      7 +
 graft/userland/neo-text/src/tests/ft_o_24.md       |      7 +
 graft/userland/neo-text/src/tests/ft_s_32.md       |      7 +
 graft/userland/neo-text/src/tests/hb_kern_sans.md  |      4 +
 graft/userland/neo-text/src/tests/mod.md           |     42 +
 graft/userland/neo-worker/build.md                 |      3 +
 graft/userland/neo-worker/src/lens.md              |     23 +
 graft/userland/neo-worker/src/main.md              |     84 +
 .../neoplutarch-std-overlay/apply-overlay.md       |     10 +
 .../std/alloc_neoplutarch.md                       |      6 +
 .../std/io_error_neoplutarch.md                    |      8 +
 .../neoplutarch-std-overlay/std/neoplutarch_rt.md  |     12 +
 .../std/stdio_neoplutarch.md                       |     20 +
 graft/userland/std-pal-smoke/src/main.md           |      6 +
 graft/userland/std-real-smoke/src/main.md          |      7 +
 graft/userland/std-real-smoke/src/panic.md         |      4 +
 graft/userland/template-app/build.md               |      3 +
 graft/userland/template-app/src/main.md            |      4 +
 graft/userland/vault-core/src/lib.md               |     31 +
 graft/userland/vault-core/src/policy.md            |      9 +
 graft/userland/vault-core/src/protocol.md          |     39 +
 graft/userland/vault-core/src/record.md            |     19 +
 graft/userland/vault-core/src/session.md           |     39 +
 tools/acpi-aml-grade/tests/jarvis_review_probe.rs  |     50 +
 539 files changed, 392570 insertions(+)
```

Non-cache paths: `tools/acpi-aml-grade/tests/jarvis_review_probe.rs`.

## Copy pushed and contents

`stash@{2}` is the only stash with other files. The exact command was:
```powershell
git push origin 'stash@{2}:refs/heads/rescue/jarvis-stash-2-2026-10-26'
```

Pushed SHA: `f2dd94c5a0585b89e9b6469746b46ca8abe61fc3`.
This is the original stash commit, without flattening or replaying it.
Its parents are preserved by the push:

- Base: `3fa65eb85522918308f4fc151a500e5a09266cf5`.
- Index: `7509c9026d9b80a753888597b0a051c785345f1f`.
- Untracked: `ca3a44f35e0b5148b2c30b7e7d0f2279a09846ac`.

`stash@{0}` and `stash@{1}` each contain only `.gitignore` and `.ignore`;
no copy was required for those cache-only stashes. Their file edits were
already saved by #81 on `rescue/jarvis-tree-2026-10-26`.

The only non-cache file in `stash@{2}` is
`tools/acpi-aml-grade/tests/jarvis_review_probe.rs`, a 50-line untracked
integration-test file. It looks like Jarvis ACPI review probes: four tests
exercise nine-byte ToInteger, oversized Concat, Mid buffer/string edge
cases, and Index at the end of a buffer. This describes the file only;
no claim is made that its expected outcomes are current or correct.
The stash label names a later model-load review because that relay preserved
pre-existing local material; the label does not mean this test belongs to
that model-load implementation. Authorship cannot be established from the
stash label alone.

Complete file patch extracted from
`git stash show -p --include-untracked 'stash@{2}'`:
```diff
diff --git a/tools/acpi-aml-grade/tests/jarvis_review_probe.rs b/tools/acpi-aml-grade/tests/jarvis_review_probe.rs
new file mode 100644
index 00000000..ac144db9
--- /dev/null
+++ b/tools/acpi-aml-grade/tests/jarvis_review_probe.rs
@@ -0,0 +1,50 @@
+use acpi_aml_grade::{corpus::load_set, grade_acpi6::fix1};
+use std::path::PathBuf;
+
+fn run(bytes: &[u8]) -> acpi_aml_grade::report::Kat {
+    let set = load_set(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/acpi/qemu-q35-seabios-smp1")).unwrap();
+    fix1::run_kat(set.find("FACP").unwrap(), 2, bytes, &["\\MAIN"])
+}
+
+#[test]
+fn jarvis_to_integer_nine_bytes() {
+    // Method MAIN { Return(ToInteger(Buffer(9){}, NullName)) }
+    let r = run(&[0x14,0x16,b'M',b'A',b'I',b'N',0,0xa4,0x99,0x11,0x0c,0x0a,9,0,0,0,0,0,0,0,0,0]);
+    eprintln!("nine-byte ToInteger: {r:?}");
+    assert!(matches!(&r.results[0], Err(e) if e.contains("panic:") && e.contains("copy_from_slice")));
+}
+
+#[test]
+fn jarvis_concat_over_limit() {
+    // Method MAIN { Store(Buffer(0x100000){},Local0);
+    //               Return(SizeOf(Concat(Local0,Local0,NullName))) }
+    let r = run(&[0x14,0x15,b'M',b'A',b'I',b'N',0,
+        0x70,0x11,0x06,0x0c,0,0,0x10,0,0x60,
+        0xa4,0x87,0x73,0x60,0x60,0]);
+    eprintln!("Concat size: {r:?}");
+    assert_eq!(r.results, vec![Ok("Integer(2097152)".into())]);
+}
+
+#[test]
+fn jarvis_mid_buffer_and_empty() {
+    // Three MAIN variants: Buffer Mid clips to one byte, then empty Buffer/String.
+    for (body, expected) in [
+        (vec![0xa4,0x9e,0x11,5,0x0a,2,0x41,0x42,1,0x0a,5,0], "Buffer([42])"),
+        (vec![0xa4,0x9e,0x11,5,0x0a,2,0x41,0x42,0x0a,2,1,0], "Buffer([])"),
+        (vec![0xa4,0x9e,0x0d,0x41,0x42,0,0x0a,2,1,0], "String(\"\")"),
+    ] {
+        let mut bytes = vec![0x14, (6+body.len()) as u8,b'M',b'A',b'I',b'N',0];
+        bytes.extend(body);
+        let r = run(&bytes);
+        eprintln!("Mid: {r:?}");
+        assert_eq!(r.results, vec![Ok(expected.into())]);
+    }
+}
+
+#[test]
+fn jarvis_index_at_end() {
+    // Method MAIN { Return(Index(Buffer(1){0xAA}, One, NullName)) }
+    let r = run(&[0x14,0x12,b'M',b'A',b'I',b'N',0,0xa4,0x88,0x11,0x04,0x0a,1,0xaa,1,0]);
+    eprintln!("Index at end: {r:?}");
+    assert_eq!(r.results, vec![Err("IndexOutOfBounds".into())]);
+}
```

**Main status: not on main (the file itself).** At `origin/main` commit
`47f5991d493d6f5249e9c4394dc269d8c7b09181`, the direct single-file comparison
`git ls-tree origin/main tools/acpi-aml-grade/tests/jarvis_review_probe.rs`
returned no entry. The file exists in the stash's untracked parent with
blob `ac144db9344db03ea8b8b441b770ab8efa44310b`:
```powershell
git rev-parse 'stash@{2}^3:tools/acpi-aml-grade/tests/jarvis_review_probe.rs'
```
This is the dispatch's single-file comparison alternative to `git cherry`.
It establishes absence of this path on main, not absence of equivalent test
coverage elsewhere. No supersession is claimed without a code review.

## RETURN delivery and close checks

The RETURN is committed in the dispatch-specified temporary detached worktree
`C:\Users\<user>\AppData\Local\Temp\jarvis-rescue`, based on
`origin/rescue/jarvis-tree-2026-10-26` at
`5d0c9f3f3e4853f29174f4a26e71c608f8e55592`, then pushed with
`HEAD:rescue/jarvis-tree-2026-10-26`. The main working branch is untouched.
The temporary worktree is removed only after its clean commit is pushed,
using the dispatch's `git worktree remove` step. No original content is deleted.

Final remote-ref and unchanged-tree/stash checks will be reported with the
RETURN SHA in chat; the unchanged status must remain:
```text
 M .gitignore
?? .ignore
```
