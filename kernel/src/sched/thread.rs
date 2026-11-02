// SPDX-License-Identifier: MPL-2.0
//! Kernel thread struct + initial stack frame construction.
//!
//! A `Thread` owns its kernel stack (except the bootstrap "main" thread, which
//! reuses the Limine-supplied entry stack — see `adopt_current`). Every spawned
//! thread starts life with a hand-built stack frame that, on the first
//! `context_switch` into it, will pop six zeroed callee-saved registers and
//! `ret` into the thread's entry function.
//!
//! Initial stack frame layout (low addresses → high addresses):
//!
//!   [r15 = 0                  ] <- initial RSP (lowest written slot)
//!   [r14 = 0                  ]
//!   [r13 = 0                  ]
//!   [r12 = 0                  ]
//!   [rbp = 0                  ]
//!   [rbx = 0                  ]
//!   [thread_start_trampoline  ] <- first context_switch `ret` pops this
//!   [entry_fn                 ] <- 16-byte aligned; trampoline `ret` pops this
//!   [exit_trampoline          ] <- defense-in-depth; `ret` from entry_fn pops this
//!
//! The trampoline does `sti; ret` — without it, threads start with IF=0
//! (cooperative `context_switch` doesn't restore RFLAGS) and Phase 1.3
//! preemption can never fire on them.
//!
//! The asm in `arch::x86_64::context` pushes rbx,rbp,r12-r15 (so r15 ends at
//! the lowest address) and pops in the inverse order, so this initial frame
//! mirrors the post-save state byte-exact.
//!
//! Stack backing: `Box<[u128]>`. `u128` has 16-byte alignment, so the heap
//! allocation is 16-byte aligned (required for SysV-emitted `movaps`). `vec!`
//! zero-initializes the buffer. Drop is automatic.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use core::mem::ManuallyDrop;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use x86_64::align_down;

#[cfg(feature = "browser-runtime")]
use crate::arch::x86_64::context::enter_userspace_arg_trampoline;
use crate::arch::x86_64::context::{
    enter_userspace_trampoline, thread_exit_trampoline, thread_start_trampoline,
};
#[cfg(feature = "xsave-fpu")]
use crate::arch::x86_64::fpu::FpuState;
use crate::arch::x86_64::gdt::{USER_CODE_SEL_RAW, USER_DATA_SEL_RAW, USER_RFLAGS_RAW};
use crate::arch::x86_64::without_interrupts;
use crate::mm::pmap::Pmap;
use crate::mm::slab::THREAD_CACHE;

use super::process::{KERNEL_PID, ProcessId};

/// Stack size for spawned kernel threads. Eight 4 KiB pages by default.
#[cfg(not(feature = "neo-test-arthur-real-brain-boot"))]
pub const STACK_SIZE: usize = 32 * 1024;

/// Stage-B runs the full real-model forward inside the inference syscall.
/// Keep existing scenarios on the historical stack size, but give the
/// real-brain integration boot enough syscall stack headroom to execute the
/// landed F1/OT-30 path without corrupting the return frame.
#[cfg(feature = "neo-test-arthur-real-brain-boot")]
pub const STACK_SIZE: usize = 256 * 1024;
const STACK_WORDS: usize = STACK_SIZE / 16; // u128 = 16 bytes

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ThreadId(pub u64);

/// Phase 1.3 reintroduces ThreadState — this time with a real consumer:
/// `sched::exit()` flips the current thread to `Zombie` before the final
/// switch-away, and the reaper checks state when draining the zombie queue.
///
/// Phase 1.5b adds `Blocked`: a thread parked inside an external structure
/// (currently only an IPC `EndpointState`). Blocked threads are NOT in the
/// ready queue, NOT current, NOT in the zombie queue — they are owned by
/// the structure that parked them. The reaper never sees them; the parking
/// structure is responsible for either completing the rendezvous (which
/// transitions the thread back to `Ready` and pushes it onto the ready
/// queue) or, on process exit, signalling the peer cleanup path.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Zombie,
}

