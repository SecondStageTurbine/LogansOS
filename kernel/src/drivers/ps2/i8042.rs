// SPDX-License-Identifier: MPL-2.0
//! The i8042 keyboard driver (`ps2-keyboard`).
//!
//! **Init** (boot, BSP, once; every wait bounded):
//! 1. Probe: a status read of `0xFF` means nothing decodes the ports. The
//!    FADT `IAPC_BOOT_ARCH` "8042 present" bit is logged, never trusted.
//! 2. Disable both ports, flush the output buffer, write a quiesced config
//!    (no IRQs, translation on), run the controller self-test (`0xAA → 0x55`)
//!    and rewrite the config (a self-test may reset it), then the port-1
//!    interface test (`0xAB → 0x00`).
//! 3. Enable port 1 and tell the keyboard to scan (`0xF4`, with the
//!    `0xFE` resend retried and the `0xFA` ACK awaited). The keyboard is never
//!    reset: some EC keyboards do not answer a reset.
//! 4. Route IRQ1 through the I/O APIC (MADT override honoured) to
//!    [`VECTOR`] on the BSP, then turn on the config's IRQ1 bit.
//!
//! **Runtime.** The ISR only moves bytes: while the output buffer is full it
//! reads status + data, keeps keyboard bytes, and pushes them into a
//! lock-free single-producer ring. It takes no lock at all (I-14(a): no
//! governance lock, and no other). [`pump_routed_events`] (syscall context,
//! next to the USB keyboard's pump) decodes the ring with the set-1
//! [`Decoder`] into the same `InputEventV1` key events the USB HID boot
//! keyboard produces, through the same SAK filter and focus stage.
//!
//! **Polled fallback.** The pump also reads the controller itself when the
//! output buffer is full, so a dead IRQ route (firmware, a missing override,
//! an unrouted GSI) still delivers keys, and is visible as `polled` > 0 with
//! `irqs` = 0 in the status line rather than as a silent keyboard. The ISR
//! and the pump never read the port at the same time: each takes
//! `PORT_OWNER` by compare-exchange and the loser skips (the ISR never
//! waits).

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

use x86_64::instructions::port::Port;

use super::i8042_logic::{self as logic, ByteSource, Decoder};
use crate::sync::IrqSafeMutex;

/// The IDT vector IRQ1 is routed to.
pub const VECTOR: u8 = 0xF5;

/// Status/command-port waits, in port reads (each is about a microsecond on
/// hardware, so this bounds a wait near 100 ms).
const WAIT_SPINS: u32 = 100_000;
/// Bytes flushed from the output buffer before init, at most.
const FLUSH_MAX: u32 = 32;
/// Resends of a keyboard command answered `0xFE`.
const RESEND_MAX: u32 = 3;
/// Bytes one ISR or pump pass reads, at most. IRQ1 is edge-triggered and the
/// line stays asserted while the output buffer is full, so a pass must leave
/// it EMPTY: a byte left behind raises no new edge, and the keyboard goes
/// silent (seen under QEMU, whose PS/2 queue holds 16 bytes, when this was
/// 16). The cap only bounds a stuck controller (I-14(b)); hitting it is
/// counted, and the pump's polled fallback takes what was left.
const DRAIN_MAX: u32 = 256;

const RING_LEN: usize = 256;
static RING: [AtomicU8; RING_LEN] = [const { AtomicU8::new(0) }; RING_LEN];
/// Producer index (written only under `PORT_OWNER`).
static HEAD: AtomicU32 = AtomicU32::new(0);
/// Consumer index (written only under the pump's `STATE` lock).
static TAIL: AtomicU32 = AtomicU32::new(0);

/// Taken (never waited for) by whoever reads the data port.
static PORT_OWNER: AtomicBool = AtomicBool::new(false);
/// The driver is initialised and its interrupt may fire.
static READY: AtomicBool = AtomicBool::new(false);

static IRQS: AtomicU64 = AtomicU64::new(0);
static POLLED: AtomicU64 = AtomicU64::new(0);
/// Keyboard bytes the ISR (not the polled fallback) moved into the ring.
static IRQ_BYTES: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static DISCARDED: AtomicU64 = AtomicU64::new(0);
/// Passes that stopped at `DRAIN_MAX` with the buffer still full.
static CAPPED: AtomicU64 = AtomicU64::new(0);
static KEYS: AtomicU64 = AtomicU64::new(0);

