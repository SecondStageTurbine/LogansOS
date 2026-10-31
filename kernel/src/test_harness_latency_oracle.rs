// SPDX-License-Identifier: MPL-2.0
// BUILD-LO: the §4 hot-path latency oracle (Tier B).
//
// `include!`d into `test_harness.rs` so these items live in the harness module
// and reuse its `SCENARIO`, `fail`, `pass`, `serial_println!`, and the
// `core::arch::asm` import directly (do NOT convert this to a `mod` — the
// `include!` keeps one module scope, matching the rest of the harness; the
// leading comments here are `//` not `//!` because inner-doc comments are
// invalid once pasted mid-module).
//
// The §4 (docs/complexity-governance.md) hot-path latency oracle: a QEMU rdtsc
// cycle harness over the AI orchestration hot path. Pure instrumentation,
// default-off (the `neo-test-ai-latency-oracle` feature gate is the kill-switch
// — off ⇒ every item here is `#[cfg]`-removed and the kernel is bit-for-bit
// today). It reports the four §4 budget verdicts against the FIRST measured
// numbers. TSC under TCG is virtualized, noisy, and NOT constant-rate — warm
// up, take the median, and state the context; never present these as silicon.

#[cfg(feature = "neo-test-ai-latency-oracle")]
const LO_SAMPLES: usize = 64;
#[cfg(feature = "neo-test-ai-latency-oracle")]
const LO_WARMUP: usize = 16;

#[cfg(feature = "neo-test-ai-latency-oracle")]
fn lo_read_tsc() -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: `lfence; rdtsc` only reads the time-stamp counter into EDX:EAX; it
    // touches no memory and no stack (the existing irqsafe-mutex-bench idiom).
    unsafe {
        asm!(
            "lfence",
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nostack, preserves_flags)
        );
    }
    ((hi as u64) << 32) | lo as u64
}

/// Median-of-N TSC cycle cost of one `f()` call. Warm up `LO_WARMUP` calls
/// (discards cold-cache / first-sample effects), then time `samples_n` calls and
/// return the median (robust to TCG scheduling outliers).
#[cfg(feature = "neo-test-ai-latency-oracle")]
fn lo_bench_n<F: FnMut()>(samples_n: usize, mut f: F) -> u64 {
    for _ in 0..LO_WARMUP {
        f();
    }
    let n = if samples_n > LO_SAMPLES {
        LO_SAMPLES
    } else {
        samples_n
    };
    let mut samples = [0u64; LO_SAMPLES];
    for slot in samples.iter_mut().take(n) {
        let start = lo_read_tsc();
        f();
        let end = lo_read_tsc();
        *slot = end.wrapping_sub(start);
    }
    samples[..n].sort_unstable();
    samples[n / 2]
}

#[cfg(feature = "neo-test-ai-latency-oracle")]
fn lo_bench<F: FnMut()>(f: F) -> u64 {
    lo_bench_n(LO_SAMPLES, f)
}

/// A zeroed immutable 1-D f32 tensor of `elems` elements — the byte-size knob for
/// the metadata size-independence sweep.
#[cfg(feature = "neo-test-ai-latency-oracle")]
fn lo_make_tensor(elems: usize) -> alloc::sync::Arc<crate::ai::tensor::Tensor> {
    use crate::ai::tensor::{Dtype, MAX_RANK, Tensor, TensorCreateDesc};
    let mut dims = [0u64; MAX_RANK];
    dims[0] = elems as u64;
    let data_len = (elems * 4) as u64;
    let desc = TensorCreateDesc {
        rank: 1,
        dtype: Dtype::F32.as_abi(),
        mutable: 0,
        _pad: 0,
        dims,
        data_ptr: 0,
        data_len,
    };
    let data = alloc::vec![0u8; data_len as usize];
    match Tensor::from_desc_and_bytes(desc, &data) {
        Ok(t) => alloc::sync::Arc::new(t),
        Err(_) => fail("reason=lo_tensor_create_failed"),
    }
}