pub struct Thread {
    pub(super) id: ThreadId,
    /// Process this thread belongs to. Phase 1.2/1.3 threads all carry
    /// `KERNEL_PID`; Phase 1.4+ user threads carry the user process's pid.
    /// The scheduler uses this to detect cross-process switches.
    pub(super) pid: ProcessId,
    /// Address space to run this thread under. Kernel threads carry `None`.
    /// User threads carry their process Pmap directly so context switching
    /// never has to look through the global process table for CR3.
    pub(super) pmap: Option<Arc<Pmap>>,
    /// OT-61 C1.1 (cross-model AAG finding 2): **scheduler-owned lifetime anchor
    /// for the owning `Process`.**
    ///
    /// CP-30 requires a user-blocking syscall to drop every owned `Arc` before
    /// parking, because force-zombify frees the kernel stack raw and would leak
    /// anything owned there. `thread_join` therefore keeps only RAW pointers to
    /// the process registry and the target's done-word. That is leak-safe but
    /// NOT lifetime-safe: a valid join-wake can make the joiner Ready→Running
    /// while a concurrent `terminate` drops the last `Arc<Process>` — the joiner
    /// is invisible to both the ready-only kill scan and the parked-only token
    /// scan — and it then dereferences freed memory (a real UAF, built as a
    /// concrete cross-CPU interleaving by the Codex cross-check).
    ///
    /// Holding the `Arc` HERE resolves both at once: it lives in the `Thread`,
    /// which the scheduler owns, so `Thread::free` drops it explicitly (no CP-30
    /// leak — the suspendable kernel stack owns nothing), while guaranteeing the
    /// `Process` outlives every thread that can still dereference it.
    ///
    /// Cycle note: `Process` transitively owns parked `Box<Thread>` values (its
    /// wait queue, and IPC endpoints reachable from its handle table), so a
    /// mutual wait between two processes can form an Arc cycle. It is bounded
    /// and non-fatal — teardown drains those queues and zombifies the threads,
    /// and `Thread::free` clears this field, breaking the cycle. A bounded leak
    /// in a pathological mutual-wait is strictly better than a UAF.
    pub(super) process: Option<ProcessAnchor>,
    pub(super) name: &'static str,
    pub(super) state: ThreadState,
    /// Current CPU placement for ready/running threads. Blocked threads carry
    /// the CPU that last owned them until a Q4 wake path re-stamps them.
    pub cpu_id: AtomicU32,
    /// Stack ownership. `None` for the adopted bootstrap thread (its stack is
    /// the Limine-provided entry stack, which lives outside the heap).
    /// Held to keep the heap allocation alive for the thread's lifetime.
    #[allow(dead_code)]
    pub(super) stack: Option<Box<[u128]>>,
    /// Per-thread x87/SSE/AVX image. `None` only while `Thread::free` is
    /// explicitly dropping owned fields before returning raw slab storage.
    /// Every constructor starts it at `FpuState::new_initial()`, never a copy
    /// of the creating context's live registers: that copy is restored on the
    /// thread's first switch-in and would hand kernel register contents to a
    /// new ring-3 thread, or ring-3-chosen MXCSR to a new kernel thread (CP-45).
    #[cfg(feature = "xsave-fpu")]
    pub(super) fpu_state: Option<FpuState>,
    /// Saved stack pointer. For the adopted main thread this is 0 until the
    /// first switch-out, when `context_switch` writes the live RSP into it.
    pub(super) rsp: u64,
    /// Scratch slot that a blocking syscall returns through. The rendezvous
    /// waker (or the process-exit cleanup path) writes the return value here
    /// before pushing the thread back onto the ready queue. The thread reads
    /// it once context_switch resumes inside the syscall body. Default `0`
    /// is harmless for a thread that never blocks. Phase 2 SMP candidate
    /// for AtomicI64 (relaxed load suffices — wake's ready-queue push under
    /// SCHED.lock provides the acquire barrier).
    pub(super) blocked_return: i64,
    /// Bounded preemption immunity used by IPC rendezvous handoff. Cleared on
    /// syscall entry so the credit cannot accumulate across syscalls.
    pub(super) preempt_defer_ticks: u8,
    /// B1c AAG J3/J4 (CP-65): the thread is inside an inference syscall, from
    /// the arm's entry to the boundary exit decision. Force-kill never
    /// zombifies it; it cancels cooperatively and exits at the boundary.
    #[cfg(feature = "inference-stack")]
    pub(super) inference_lifecycle: bool,
    /// CP-66 (J3-P): set while this thread is inside the accelerator admission
    /// section (`admission::run_scheduled`: parked behind a holder, or the
    /// holder preempted mid-dispatch), where its kernel stack owns the
    /// request's `Arc`s. The force-kill scans never zombify such a thread from
    /// a queue: `admission::cancel_pid` cancels it and it exits at the syscall
    /// boundary once its frames have unwound. Written by the thread itself
    /// under its CPU's scheduler lock; read by the scans under the lock of the
    /// queue that holds it.
    pub(super) admission_section: bool,
    /// OT-93 (CP-95): the thread is inside a syscall, from `syscall_dispatch`'s
    /// entry to its return decision. A Ready thread with this set resumes in
    /// kernel mode with a live syscall frame, which may own `Arc`s and `Vec`s
    /// (a rendezvous sender holds both pmaps, CP-95). Force-kill never
    /// zombifies it: the thread sees its process's forced exit at its syscall
    /// boundary (`scheduler::leave_syscall`, or a park it would start) and
    /// exits there, with its frames unwound. Written by the thread under its CPU's scheduler lock;
    /// read by the kill scan under the lock of the queue that holds it.
    pub(super) in_syscall: bool,
    /// OT-61 C2.2b: what this thread's creation charged its process's
    /// account (`SYS_THREAD_CREATE`); released by `Thread::free`.
    #[cfg(feature = "obj-quota")]
    obj_charge: Option<crate::mm::mem_account_logic::ObjCharge>,
    /// CP-67: the channel a blocking IPC syscall (send, recv, grant,
    /// recv_grant) is using, moved here from the syscall frame for the call
    /// (`scheduler::with_channel_anchor`). A force-kill extracts the parked
    /// thread and the reaper frees its stack as raw bytes, so a frame-owned
    /// `Arc<Channel>` would leak (CP-30); owned here, `Thread::free` drops it
    /// (CP-36's pattern). Written only by the thread itself, and by `free`.
    pub(super) ipc_anchor: Option<Arc<crate::ipc::Channel>>,
    /// CP-67 for the pipe (L1a.1 step 2a): the pipe a blocking pipe syscall
    /// is using, moved here from the syscall frame for the call
    /// (`scheduler::with_pipe_anchor`), so a thread force-killed while parked
    /// in the pipe doesn't leak it. Dropped by `Thread::free`.
    #[cfg(feature = "pipe")]
    pub(super) pipe_anchor: Option<Arc<crate::ipc::pipe::PipeObject>>,
    /// OT-61 C1.2 (P3): this thread's user FS.base, set by `SYS_THREAD_SET_TLS`
    /// and loaded into `IA32_FS_BASE` whenever the thread is switched in
    /// (`scheduler::load_user_fs_base`). Every thread starts at 0, including a
    /// new `thread_create` child, which installs its own block from the
    /// TLS-free bootstrap. Ring 3 can still change the hardware FS.base behind
    /// the kernel (a `mov fs, <selector>` loads the descriptor's base, and
    /// `WRFSBASE` would once CR4.FSGSBASE is on), so the kernel never trusts the
    /// hardware value: every switch-in and every set writes this field to the
    /// MSR (CP-83). This field is authoritative; a user-written base is lost at
    /// the thread's next switch-in, which is why there is no save on switch-out.
    /// Written only by the thread itself, under its CPU's scheduler lock.
    #[cfg(feature = "browser-runtime")]
    pub(super) fs_base: u64,
}

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(0);