/// Decoder state, owned by whoever pumps.
struct State {
    decoder: Decoder,
    /// HID boot-report modifier byte (bit = usage − 0xE0).
    modifiers: u8,
    #[cfg(feature = "atrium-input-routing")]
    stage: crate::atrium::focus::Stage,
    #[cfg(feature = "atrium-input-routing")]
    sak: crate::atrium::sak_logic::SakFilter,
    #[cfg(feature = "key-autorepeat")]
    repeat: crate::atrium::key_repeat_logic::KeyRepeat,
}

static STATE: IrqSafeMutex<Option<State>> = IrqSafeMutex::new(None);

/// `HEAD` when the first keyboard route came up ([`note_route_activated`]);
/// [`CUT_UNSET`] until then, [`CUT_DONE`] once the pump has applied it.
#[cfg(feature = "atrium-input-routing")]
static BOOT_CUT: AtomicU64 = AtomicU64::new(CUT_UNSET);
#[cfg(feature = "atrium-input-routing")]
const CUT_UNSET: u64 = u64::MAX;
#[cfg(feature = "atrium-input-routing")]
const CUT_DONE: u64 = u64::MAX - 1;
/// Bytes received before the first route that the pump skipped.
static BOOT_SKIPPED: AtomicU64 = AtomicU64::new(0);

/// A keyboard route became active. The first time, everything the ring holds
/// from before is marked for the pump to skip (`logic::tail_after_cut`).
/// Lock-free: called under the focus delivery lock.
#[cfg(feature = "atrium-input-routing")]
pub fn note_route_activated() {
    // Kill-switch: the boot backlog is delivered, as before.
    if cfg!(feature = "ps2-boot-keys-legacy") {
        return;
    }
    let head = u64::from(HEAD.load(Ordering::Acquire));
    let _ = BOOT_CUT.compare_exchange(CUT_UNSET, head, Ordering::AcqRel, Ordering::Relaxed);
}

/// Under the pump lock: skip the bytes received before the first route and
/// start the decoder, modifiers, repeat, SAK filter and stage afresh, so no
/// boot-time key reaches the desktop and no key is left held by a lost break.
/// Returns the bytes skipped when it applies the cut (once).
#[cfg(feature = "atrium-input-routing")]
fn apply_boot_cut(state: &mut State) -> Option<u32> {
    let cut = BOOT_CUT.load(Ordering::Acquire);
    if cut == CUT_UNSET || cut == CUT_DONE {
        return None;
    }
    let tail = TAIL.load(Ordering::Relaxed);
    let head = HEAD.load(Ordering::Acquire);
    let new_tail = logic::tail_after_cut(tail, head, cut as u32);
    BOOT_SKIPPED.fetch_add(u64::from(new_tail.wrapping_sub(tail)), Ordering::Relaxed);
    TAIL.store(new_tail, Ordering::Release);
    state.decoder = Decoder::new();
    state.modifiers = 0;
    state.sak = crate::atrium::sak_logic::SakFilter::new();
    state.stage =
        crate::atrium::focus::Stage::with_capacity(crate::atrium::focus::INPUT_STAGE_CAPACITY);
    #[cfg(feature = "key-autorepeat")]
    {
        state.repeat = crate::atrium::key_repeat_logic::KeyRepeat::new();
    }
    BOOT_CUT.store(CUT_DONE, Ordering::Release);
    Some(new_tail.wrapping_sub(tail))
}

fn inb(port: u16) -> u8 {
    // SAFETY: the i8042's ports; reading them has no memory effect.
    unsafe { Port::<u8>::new(port).read() }
}

fn outb(port: u16, value: u8) {
    // SAFETY: as above.
    unsafe { Port::<u8>::new(port).write(value) }
}

fn wait_input_empty() -> bool {
    (0..WAIT_SPINS).any(|_| inb(logic::STATUS_PORT) & logic::STATUS_IBF == 0)
}

fn wait_output_full() -> bool {
    (0..WAIT_SPINS).any(|_| inb(logic::STATUS_PORT) & logic::STATUS_OBF != 0)
}

fn command(cmd: u8) -> bool {
    if !wait_input_empty() {
        return false;
    }
    outb(logic::STATUS_PORT, cmd);
    true
}

fn read_data() -> Option<u8> {
    wait_output_full().then(|| inb(logic::DATA_PORT))
}