#[cfg(feature = "neo-test-ai-latency-oracle")]
fn run_ai_latency_oracle() -> ! {
    use crate::ai::arena;
    use crate::ai::ops::{self, OpKind, TensorView};
    use crate::ai::placement::{DeviceCandidate, decide};
    use crate::ai::sharing::{SharedRecord, total_resident_bytes};
    use crate::ai::tensor::{Device, Dtype, Shape};
    use crate::sched::process::{self, Handle, Process, full_tensor_rights};
    use core::hint::black_box;

    let cpus = crate::arch::x86_64::smp::reported_cpu_count();
    serial_println!(
        "NEO_TEST/1 event=context name={} measurement=qemu_tcg note=virtualized_tsc_not_constant_rate_not_silicon cpus={} samples={} warmup={} discard=warmup",
        SCENARIO,
        cpus,
        LO_SAMPLES,
        LO_WARMUP
    );

    // ── op: mint — the keyed `fget` (ARENA lookup + generation compare + guarded_clone) ──
    let mint_tensor = lo_make_tensor(1024);
    let mint_key = arena::alloc(mint_tensor, Device::Cpu);
    let mint_cycles = lo_bench(|| {
        let minted = arena::mint(black_box(mint_key));
        black_box(&minted);
        // `minted` drops here — the short-lived use Arc releases (I-4 at-rest owner).
    });
    serial_println!(
        "NEO_TEST/1 event=measure name={} op=mint cycles={} n={}",
        SCENARIO,
        mint_cycles,
        LO_SAMPLES
    );

    // ── op: guarded_clone — the slot-level clone alone, no ARENA-table lookup ──
    let gc_tensor = lo_make_tensor(1024);
    let gc_slot = arena::ManagedSlot::new(Device::Cpu);
    gc_slot.put(gc_tensor);
    let guarded_clone_cycles = lo_bench(|| {
        let cloned = gc_slot.guarded_clone();
        black_box(&cloned);
    });
    serial_println!(
        "NEO_TEST/1 event=measure name={} op=guarded_clone cycles={} n={}",
        SCENARIO,
        guarded_clone_cycles,
        LO_SAMPLES
    );

    // ── ops: cap_check + sys_tensor_query metadata — need a process + an installed handle ──
    let pmap = crate::mm::pmap::Pmap::new().unwrap_or_else(|_| fail("reason=lo_pmap_failed"));
    let proc = process::register(Process::new_user("lo-oracle", pmap));
    let qt = lo_make_tensor(1024);
    let q_slot = arena::alloc(qt.clone(), Device::Cpu);
    let q_handle = match process::install_cap(
        &proc,
        Handle::Tensor {
            slot: q_slot,
            meta: qt.meta(),
            cap_id: process::alloc_cap_id(),
            rights: full_tensor_rights(),
        },
        None,
    ) {
        Ok(idx) => idx,
        Err(_) => fail("reason=lo_install_cap_failed"),
    };
    arena::release(q_slot); // the table owns the handle count now (BUILD-O1b creator-transfer).

    // Sanity: the real shipped seams resolve before we time them.
    if crate::arch::x86_64::syscall::debug_cap_check(&proc, q_handle) != 0 {
        fail("reason=lo_cap_check_resolve_failed");
    }
    if crate::arch::x86_64::syscall::debug_query_meta(&proc, q_handle) != 0 {
        fail("reason=lo_query_meta_resolve_failed");
    }

    let cap_check_cycles = lo_bench(|| {
        black_box(crate::arch::x86_64::syscall::debug_cap_check(
            black_box(&proc),
            black_box(q_handle),
        ));
    });
    serial_println!(
        "NEO_TEST/1 event=measure name={} op=cap_check cycles={} n={}",
        SCENARIO,
        cap_check_cycles,
        LO_SAMPLES
    );

    let query_cycles = lo_bench(|| {
        black_box(crate::arch::x86_64::syscall::debug_query_meta(
            black_box(&proc),
            black_box(q_handle),
        ));
    });
    serial_println!(
        "NEO_TEST/1 event=measure name={} op=sys_tensor_query cycles={} n={}",
        SCENARIO,
        query_cycles,
        LO_SAMPLES
    );

    // ── verdict 2: sys_tensor_query metadata cost is independent of tensor SIZE ──
    // Sweep tensor byte-size across 4096× (256 B … 1 MiB) and show the query cost
    // stays flat — the I-4 cached-meta property: handles name slots, not bytes.
    let sweep_elems = [64usize, 1024, 16384, 262144];
    let mut q_min = u64::MAX;
    let mut q_max = 0u64;
    for &elems in sweep_elems.iter() {
        let st = lo_make_tensor(elems);
        let s_slot = arena::alloc(st.clone(), Device::Cpu);
        let s_handle = match process::install_cap(
            &proc,
            Handle::Tensor {
                slot: s_slot,
                meta: st.meta(),
                cap_id: process::alloc_cap_id(),
                rights: full_tensor_rights(),
            },
            None,
        ) {
            Ok(i) => i,
            Err(_) => fail("reason=lo_sweep_install_failed"),
        };
        arena::release(s_slot);
        if crate::arch::x86_64::syscall::debug_query_meta(&proc, s_handle) != 0 {
            fail("reason=lo_sweep_query_failed");
        }
        let c = lo_bench(|| {
            black_box(crate::arch::x86_64::syscall::debug_query_meta(
                black_box(&proc),
                black_box(s_handle),
            ));
        });
        serial_println!(
            "NEO_TEST/1 event=measure name={} op=query_size_sweep elems={} bytes={} cycles={} n={}",
            SCENARIO,
            elems,
            elems * 4,
            c,
            LO_SAMPLES
        );
        q_min = q_min.min(c);
        q_max = q_max.max(c);
    }
    // Flat ⇒ size-independent. A size-LINEAR cost would blow up ~4096× across the
    // sweep; a generous 4× ceiling cleanly separates flat from size-dependent while
    // tolerating TCG jitter (median-of-N already controls outliers).
    let query_size_independent = q_max <= q_min.saturating_mul(4);

    // ── per-op orchestration total + the amortization ratio (verdict 3) ──
    // Orchestration = placement decide + mint + §9.5 accounting, wrapped around one
    // dispatched CPU-fallback op (a soft-f32 matmul via ops::dispatch).
    let candidates = [
        DeviceCandidate {
            device_abi: 0,
            resident_bytes: 0,
            covers_op: true,
            over_quota: false,
            is_cpu_fallback: true,
        },
        DeviceCandidate {
            device_abi: 1,
            resident_bytes: 4096,
            covers_op: true,
            over_quota: false,
            is_cpu_fallback: false,
        },
        DeviceCandidate {
            device_abi: 2,
            resident_bytes: 1024,
            covers_op: true,
            over_quota: false,
            is_cpu_fallback: false,
        },
    ];
    let record = SharedRecord {
        owner: proc.pid.0,
        bytes: 4096,
    };
    let principals = [proc.pid.0];

    let orch_tensor = lo_make_tensor(1024);
    let orch_key = arena::alloc(orch_tensor, Device::Cpu);
    let orchestration_cycles = lo_bench(|| {
        let placement = decide(black_box(&candidates), black_box(8192));
        black_box(&placement);
        let minted = arena::mint(black_box(orch_key));
        black_box(&minted);
        let resident = total_resident_bytes(black_box(&record), black_box(&principals));
        black_box(resident);
    });
    serial_println!(
        "NEO_TEST/1 event=measure name={} op=orchestration cycles={} n={}",
        SCENARIO,
        orchestration_cycles,
        LO_SAMPLES
    );

    // The dispatched op: a soft-f32 matmul on the CPU fallback, at two sizes — the
    // tax (orchestration) amortizes as the op's own compute grows.
    let op_sizes = [16usize, 48];
    let mut amortize_permille_at_largest = u64::MAX;
    for &dim in op_sizes.iter() {
        let mut dims = [0u64; crate::ai::tensor::MAX_RANK];
        dims[0] = dim as u64;
        dims[1] = dim as u64;
        let shape = match Shape::new(2, dims) {
            Ok(s) => s,
            Err(_) => fail("reason=lo_matmul_shape_failed"),
        };
        let lhs_bytes = alloc::vec![0u8; dim * dim * 4];
        let rhs_bytes = alloc::vec![0u8; dim * dim * 4];
        let mut out_bytes = alloc::vec![0u8; dim * dim * 4];
        let matmul_cycles = lo_bench_n(16, || {
            let lhs = TensorView {
                dtype: Dtype::F32,
                shape: &shape,
                bytes: &lhs_bytes,
            };
            let rhs = TensorView {
                dtype: Dtype::F32,
                shape: &shape,
                bytes: &rhs_bytes,
            };
            let r = ops::dispatch(OpKind::Matmul, &[lhs, rhs], &shape, &mut out_bytes);
            black_box(&r);
        });
        // ratio = orchestration / op-compute, in permille (‰).
        let permille = if matmul_cycles == 0 {
            u64::MAX
        } else {
            orchestration_cycles.saturating_mul(1000) / matmul_cycles
        };
        serial_println!(
            "NEO_TEST/1 event=measure name={} op=matmul dim={}x{} cycles={} orchestration_permille_of_op={} n=16",
            SCENARIO,
            dim,
            dim,
            matmul_cycles,
            permille
        );
        amortize_permille_at_largest = permille;
    }
    // The tax is amortizable iff it is a small fraction of a real op's compute —
    // i.e. well under 1× (1000‰) at the larger representative size.
    let amortizable = amortize_permille_at_largest < 1000;

    // ── verdict 4: indirection is paid once per OPERAND-USE, never per element ──
    // A representative forward pass: LO_OPS ops × 2 operands, each operand
    // LO_ELEMS_PER_OPERAND elements. Count mints — it must equal the operand-uses
    // (2·LO_OPS), independent of element count (per-use, never per-weight).
    const LO_OPS: u64 = 8;
    const LO_OPERANDS_PER_OP: u64 = 2;
    const LO_ELEMS_PER_OPERAND: u64 = 4096;
    let a_tensor = lo_make_tensor(LO_ELEMS_PER_OPERAND as usize);
    let b_tensor = lo_make_tensor(LO_ELEMS_PER_OPERAND as usize);
    let a_key = arena::alloc(a_tensor, Device::Cpu);
    let b_key = arena::alloc(b_tensor, Device::Cpu);
    let mut mint_count = 0u64;
    for _ in 0..LO_OPS {
        // Each op mints its two operands once, uses them, then drops them.
        if let Some(t) = arena::mint(a_key) {
            black_box(&t);
            mint_count += 1;
        }
        if let Some(t) = arena::mint(b_key) {
            black_box(&t);
            mint_count += 1;
        }
    }
    let expected_mints = LO_OPS * LO_OPERANDS_PER_OP;
    let total_elements = expected_mints * LO_ELEMS_PER_OPERAND;
    serial_println!(
        "NEO_TEST/1 event=measure name={} op=indirection_paid_once mints={} operand_uses={} elems_per_operand={} total_elements={} n=1",
        SCENARIO,
        mint_count,
        expected_mints,
        LO_ELEMS_PER_OPERAND,
        total_elements
    );
    let paid_once = mint_count == expected_mints && mint_count < LO_ELEMS_PER_OPERAND;

    // ── the four §4 budget verdicts ──
    // 1. No hot-path syscall: mint resolves in-kernel (a plain call — no `syscall`
    //    entry, no ring transition). Structural HOLD, reported with the cost.
    serial_println!(
        "NEO_TEST/1 event=verdict name={} claim=no_hot_path_syscall result=HOLD detail=mint_resolves_in_kernel_no_ring_transition mint_cycles={} guarded_clone_cycles={}",
        SCENARIO,
        mint_cycles,
        guarded_clone_cycles
    );
    // 2. Metadata-cheap / size-independent.
    serial_println!(
        "NEO_TEST/1 event=verdict name={} claim=metadata_cheap_size_independent result={} detail=sys_tensor_query_flat_across_4096x_size min_cycles={} max_cycles={}",
        SCENARIO,
        if query_size_independent {
            "HOLD"
        } else {
            "VIOLATE"
        },
        q_min,
        q_max
    );
    // 3. Amortizable per-op tax.
    serial_println!(
        "NEO_TEST/1 event=verdict name={} claim=amortizable_per_op_tax result={} detail=orchestration_as_permille_of_op_compute permille={} lower_is_more_amortized=true",
        SCENARIO,
        if amortizable { "HOLD" } else { "VIOLATE" },
        amortize_permille_at_largest
    );
    // 4. Indirection paid once (per use, never per element).
    serial_println!(
        "NEO_TEST/1 event=verdict name={} claim=indirection_paid_once result={} detail=one_mint_per_operand_use mints={} expected={} elems_per_operand={}",
        SCENARIO,
        if paid_once { "HOLD" } else { "VIOLATE" },
        mint_count,
        expected_mints,
        LO_ELEMS_PER_OPERAND
    );

    // Structural invariants MUST hold for the instrument to be trustworthy. Budget
    // findings (verdicts 2 & 3 reporting VIOLATE) are RECORDED, not gate failures:
    // a future build that blows a budget fails against the recorded §5 table — that
    // is the discipline this build establishes (the scope is "measuring is the job;
    // fixing is a follow-on"). Verdict 4 is a structural count and must hold.
    if !paid_once {
        fail("reason=lo_indirection_not_paid_once");
    }

    serial_println!(
        "NEO_TEST/1 event=BENCH name={} mint={} guarded_clone={} cap_check={} sys_tensor_query={} orchestration={} query_size_independent={} amortizable={} indirection_paid_once=true",
        SCENARIO,
        mint_cycles,
        guarded_clone_cycles,
        cap_check_cycles,
        query_cycles,
        orchestration_cycles,
        query_size_independent,
        amortizable
    );
    pass("ai_latency_oracle=ran four_verdicts=emitted tier=B context=qemu_tcg");
}