fn alloc_id() -> ThreadId {
    ThreadId(NEXT_THREAD_ID.fetch_add(1, Ordering::Relaxed))
}

impl Thread {
    fn alloc_from_slab(thread: Self) -> Box<Self> {
        without_interrupts(|| {
            let slab_box = THREAD_CACHE
                .get()
                .expect("slab not initialized")
                .alloc()
                .expect("THREAD_CACHE alloc failed");
            let raw = Box::into_raw(slab_box);
            unsafe {
                core::ptr::write(raw, thread);
                Box::from_raw(raw)
            }
        })
    }

    /// CP-66 discriminator: make this kernel-built thread a thread of `pid`,
    /// running under its address space (the scenario's stand-in callers).
    #[cfg(any(
        feature = "neo-test-admission-park-kill",
        feature = "neo-test-ipc-park-kill",
        feature = "neo-test-kill-at-exit",
        feature = "neo-test-pipe-core",
        feature = "neo-test-pipe-park-kill"
    ))]
    pub fn bind_process_for_test(&mut self, pid: ProcessId, pmap: Arc<Pmap>) {
        self.pid = pid;
        self.pmap = Some(pmap);
    }

    /// Install the finding-2 lifetime anchor (see the `process` field).
    #[cfg(feature = "obj-quota")]
    pub fn set_obj_charge(&mut self, charge: Option<crate::mm::mem_account_logic::ObjCharge>) {
        self.obj_charge = charge;
    }

    pub fn set_process_anchor(&mut self, process: Arc<crate::sched::process::Process>) {
        self.process = Some(ProcessAnchor::new(process));
    }

    pub fn free(mut thread: Box<Self>) {
        let anchor = without_interrupts(|| {
            // The slab stores raw Thread objects without running Drop on free.
            // Drop owned fields before returning the Thread storage.
            thread.stack = None;
            thread.pmap = None;
            // Finding-2 anchor: taken here, at reap — the point after which
            // this thread can no longer dereference the process — and
            // dropped after this IF-off block, so a last `Arc` it holds drops
            // in the reaper's own context (Janus #188 recheck 2, item 3).
            let anchor = thread.process.take();
            // CP-67: a thread reaped while parked in IPC (force-killed).
            thread.ipc_anchor = None;
            #[cfg(feature = "pipe")]
            {
                thread.pipe_anchor = None;
            }
            #[cfg(feature = "obj-quota")]
            {
                thread.obj_charge = None;
            }
            #[cfg(feature = "xsave-fpu")]
            {
                thread.fpu_state = None;
            }
            THREAD_CACHE
                .get()
                .expect("slab not initialized")
                .free(thread);
            anchor
        });
        drop(anchor);
    }

    /// B1c AAG J2: attribute a kernel thread to a user process, so the
    /// syscall-level bounds probe resolves that process's handle table the way
    /// a real syscall does (`current_pid`). Its reap then deregisters the
    /// process like a natural exit.
    #[cfg(any(
        feature = "neo-test-real-lm-generate",
        feature = "neo-test-inference-kill"
    ))]
    pub fn set_pid_for_test(&mut self, pid: ProcessId) {
        self.pid = pid;
    }

    /// B1c AAG J3: also give the thread its process's address space, so every
    /// switch-in loads the process's CR3 (a preemptible stand-in for a user
    /// thread making syscalls).
    #[cfg(feature = "neo-test-inference-kill")]
    pub fn attach_process_for_test(&mut self, pid: ProcessId, pmap: Arc<Pmap>) {
        self.pid = pid;
        self.pmap = Some(pmap);
    }

    /// Build a new thread with a freshly allocated 16 KiB stack and an initial
    /// frame that will `ret` into `entry` on first switch-in.
    pub fn new(name: &'static str, entry: extern "C" fn() -> !) -> Box<Self> {
        let stack = vec![0u128; STACK_WORDS].into_boxed_slice();
        let stack_base = stack.as_ptr() as u64;
        let stack_top = stack_base + STACK_SIZE as u64;

        // entry_slot is 16-aligned so post-`ret` (from start_trampoline) RSP
        // ≡ 8 mod 16 (SysV). exit_slot sits 8 above it; start_trampoline_slot
        // sits 8 below; six callee-saved slots sit below that.
        let entry_slot = align_down(stack_top - 16, 16);
        let exit_slot = entry_slot + 8;
        let start_trampoline_slot = entry_slot - 8;
        let initial_rsp = start_trampoline_slot - 6 * 8;
        debug_assert!(entry_slot & 0xF == 0, "entry slot must be 16-byte aligned");
        debug_assert!(
            initial_rsp >= stack_base,
            "stack too small for initial frame"
        );

        // SAFETY: addresses are inside the freshly allocated stack we own;
        // 8-byte aligned (subtracted multiples of 8 from a 16-aligned base);
        // no aliasing. Callee-saved slots left zero by `vec![0u128; ...]`.
        unsafe {
            (start_trampoline_slot as *mut u64).write(thread_start_trampoline as *const () as u64);
            (entry_slot as *mut u64).write(entry as *const () as u64);
            (exit_slot as *mut u64).write(thread_exit_trampoline as *const () as u64);
        }

        Self::alloc_from_slab(Self {
            id: alloc_id(),
            pid: KERNEL_PID,
            pmap: None,
            process: None,
            name,
            state: ThreadState::Ready,
            cpu_id: AtomicU32::new(crate::arch::x86_64::per_cpu::current_cpu_id()),
            stack: Some(stack),
            #[cfg(feature = "xsave-fpu")]
            fpu_state: Some(FpuState::new_initial()),
            rsp: initial_rsp,
            blocked_return: 0,
            preempt_defer_ticks: 0,
            #[cfg(feature = "inference-stack")]
            inference_lifecycle: false,
            admission_section: false,
            in_syscall: false,
            #[cfg(feature = "obj-quota")]
            obj_charge: None,
            #[cfg(feature = "browser-runtime")]
            fs_base: 0,
            ipc_anchor: None,
            #[cfg(feature = "pipe")]
            pipe_anchor: None,
        })
    }

    #[cfg(any(
        feature = "neo-test-q7b-cr3-cache-skip-count",
        feature = "neo-test-q7c-concurrent-munmap-smp",
        feature = "neo-test-q7c-process-exit-during-thread-run",
        feature = "neo-test-q7c-missing-shootdown-detection",
        feature = "neo-test-d5-phase-a-coverage",
        feature = "neo-test-q8b-exit-state-cas",
        feature = "neo-test-q8c-blocked-kill-walk",
        feature = "neo-test-q8c-blocked-kill-foreign-parked",
        feature = "neo-test-smp-concurrent-reap-after-terminate",
        feature = "neo-test-phase2-perf-proxy-baseline",
        feature = "neo-test-phase3-sched-wake-token-abandon",
        feature = "neo-test-phase3-virtio-blk-abandoned-io-no-leak",
        feature = "neo-test-token-park-teardown-churn",
        feature = "neo-test-hid-contention",
        feature = "neo-test-wait-any-core"
    ))]
    pub fn new_kernel_for_process_test(
        name: &'static str,
        pid: ProcessId,
        entry: extern "C" fn() -> !,
    ) -> Box<Self> {
        let mut thread = Self::new(name, entry);
        thread.pid = pid;
        thread.pmap = super::process::get(pid).and_then(|proc| proc.pmap.as_ref().cloned());
        thread
    }

    /// Construct a Thread that represents the currently executing context
    /// (the kernel's bootstrap main thread). It does NOT own a stack — the
    /// active stack is whatever Limine handed us at `_kernel_start` — and its
    /// `rsp` will be filled in by the first `context_switch` that switches
    /// AWAY from it.
    pub fn adopt_current(name: &'static str) -> Box<Self> {
        Self::alloc_from_slab(Self {
            id: alloc_id(),
            pid: KERNEL_PID,
            pmap: None,
            process: None,
            name,
            state: ThreadState::Running,
            cpu_id: AtomicU32::new(crate::arch::x86_64::per_cpu::current_cpu_id()),
            stack: None,
            // Never restored before the first switch-out saves the live state
            // over it; the init image just keeps one constructor.
            #[cfg(feature = "xsave-fpu")]
            fpu_state: Some(FpuState::new_initial()),
            rsp: 0, // overwritten by first switch-out via *prev_rsp_ptr
            blocked_return: 0,
            preempt_defer_ticks: 0,
            #[cfg(feature = "inference-stack")]
            inference_lifecycle: false,
            admission_section: false,
            in_syscall: false,
            #[cfg(feature = "obj-quota")]
            obj_charge: None,
            #[cfg(feature = "browser-runtime")]
            fs_base: 0,
            ipc_anchor: None,
            #[cfg(feature = "pipe")]
            pipe_anchor: None,
        })
    }

    /// Process this thread belongs to. Exposed for cross-module callers
    /// (e.g. `ipc::Channel::close_peer`) that need to compare an extracted
    /// parked thread's pid against an exiting process's pid.
    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    /// Returns the top-of-kernel-stack address, suitable for TSS.rsp0 and
    /// PER_CPU.kernel_rsp updates. Returns None for the bootstrap "main"
    /// thread (which owns no Box stack — uses Limine's stack). Callers
    /// MUST handle that case (Phase 1.4: the bootstrap thread is never
    /// the target of a syscall, so this returning None for main is safe).
    pub(super) fn kernel_stack_top(&self) -> Option<u64> {
        self.stack
            .as_ref()
            .map(|s| s.as_ptr() as u64 + (s.len() * 16) as u64)
    }

    #[cfg(feature = "xsave-fpu")]
    pub(super) fn fpu_state_ptr(&mut self) -> *mut u8 {
        self.fpu_state
            .as_mut()
            .expect("thread FPU state missing outside Thread::free")
            .as_mut_ptr()
    }

    /// Construct a new USER thread. Allocates a kernel stack (used during
    /// syscalls — the user has its OWN stack mapped via the Pmap, identified
    /// by `user_rsp`). The initial kernel-stack frame is shaped so that on
    /// first context_switch into this thread, `enter_userspace_trampoline`
    /// runs and immediately iretqs to ring 3 with (rip=`entry_rip`,
    /// rsp=`user_rsp`, cs=USER_CODE_SELECTOR, ss=USER_DATA_SELECTOR).
    ///
    /// Initial kernel-stack frame layout (low → high addresses):
    ///
    ///   [r15 = 0       ] <- initial RSP
    ///   [r14 = 0       ]
    ///   [r13 = 0       ]
    ///   [r12 = 0       ]
    ///   [rbp = 0       ]
    ///   [rbx = 0       ]
    ///   [enter_userspace_trampoline ] <- first ret pops this
    ///   [user RIP      ] <- iretq pops these 5 (low to high)
    ///   [user CS       ]
    ///   [user RFLAGS   ]
    ///   [user RSP      ]
    ///   [user SS       ]
    ///
    /// USER_CODE_SELECTOR = 0x2B (gdt slot 5, RPL 3); USER_DATA_SELECTOR =
    /// 0x23 (gdt slot 4, RPL 3). RFLAGS gets bit 9 (IF=1) + bit 1 (reserved,
    /// always 1) so the user runs with interrupts enabled.
    pub fn new_user(
        name: &'static str,
        pid: ProcessId,
        entry_rip: u64,
        user_rsp: u64,
        pmap: Arc<Pmap>,
    ) -> Box<Self> {
        let stack = vec![0u128; STACK_WORDS].into_boxed_slice();
        let stack_base = stack.as_ptr() as u64;
        let stack_top = stack_base + STACK_SIZE as u64;

        // Layout (high addr → low addr; stack grows down):
        //   user SS                    ← iretq frame top
        //   user RSP
        //   user RFLAGS
        //   user CS
        //   user RIP                   ← iretq pops from here
        //   enter_userspace_trampoline ← context_switch's ret pops to here
        //   rbx, rbp, r12-r15 (zero)   ← 6 callee-saved slots
        //                                initial kernel RSP = lowest of these
        //
        // 12 qwords = 96 bytes total. 8-byte alignment is sufficient for
        // long-mode iretq; place the SS slot at stack_top - 8.
        let ss_slot = stack_top - 8;
        let rsp_slot = ss_slot - 8;
        let rflags_slot = rsp_slot - 8;
        let cs_slot = rflags_slot - 8;
        let rip_slot = cs_slot - 8;
        let trampoline_slot = rip_slot - 8;
        let initial_rsp = trampoline_slot - 6 * 8;

        debug_assert!(
            initial_rsp >= stack_base,
            "user-thread kernel stack too small"
        );

        // SAFETY: addresses are inside our freshly-allocated kernel stack.
        // 8-byte aligned writes; no aliasing. Six callee-saved slots are
        // already zeroed by `vec![0u128; ...]`.
        unsafe {
            (trampoline_slot as *mut u64).write(enter_userspace_trampoline as *const () as u64);
            (rip_slot as *mut u64).write(entry_rip);
            (cs_slot as *mut u64).write(USER_CODE_SEL_RAW);
            (rflags_slot as *mut u64).write(USER_RFLAGS_RAW);
            (rsp_slot as *mut u64).write(user_rsp);
            (ss_slot as *mut u64).write(USER_DATA_SEL_RAW);
        }

        Self::alloc_from_slab(Self {
            id: alloc_id(),
            pid,
            pmap: Some(pmap),
            process: None,
            name,
            state: ThreadState::Ready,
            cpu_id: AtomicU32::new(crate::arch::x86_64::per_cpu::current_cpu_id()),
            stack: Some(stack),
            #[cfg(feature = "xsave-fpu")]
            fpu_state: Some(FpuState::new_initial()),
            rsp: initial_rsp,
            blocked_return: 0,
            preempt_defer_ticks: 0,
            #[cfg(feature = "inference-stack")]
            inference_lifecycle: false,
            admission_section: false,
            in_syscall: false,
            #[cfg(feature = "obj-quota")]
            obj_charge: None,
            #[cfg(feature = "browser-runtime")]
            fs_base: 0,
            ipc_anchor: None,
            #[cfg(feature = "pipe")]
            pipe_anchor: None,
        })
    }

    /// OT-61 C1.1: like [`Thread::new_user`], but the ring-3 entry receives
    /// `arg` in RDI (its first SysV argument) via
    /// `enter_userspace_arg_trampoline`, which pops one extra frame word and
    /// zeroes the remaining caller-saved registers. Used by
    /// `SYS_THREAD_CREATE`; the spawn path keeps the arg-less shape.
    ///
    /// Initial kernel-stack frame (low → high addresses):
    ///
    ///   [r15..rbx = 0 (6 slots)      ] <- initial RSP
    ///   [enter_userspace_arg_trampoline]
    ///   [arg                          ] <- trampoline pops into RDI
    ///   [user RIP | CS | RFLAGS | RSP | SS]  <- iretq frame
    #[cfg(feature = "browser-runtime")]
    pub fn new_user_with_arg(
        name: &'static str,
        pid: ProcessId,
        entry_rip: u64,
        user_rsp: u64,
        arg: u64,
        pmap: Arc<Pmap>,
    ) -> Box<Self> {
        let stack = vec![0u128; STACK_WORDS].into_boxed_slice();
        let stack_base = stack.as_ptr() as u64;
        let stack_top = stack_base + STACK_SIZE as u64;

        let ss_slot = stack_top - 8;
        let rsp_slot = ss_slot - 8;
        let rflags_slot = rsp_slot - 8;
        let cs_slot = rflags_slot - 8;
        let rip_slot = cs_slot - 8;
        let arg_slot = rip_slot - 8;
        let trampoline_slot = arg_slot - 8;
        let initial_rsp = trampoline_slot - 6 * 8;

        debug_assert!(
            initial_rsp >= stack_base,
            "user-thread kernel stack too small"
        );

        // SAFETY: addresses are inside our freshly-allocated kernel stack;
        // 8-byte aligned writes; no aliasing. Six callee-saved slots stay
        // zeroed from `vec![0u128; ...]`.
        unsafe {
            (trampoline_slot as *mut u64).write(enter_userspace_arg_trampoline as *const () as u64);
            (arg_slot as *mut u64).write(arg);
            (rip_slot as *mut u64).write(entry_rip);
            (cs_slot as *mut u64).write(USER_CODE_SEL_RAW);
            (rflags_slot as *mut u64).write(USER_RFLAGS_RAW);
            (rsp_slot as *mut u64).write(user_rsp);
            (ss_slot as *mut u64).write(USER_DATA_SEL_RAW);
        }

        Self::alloc_from_slab(Self {
            id: alloc_id(),
            pid,
            pmap: Some(pmap),
            process: None,
            name,
            state: ThreadState::Ready,
            cpu_id: AtomicU32::new(crate::arch::x86_64::per_cpu::current_cpu_id()),
            stack: Some(stack),
            #[cfg(feature = "xsave-fpu")]
            fpu_state: Some(FpuState::new_initial()),
            rsp: initial_rsp,
            blocked_return: 0,
            preempt_defer_ticks: 0,
            #[cfg(feature = "inference-stack")]
            inference_lifecycle: false,
            admission_section: false,
            in_syscall: false,
            #[cfg(feature = "obj-quota")]
            obj_charge: None,
            #[cfg(feature = "browser-runtime")]
            fs_base: 0,
            ipc_anchor: None,
            #[cfg(feature = "pipe")]
            pipe_anchor: None,
        })
    }
}