fn command_reply(cmd: u8) -> Option<u8> {
    if !command(cmd) {
        return None;
    }
    read_data()
}

fn write_config(config: u8) -> bool {
    if !command(logic::CMD_WRITE_CONFIG) || !wait_input_empty() {
        return false;
    }
    outb(logic::DATA_PORT, config);
    true
}

/// Send a byte to the keyboard and await its ACK, resending on `0xFE`.
fn keyboard_command(byte: u8) -> KbdReply {
    for _ in 0..=RESEND_MAX {
        if !wait_input_empty() {
            return KbdReply::Timeout;
        }
        outb(logic::DATA_PORT, byte);
        match read_data() {
            Some(logic::KBD_ACK) => return KbdReply::Ack,
            Some(logic::KBD_RESEND) => continue,
            Some(other) => return KbdReply::Other(other),
            None => return KbdReply::Timeout,
        }
    }
    KbdReply::Resend
}

#[derive(Clone, Copy, Debug)]
enum KbdReply {
    Ack,
    Resend,
    Timeout,
    Other(u8),
}

/// Read what is waiting (at most [`DRAIN_MAX`] bytes) into the ring. The
/// caller owns `PORT_OWNER`. Returns the bytes read.
fn drain_port() -> u32 {
    let mut n = 0;
    loop {
        let status = inb(logic::STATUS_PORT);
        if status & logic::STATUS_OBF == 0 {
            break;
        }
        if n == DRAIN_MAX {
            CAPPED.fetch_add(1, Ordering::Relaxed);
            break;
        }
        let byte = inb(logic::DATA_PORT);
        n += 1;
        if logic::classify(status) != ByteSource::Keyboard {
            DISCARDED.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        let head = HEAD.load(Ordering::Relaxed);
        if head.wrapping_sub(TAIL.load(Ordering::Acquire)) as usize >= RING_LEN {
            DROPPED.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        RING[head as usize % RING_LEN].store(byte, Ordering::Relaxed);
        HEAD.store(head.wrapping_add(1), Ordering::Release);
        BYTES.fetch_add(1, Ordering::Relaxed);
    }
    n
}

fn try_drain_port() -> u32 {
    if PORT_OWNER
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return 0;
    }
    let n = drain_port();
    PORT_OWNER.store(false, Ordering::Release);
    n
}

/// IRQ1 body (IF=0, interrupt gate). Lock-free: it reads the ports and
/// pushes into the ring, nothing else. The caller sends the EOI.
pub fn handle_interrupt() {
    IRQS.fetch_add(1, Ordering::Relaxed);
    if READY.load(Ordering::Acquire) {
        let before = BYTES.load(Ordering::Relaxed);
        try_drain_port();
        IRQ_BYTES.fetch_add(BYTES.load(Ordering::Relaxed) - before, Ordering::Relaxed);
    }
}

/// The boot status line's fields.
struct Probe {
    boot_arch: Option<u16>,
    present: bool,
    selftest: &'static str,
    port1: &'static str,
    kbd: KbdReply,
    irq1: Irq1,
}

#[derive(Clone, Copy)]
enum Irq1 {
    NotTried,
    Routed(crate::arch::x86_64::ioapic::Routed),
    Failed(crate::arch::x86_64::ioapic::RouteError),
    NoMadt,
}

fn log_probe(p: &Probe) {
    let present = if p.present { "yes" } else { "no" };
    let boot_arch = match p.boot_arch {
        Some(flags) if flags & 0b10 != 0 => "8042",
        Some(_) => "no-8042",
        None => "unknown",
    };
    match p.irq1 {
        Irq1::Routed(r) => {
            crate::serial_println!(
                "i8042: present={} selftest={} port1={} kbd={:?} irq1=gsi{}/pin{}{}{} vector={:#x} fadt_boot_arch={} keys={}",
                present,
                p.selftest,
                p.port1,
                p.kbd,
                r.gsi,
                r.pin,
                if r.active_low { "/low" } else { "" },
                if r.level { "/level" } else { "" },
                VECTOR,
                boot_arch,
                KEYS.load(Ordering::Relaxed)
            );
        }
        other => {
            crate::serial_println!(
                "i8042: present={} selftest={} port1={} kbd={:?} irq1={} fadt_boot_arch={} keys={}",
                present,
                p.selftest,
                p.port1,
                p.kbd,
                Irq1Text(other),
                boot_arch,
                KEYS.load(Ordering::Relaxed)
            );
        }
    }
    crate::log::flush();
}

/// `irq1=` for an unrouted IRQ1; the driver then runs on the polled
/// fallback alone.
struct Irq1Text(Irq1);

impl core::fmt::Display for Irq1Text {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::arch::x86_64::ioapic::RouteError;
        match self.0 {
            Irq1::NotTried => f.write_str("not-tried"),
            Irq1::NoMadt => f.write_str("no-madt(polled)"),
            Irq1::Failed(RouteError::NoIoApic) => f.write_str("no-ioapic(polled)"),
            Irq1::Failed(RouteError::NoPin(gsi)) => write!(f, "no-pin-gsi{}(polled)", gsi),
            Irq1::Failed(RouteError::DestinationTooWide(id)) => {
                write!(f, "dest-apic-id-{}-too-wide(polled)", id)
            }
            Irq1::Routed(r) => write!(f, "gsi{}", r.gsi),
        }
    }
}

/// Probe and bring up the controller and keyboard. Boot-time, BSP, after the
/// USB stack has taken its controllers (firmware's USB legacy emulation of an
/// 8042 ends at the xHCI handoff, so a probe before it can find a controller
/// that then disappears). Returns whether the keyboard is live.
pub fn init(rsdp_phys: u64) -> bool {
    let mut probe = Probe {
        boot_arch: crate::acpi::fadt_boot_arch(rsdp_phys),
        present: false,
        selftest: "not-run",
        port1: "not-run",
        kbd: KbdReply::Timeout,
        irq1: Irq1::NotTried,
    };
    let live = bring_up(&mut probe, rsdp_phys);
    log_probe(&probe);
    live
}

fn bring_up(p: &mut Probe, rsdp_phys: u64) -> bool {
    // Nothing decodes the ports: the bus floats high.
    if inb(logic::STATUS_PORT) == 0xFF {
        return false;
    }
    if !command(logic::CMD_DISABLE_PORT1) || !command(logic::CMD_DISABLE_PORT2) {
        return false;
    }
    p.present = true;
    for _ in 0..FLUSH_MAX {
        if inb(logic::STATUS_PORT) & logic::STATUS_OBF == 0 {
            break;
        }
        let _ = inb(logic::DATA_PORT);
    }
    let Some(read) = command_reply(logic::CMD_READ_CONFIG) else {
        p.selftest = "no-config";
        return false;
    };
    let config = logic::config_quiesced(read);
    #[cfg(feature = "neo-test-ps2-keyboard-control")]
    let config = config & !logic::CONFIG_TRANSLATE;
    if !write_config(config) {
        p.selftest = "config-write-timeout";
        return false;
    }
    p.selftest = match command_reply(logic::CMD_SELF_TEST) {
        Some(logic::SELF_TEST_OK) => "ok",
        // An EC that does not implement the test: carry on, the port test
        // and the keyboard's ACK still have to pass.
        None => "timeout",
        Some(_) => "fail",
    };
    if p.selftest == "fail" || !write_config(config) {
        return false;
    }
    p.port1 = match command_reply(logic::CMD_TEST_PORT1) {
        Some(logic::PORT_TEST_OK) => "ok",
        None => "timeout",
        Some(_) => "fail",
    };
    if p.port1 == "fail" || !command(logic::CMD_ENABLE_PORT1) {
        return false;
    }
    p.kbd = keyboard_command(logic::KBD_ENABLE_SCANNING);

    crate::arch::x86_64::without_interrupts(|| {
        *STATE.lock() = Some(State {
            decoder: Decoder::new(),
            modifiers: 0,
            #[cfg(feature = "atrium-input-routing")]
            stage: crate::atrium::focus::Stage::with_capacity(
                crate::atrium::focus::INPUT_STAGE_CAPACITY,
            ),
            #[cfg(feature = "atrium-input-routing")]
            sak: crate::atrium::sak_logic::SakFilter::new(),
            #[cfg(feature = "key-autorepeat")]
            repeat: crate::atrium::key_repeat_logic::KeyRepeat::new(),
        });
    });
    p.irq1 = match crate::acpi::madt(rsdp_phys) {
        Some(madt) => match crate::arch::x86_64::ioapic::route_isa_irq(&madt, 1, VECTOR) {
            Ok(r) => Irq1::Routed(r),
            Err(e) => Irq1::Failed(e),
        },
        None => Irq1::NoMadt,
    };
    READY.store(true, Ordering::Release);
    if !write_config(logic::config_running(config)) {
        return false;
    }
    // An edge-triggered IRQ1 raised while a byte already sits in the buffer
    // never comes: take what is waiting now.
    try_drain_port();
    true
}