/// Compile-time guard: STACK_SIZE must be a multiple of 16 so `STACK_WORDS *
/// 16 == STACK_SIZE` exactly. If this ever fires, change STACK_SIZE.
const _: () = assert!(STACK_SIZE.is_multiple_of(16));

/// OT-93 (the PM's #204 bounce, item 1): a thread's anchor on its process,
/// counted in `Process::live_threads` from its install to its release (the
/// reap's `Thread::free`, or any other drop of the thread). A kill's
/// completion waits for that count to reach 0 (`process::finish_kill`).
///
/// Its `Arc` can be the process's last: a kill that saw the count reach 0
/// completes and releases its own on another CPU. So the drop takes the
/// `Arc` out, publishes the count, then releases it through
/// `release_last_process`, which hands a last one to the reaper when
/// interrupts are off or the reap is nested (Janus #188 recheck 2, item 3).
pub(super) struct ProcessAnchor(ManuallyDrop<Arc<crate::sched::process::Process>>);

impl ProcessAnchor {
    fn new(process: Arc<crate::sched::process::Process>) -> ProcessAnchor {
        process.anchor_thread();
        ProcessAnchor(ManuallyDrop::new(process))
    }
}

impl core::ops::Deref for ProcessAnchor {
    type Target = crate::sched::process::Process;

    fn deref(&self) -> &crate::sched::process::Process {
        &self.0
    }
}

impl Drop for ProcessAnchor {
    fn drop(&mut self) {
        // SAFETY: taken once, here; `self.0` is not used after.
        let process = unsafe { ManuallyDrop::take(&mut self.0) };
        process.unanchor_thread();
        crate::sched::process::release_last_process(process);
    }
}

/// `cap_graph_revoke` F1h: an anchor on `process` installed and dropped with
/// interrupts off, as `Thread::free`'s used to be.
#[cfg(feature = "neo-test-cap-graph-revoke")]
pub fn anchor_drop_for_test(process: Arc<crate::sched::process::Process>) {
    let anchor = ProcessAnchor::new(process);
    without_interrupts(|| drop(anchor));
}