/// Decode what the ring holds into `emit`, under the pump lock.
fn decode_ring(
    state: &mut State,
    mut emit: impl FnMut(&mut State, crate::atrium::input_logic::InputEventV1),
) {
    loop {
        let tail = TAIL.load(Ordering::Relaxed);
        if tail == HEAD.load(Ordering::Acquire) {
            break;
        }
        let byte = RING[tail as usize % RING_LEN].load(Ordering::Relaxed);
        TAIL.store(tail.wrapping_add(1), Ordering::Release);
        let Some(event) = state.decoder.feed(byte) else {
            continue;
        };
        if (0xE0..=0xE7).contains(&event.usage) {
            // Like the HID boot report: a modifier is state on the next key
            // event, never an event of its own.
            let bit = 1u8 << (event.usage - 0xE0);
            if event.pressed {
                state.modifiers |= bit;
            } else {
                state.modifiers &= !bit;
            }
            #[cfg(feature = "key-autorepeat")]
            state.repeat.on_modifiers(state.modifiers);
            continue;
        }
        KEYS.fetch_add(1, Ordering::Relaxed);
        let modifiers = state.modifiers;
        let locks = crate::drivers::key_locks::on_key(event.usage, event.pressed);
        emit(
            state,
            crate::atrium::input_logic::InputEventV1::key_locked(
                event.usage,
                modifiers,
                locks,
                event.pressed,
            ),
        );
    }
    #[cfg(feature = "key-autorepeat")]
    crate::atrium::key_repeat::note_typematic(state.decoder.take_typematic() as u64);
}

/// Take what the controller holds if the interrupt has not (the polled
/// fallback), counted.
fn poll_port() {
    if READY.load(Ordering::Acquire) && inb(logic::STATUS_PORT) & logic::STATUS_OBF != 0 {
        let n = try_drain_port();
        POLLED.fetch_add(n as u64, Ordering::Relaxed);
    }
}

/// Decode and deliver PS/2 key events to the focused session: the same
/// `InputEventV1` key events, SAK filter and focus stage as
/// `hid_keyboard::pump_routed_events`. Syscall context; never waits on the
/// controller.
#[cfg(feature = "atrium-input-routing")]
pub fn pump_routed_events() {
    if !READY.load(Ordering::Acquire) {
        return;
    }
    let keys_before = KEYS.load(Ordering::Relaxed);
    let skipped;
    {
        let mut guard = STATE.lock();
        let Some(state) = guard.as_mut() else {
            return;
        };
        poll_port();
        skipped = apply_boot_cut(state);
        let routing = crate::atrium::input_routing_enabled();
        #[cfg(feature = "key-autorepeat")]
        let now_ns = crate::atrium::key_repeat::now_ns();
        decode_ring(state, |state, event| {
            if !routing {
                return;
            }
            if cfg!(feature = "atrium-sak") {
                use crate::atrium::sak_logic::SakVerdict;
                match state.sak.filter(event) {
                    SakVerdict::Chord(key) => {
                        crate::atrium::focus::latch_sak(key);
                        return;
                    }
                    SakVerdict::Swallow => return,
                    SakVerdict::Pass => {}
                }
            }
            #[cfg(feature = "key-autorepeat")]
            crate::atrium::key_repeat::on_event(&mut state.repeat, event, now_ns);
            if cfg!(feature = "atrium-input-stage-legacy") {
                crate::atrium::focus::route_event(event);
            } else {
                crate::atrium::focus::stage_event(&mut state.stage, event);
            }
        });
        // Key auto-repeat: the repeat due now, if the held key's press was
        // delivered. It carries that press's modifiers, so it needs no SAK
        // filter (a modifier change has already ended it).
        #[cfg(feature = "key-autorepeat")]
        if routing
            && let Some((event, epoch)) = crate::atrium::key_repeat::poll(&mut state.repeat, now_ns)
        {
            crate::atrium::key_repeat::deliver(&mut state.stage, event, epoch);
        }
        crate::atrium::focus::flush_staged(&mut state.stage);
    }
    // Once, with the pump lock released. A syscall may be at IF=0 here, so
    // the line is staged, never flushed (CP-71).
    if let Some(n) = skipped {
        let stage_only = crate::log::enter_stage_only();
        crate::serial_println!("i8042: dropped {} pre-desktop bytes", n);
        crate::log::leave_stage_only(stage_only);
    }
    note_progress(keys_before);
}

/// Test hook: decode into `InputEventV1`s without routing them, and without
/// the polled fallback, so every byte decoded came through IRQ1.
#[cfg(feature = "neo-test-ps2-keyboard")]
pub fn drain_events(out: &mut alloc::vec::Vec<crate::atrium::input_logic::InputEventV1>) {
    let keys_before = KEYS.load(Ordering::Relaxed);
    {
        let mut guard = STATE.lock();
        let Some(state) = guard.as_mut() else {
            return;
        };
        decode_ring(state, |_, event| out.push(event));
    }
    note_progress(keys_before);
}

/// Test hook for `ps2_key_repeat`: like `drain_events`, plus the repeats the
/// routed pump would generate. The clock is read once per call, given to the
/// repeat state machine, and stamped on every event returned (in ns), so the
/// scenario's timing checks see exactly the times the machine used.
#[cfg(feature = "neo-test-ps2-key-repeat")]
pub fn drain_events_stamped(
    out: &mut alloc::vec::Vec<(crate::atrium::input_logic::InputEventV1, u64)>,
) {
    let keys_before = KEYS.load(Ordering::Relaxed);
    let now_ns = crate::atrium::key_repeat::now_ns();
    {
        let mut guard = STATE.lock();
        let Some(state) = guard.as_mut() else {
            return;
        };
        decode_ring(state, |state, event| {
            crate::atrium::key_repeat::on_event(&mut state.repeat, event, now_ns);
            out.push((event, now_ns));
        });
        if let Some((event, _epoch)) = crate::atrium::key_repeat::poll(&mut state.repeat, now_ns) {
            out.push((event, now_ns));
        }
    }
    note_progress(keys_before);
}

/// The status line again at the 1st, 2nd, 4th, 8th … key event, so the
/// on-screen boot log shows the keyboard working (or `irqs=0 polled>0`, the
/// IRQ route dead) without one line per key.
fn note_progress(keys_before: u64) {
    let keys = KEYS.load(Ordering::Relaxed);
    // `keys` reached a new power of two since `keys_before`.
    let crossed = keys > keys_before && (keys_before == 0 || keys.ilog2() > keys_before.ilog2());
    if crossed {
        crate::serial_println!("{}", Counters::now());
        #[cfg(feature = "key-autorepeat")]
        {
            let (repeats, typematic) = crate::atrium::key_repeat::counters();
            crate::serial_println!("key-repeat: rep={} typematic={}", repeats, typematic);
        }
    }
}

/// Test diagnostic: the controller's status byte (a read has no side effect).
#[cfg(feature = "neo-test-ps2-keyboard")]
pub fn status_for_test() -> u8 {
    inb(logic::STATUS_PORT)
}

/// The runtime counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counters {
    pub keys: u64,
    pub irqs: u64,
    pub polled: u64,
    pub irq_bytes: u64,
    pub bytes: u64,
    pub dropped: u64,
    pub discarded: u64,
    pub capped: u64,
    /// Bytes typed before the first keyboard route, skipped by the pump.
    pub boot_skipped: u64,
}

impl Counters {
    pub fn now() -> Self {
        Self {
            keys: KEYS.load(Ordering::Relaxed),
            irqs: IRQS.load(Ordering::Relaxed),
            polled: POLLED.load(Ordering::Relaxed),
            irq_bytes: IRQ_BYTES.load(Ordering::Relaxed),
            bytes: BYTES.load(Ordering::Relaxed),
            dropped: DROPPED.load(Ordering::Relaxed),
            discarded: DISCARDED.load(Ordering::Relaxed),
            capped: CAPPED.load(Ordering::Relaxed),
            boot_skipped: BOOT_SKIPPED.load(Ordering::Relaxed),
        }
    }
}

impl core::fmt::Display for Counters {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "i8042: keys={} irqs={} irq_bytes={} polled={} bytes={} dropped={} discarded={} capped={} boot_skipped={}",
            self.keys,
            self.irqs,
            self.irq_bytes,
            self.polled,
            self.bytes,
            self.dropped,
            self.discarded,
            self.capped,
            self.boot_skipped
        )
    }
}
