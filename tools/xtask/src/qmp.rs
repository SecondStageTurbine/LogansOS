// SPDX-License-Identifier: MPL-2.0
//! OT-90 P2: QEMU's own report of why it stopped, read over QMP (THINK §8.1).
//!
//! Under `-no-reboot` a reset, a triple fault and S5 all end QEMU with exit
//! status 0, so a power scenario can't pass on its exit code. QMP's `SHUTDOWN`
//! event says why: `reason: guest-shutdown` for ACPI S5, `guest-reset` for a
//! reset (with `-no-reboot`). Without `-no-reboot` a reset is a `RESET` event
//! (`guest-reset`) and the guest boots again in the same process.
//!
//! **No event is missed.** xtask listens first; QEMU starts paused (`-S`) and
//! connects to the listener as a QMP client (`-qmp tcp:…`). The recorder
//! negotiates capabilities, then sends `cont`, so the guest runs only once
//! events are being delivered. QEMU closes the socket when it exits.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One QMP event: its name and, for SHUTDOWN and RESET, its `reason`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QmpEvent {
    pub event: String,
    pub reason: Option<String>,
}

/// The value of the first `"key": "value"` string member in a QMP line. QMP
/// writes one JSON object per line; the members xtask needs are plain strings
/// with no escapes (event names and reason enums), so a scan is enough.
fn string_member(line: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let at = line.find(&pat)? + pat.len();
    let rest = line[at..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// Parse one line from QEMU. `None` for anything that isn't an event (the
/// greeting, command returns).
pub fn parse_event(line: &str) -> Option<QmpEvent> {
    let event = string_member(line, "event")?;
    Some(QmpEvent {
        reason: string_member(line, "reason"),
        event,
    })
}

/// A running recorder: the QMP argument QEMU needs, and the thread that
/// collects events until QEMU closes the connection.
pub struct QmpRecorder {
    pub port: u16,
    thread: JoinHandle<Result<Vec<QmpEvent>, String>>,
}

/// How long the recorder waits for QEMU to connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

impl QmpRecorder {
    /// Listen on a free loopback port and start the recorder. Every line
    /// QEMU sends is appended to `log` as it arrives.
    pub fn start(log: &Path) -> Result<QmpRecorder, String> {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|e| format!("qmp: bind loopback: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("qmp: local addr: {e}"))?
            .port();
        let log = log.to_path_buf();
        let thread = std::thread::spawn(move || record(listener, &log));
        Ok(QmpRecorder { port, thread })
    }

    /// The QEMU arguments: start paused, and connect to the recorder.
    pub fn qemu_args(&self) -> [String; 3] {
        [
            "-S".to_string(),
            "-qmp".to_string(),
            format!("tcp:127.0.0.1:{}", self.port),
        ]
    }

    /// Wait for QEMU to close the connection and return every event, in order.
    pub fn finish(self) -> Result<Vec<QmpEvent>, String> {
        self.thread
            .join()
            .map_err(|_| "qmp: recorder thread panicked".to_string())?
    }
}

fn accept(listener: &TcpListener) -> Result<TcpStream, String> {
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("qmp: nonblocking: {e}"))?;
    let start = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|e| format!("qmp: blocking stream: {e}"))?;
                return Ok(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() > CONNECT_TIMEOUT {
                    return Err(format!(
                        "qmp: QEMU did not connect within {}s",
                        CONNECT_TIMEOUT.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(format!("qmp: accept: {e}")),
        }
    }
}

fn record(listener: TcpListener, log: &PathBuf) -> Result<Vec<QmpEvent>, String> {
    let stream = accept(&listener)?;
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("qmp: clone stream: {e}"))?;
    let mut lines = BufReader::new(stream).lines();
    let mut out =
        std::fs::File::create(log).map_err(|e| format!("qmp: create {}: {e}", log.display()))?;
    let mut events = Vec::new();
    let mut next = |out: &mut std::fs::File| -> Result<Option<String>, String> {
        match lines.next() {
            Some(Ok(l)) => {
                let _ = writeln!(out, "{l}");
                Ok(Some(l))
            }
            Some(Err(e)) => Err(format!("qmp: read: {e}")),
            None => Ok(None),
        }
    };
    // The greeting, then capabilities negotiation, then `cont`.
    let greeting = next(&mut out)?.ok_or("qmp: closed before the greeting")?;
    if !greeting.contains("\"QMP\"") {
        return Err(format!("qmp: unexpected greeting {greeting}"));
    }
    for cmd in ["qmp_capabilities", "cont"] {
        writeln!(writer, "{{\"execute\": \"{cmd}\"}}")
            .map_err(|e| format!("qmp: send {cmd}: {e}"))?;
        loop {
            let line = next(&mut out)?.ok_or_else(|| format!("qmp: closed awaiting {cmd}"))?;
            if let Some(ev) = parse_event(&line) {
                events.push(ev);
            } else if line.contains("\"return\"") {
                break;
            } else if line.contains("\"error\"") {
                return Err(format!("qmp: {cmd} failed: {line}"));
            }
        }
    }
    while let Some(line) = next(&mut out)? {
        if let Some(ev) = parse_event(&line) {
            events.push(ev);
        }
    }
    Ok(events)
}

/// What an OT-90 power scenario must show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerExpect {
    /// One boot that enters S5.
    PowerOff,
    /// Boot 1 restarts; boot 2 enters S5. With `fallback`, the FADT reset
    /// register step was a no-op, so the keyboard controller must have reset.
    RestartThenPowerOff { fallback: bool },
}

fn count(events: &[QmpEvent], event: &str, reason: Option<&str>) -> usize {
    events
        .iter()
        .filter(|e| e.event == event && (reason.is_none() || e.reason.as_deref() == reason))
        .count()
}

/// The index of `needle` in `serial` at or after `from`, as an error naming
/// what was missing otherwise.
fn find_after(serial: &str, from: usize, needle: &str, what: &str) -> Result<usize, String> {
    serial[from..]
        .find(needle)
        .map(|i| from + i)
        .ok_or_else(|| format!("serial lacks `{needle}` ({what})"))
}

/// The kernel's test-only neutral reset step waits this long
/// (`NEUTRAL_STEP_WAIT_US` in `kernel/src/acpi/power.rs`): more than one
/// 24-bit PM timer wrap (4.69 s). Its serial marker must carry it.
pub const NEUTRAL_STEP_WAIT_US: u64 = 6_000_000;

/// Judge an OT-90 power run from its QMP events and serial log.
pub fn judge(expect: PowerExpect, events: &[QmpEvent], serial: &str) -> Result<(), String> {
    let shutdowns = count(events, "SHUTDOWN", None);
    let guest_shutdowns = count(events, "SHUTDOWN", Some("guest-shutdown"));
    if shutdowns != 1 || guest_shutdowns != 1 {
        return Err(format!(
            "QMP: want exactly one SHUTDOWN, reason guest-shutdown (S5); got {events:?}"
        ));
    }
    let resets = count(events, "RESET", None);
    let guest_resets = count(events, "RESET", Some("guest-reset"));
    let last = events.iter().rposition(|e| e.event == "SHUTDOWN");
    // Each kernel boot begins the scenario once (`run_selected_scenario`'s
    // BEGIN frame). Counting them catches a boot that crashed and rebooted
    // (the AAG's F4: a triple fault is also a guest-reset).
    let boots = serial.matches("NEO_TEST/1 event=BEGIN ").count();
    let want_boots = match expect {
        PowerExpect::PowerOff => 1,
        PowerExpect::RestartThenPowerOff { .. } => 2,
    };
    if boots != want_boots {
        return Err(format!(
            "serial: {boots} kernel boot(s) (BEGIN frames), want {want_boots}"
        ));
    }
    match expect {
        PowerExpect::PowerOff => {
            if resets != 0 {
                return Err(format!("QMP: a RESET in a power-off run: {events:?}"));
            }
            let at = find_after(
                serial,
                0,
                "stage=power_off_requested",
                "the scenario's request",
            )?;
            find_after(serial, at, "power: S5 PM1a=", "the PM1 writes")?;
        }
        PowerExpect::RestartThenPowerOff { fallback } => {
            // SeaBIOS answers a warm reset with a hard reboot of its own
            // ("Attempting a hard reboot"), so one guest reset is two RESET
            // events under SeaBIOS. So: one or two, every one a guest reset,
            // all before the SHUTDOWN; a third is another reset (a crash).
            let last_reset = events.iter().rposition(|e| e.event == "RESET");
            if !(1..=2).contains(&resets) || guest_resets != resets || last_reset > last {
                return Err(format!(
                    "QMP: want one or two RESET (guest-reset; SeaBIOS doubles one) before the \
                     SHUTDOWN; got {events:?}"
                ));
            }
            for marker in ["stage=boot1 action=restart", "stage=boot2 action=power_off"] {
                let n = serial.matches(marker).count();
                if n != 1 {
                    return Err(format!(
                        "serial: `{marker}` {n} times, want once (a boot loop?)"
                    ));
                }
            }
            let b1 = find_after(serial, 0, "stage=boot1 action=restart", "boot 1")?;
            let step1 = if fallback {
                let neutral = find_after(
                    serial,
                    b1,
                    "reset step=1 fadt_register neutral",
                    "step 1 made a no-op",
                )?;
                // The neutral step's 6 s wait (past one 24-bit PM timer wrap)
                // must have ended for the chain to go on (the AAG's F5), and
                // with its planned duration: a shortened wait no longer crosses
                // a wrap, so it would remove the discriminator while the run
                // stayed green (#154 F5, Jarvis).
                let needle = format!("reset step=1 wait ended us={NEUTRAL_STEP_WAIT_US}");
                let at = find_after(
                    serial,
                    neutral,
                    &needle,
                    "step 1's wait ended, with its planned duration",
                )?;
                // The whole number: `us=60000000` is not the planned wait.
                if serial[at + needle.len()..].starts_with(|c: char| c.is_ascii_digit()) {
                    return Err(format!("serial: step 1's wait is not `{needle}`"));
                }
                at
            } else {
                find_after(
                    serial,
                    b1,
                    "reset step=1 fadt_register space=",
                    "step 1, the FADT register",
                )?
            };
            let b2 = find_after(serial, step1, "stage=boot2 action=power_off", "boot 2")?;
            if fallback {
                let step2 =
                    find_after(serial, step1, "reset step=2 keyboard", "step 2, the i8042")?;
                if step2 > b2 {
                    return Err("serial: boot 2 began before step 2 was logged".to_string());
                }
                if serial[step2..b2].contains("reset step=3") {
                    return Err("serial: the chain went past the i8042 before boot 2".to_string());
                }
            } else if serial[step1..b2].contains("reset step=2") {
                return Err(
                    "serial: the FADT register didn't reset; the chain moved on".to_string()
                );
            }
            find_after(serial, b2, "power: S5 PM1a=", "boot 2's PM1 writes")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(event: &str, reason: Option<&str>) -> QmpEvent {
        QmpEvent {
            event: event.to_string(),
            reason: reason.map(str::to_string),
        }
    }

    // Lines captured from QEMU 8.2.2 (`real_qemu_reports_reset_then_s5`'s
    // qmp.log: the i8042 reset, then S5).
    const SHUTDOWN_S5: &str = r#"{"timestamp": {"seconds": 1793255472, "microseconds": 491552}, "event": "SHUTDOWN", "data": {"guest": true, "reason": "guest-shutdown"}}"#;
    const RESET_GUEST: &str = r#"{"timestamp": {"seconds": 1793255472, "microseconds": 263497}, "event": "RESET", "data": {"guest": true, "reason": "guest-reset"}}"#;

    #[test]
    fn events_parse() {
        assert_eq!(
            parse_event(SHUTDOWN_S5),
            Some(ev("SHUTDOWN", Some("guest-shutdown")))
        );
        assert_eq!(
            parse_event(RESET_GUEST),
            Some(ev("RESET", Some("guest-reset")))
        );
        assert_eq!(
            parse_event(r#"{"timestamp": {"seconds": 1, "microseconds": 2}, "event": "RESUME"}"#),
            Some(ev("RESUME", None))
        );
        assert_eq!(parse_event(r#"{"return": {}}"#), None);
        assert_eq!(
            parse_event(r#"{"QMP": {"version": {}, "capabilities": ["oob"]}}"#),
            None
        );
    }

    const POWER_OFF_SERIAL: &str = "NEO_TEST/1 event=BEGIN name=ot90_s5_exits_qemu phase=dispatch\n\
         NEO_TEST/1 event=PASS name=ot90_s5_exits_qemu stage=power_off_requested\n\
         [ kernel ] power: off plan=Legacy\n[ kernel ] power: S5 PM1a=0x604 write1=0x1 write2=0x2001\n";

    #[test]
    fn power_off_needs_guest_shutdown_and_no_reset() {
        let s5 = [ev("RESUME", None), ev("SHUTDOWN", Some("guest-shutdown"))];
        assert_eq!(judge(PowerExpect::PowerOff, &s5, POWER_OFF_SERIAL), Ok(()));
        // A reset under -no-reboot ends QEMU with status 0 too: refused.
        let reset = [ev("SHUTDOWN", Some("guest-reset"))];
        assert!(judge(PowerExpect::PowerOff, &reset, POWER_OFF_SERIAL).is_err());
        // No event at all (QEMU killed, or hung): refused.
        assert!(judge(PowerExpect::PowerOff, &[], POWER_OFF_SERIAL).is_err());
        let both = [
            ev("RESET", Some("guest-reset")),
            ev("SHUTDOWN", Some("guest-shutdown")),
        ];
        assert!(judge(PowerExpect::PowerOff, &both, POWER_OFF_SERIAL).is_err());
        // The right event without the PM1 writes in the log: refused.
        let no_write = "NEO_TEST/1 event=BEGIN name=x phase=dispatch\n\
                        NEO_TEST/1 event=PASS name=x stage=power_off_requested\n";
        assert!(judge(PowerExpect::PowerOff, &s5, no_write).is_err());
    }

    const BEGIN: &str = "NEO_TEST/1 event=BEGIN name=x phase=dispatch\n";

    fn restart_serial(fallback: bool) -> String {
        let mut s = String::from(BEGIN);
        s += "NEO_TEST/1 event=INFO name=x stage=boot1 action=restart\n";
        if fallback {
            s += "[ kernel ] power: reset step=1 fadt_register neutral (test) value=0xf to port 0x80\n";
            s += "[ kernel ] power: reset step=1 wait ended us=6000000\n";
            s += "[ kernel ] power: reset step=2 keyboard 0xFE to 0x64\n";
        } else {
            s += "[ kernel ] power: reset step=1 fadt_register space=1 addr=0xcf9 value=0xf\n";
        }
        s += BEGIN;
        s += "NEO_TEST/1 event=PASS name=x stage=boot2 action=power_off\n";
        s += "[ kernel ] power: S5 PM1a=0x604 write1=0x1 write2=0x2001\n";
        s
    }

    #[test]
    fn restart_needs_one_reset_then_guest_shutdown() {
        let ok = [
            ev("RESET", Some("guest-reset")),
            ev("SHUTDOWN", Some("guest-shutdown")),
        ];
        for fallback in [false, true] {
            let e = PowerExpect::RestartThenPowerOff { fallback };
            assert_eq!(
                judge(e, &ok, &restart_serial(fallback)),
                Ok(()),
                "fallback={fallback}"
            );
            // No reset (boot 1 powered off instead): refused.
            assert!(judge(e, &ok[1..], &restart_serial(fallback)).is_err());
            // Two RESETs for one guest reset (SeaBIOS's own hard reboot): fine.
            let twice = [ok[0].clone(), ok[0].clone(), ok[1].clone()];
            assert_eq!(judge(e, &twice, &restart_serial(fallback)), Ok(()));
            // A boot loop: boot 1 ran twice. Refused.
            let looped = restart_serial(fallback).replacen(
                "NEO_TEST/1 event=INFO",
                "NEO_TEST/1 event=INFO name=x stage=boot1 action=restart\nNEO_TEST/1 event=INFO",
                1,
            );
            assert!(judge(e, &twice, &looped).is_err());
            // A RESET for another reason (e.g. host-qmp-system-reset): refused.
            let host = [ev("RESET", Some("host-qmp-system-reset")), ok[1].clone()];
            assert!(judge(e, &host, &restart_serial(fallback)).is_err());
            // Boot 2 never powered off: refused.
            assert!(judge(e, &ok[..1], &restart_serial(fallback)).is_err());
            // The other variant's serial: refused.
            assert!(judge(e, &ok, &restart_serial(!fallback)).is_err());
        }
        // The AAG's F4: boot 1 resets, the next boot crashes (a triple fault,
        // also guest-reset) before the scenario clears CMOS, and a third boot
        // prints boot 2 once and powers off. Three BEGIN frames: refused, with
        // the crash's RESET or without it.
        for fallback in [false, true] {
            let e = PowerExpect::RestartThenPowerOff { fallback };
            let crashed = restart_serial(fallback).replacen(
                "NEO_TEST/1 event=PASS name=x stage=boot2",
                "NEO_TEST/1 event=BEGIN name=x phase=dispatch\nNEO_TEST/1 event=PASS name=x stage=boot2",
                1,
            );
            let three = [ok[0].clone(), ok[0].clone(), ok[0].clone(), ok[1].clone()];
            assert!(judge(e, &three, &crashed).is_err());
            let two = [ok[0].clone(), ok[0].clone(), ok[1].clone()];
            assert!(judge(e, &two, &crashed).is_err());
            // A third RESET alone, with two boots in the serial: refused too.
            assert!(judge(e, &three, &restart_serial(fallback)).is_err());
        }
        // The fallback's neutral step must log its wait ending (the AAG's F5).
        let no_wait = restart_serial(true)
            .replace("[ kernel ] power: reset step=1 wait ended us=6000000\n", "");
        let e = PowerExpect::RestartThenPowerOff { fallback: true };
        assert!(judge(e, &ok, &no_wait).is_err());
        // #154 F5 (Jarvis's negative control): the valid fallback serial with
        // its wait shortened to `us=0`, and valid RESET/SHUTDOWN events, is
        // refused; so is any other duration than the planned one.
        for us in ["0", "4000000", "60000000"] {
            let short = restart_serial(true)
                .replace("wait ended us=6000000", &format!("wait ended us={us}"));
            assert!(judge(e, &ok, &short).is_err(), "us={us}");
        }
        assert_eq!(judge(e, &ok, &restart_serial(true)), Ok(()));
        // The fallback chain must stop at the i8042: a step 3 before boot 2 is refused.
        let past = restart_serial(true).replace(
            "NEO_TEST/1 event=PASS",
            "[ kernel ] power: reset step=3 cf9 0x2 then 0xe\nNEO_TEST/1 event=PASS",
        );
        let e = PowerExpect::RestartThenPowerOff { fallback: true };
        assert!(judge(e, &ok, &past).is_err());
        // The direct restart must not need step 2.
        let moved = restart_serial(false).replace(
            "NEO_TEST/1 event=PASS",
            "[ kernel ] power: reset step=2 keyboard 0xFE to 0x64\nNEO_TEST/1 event=PASS",
        );
        let e = PowerExpect::RestartThenPowerOff { fallback: false };
        assert!(judge(e, &ok, &moved).is_err());
    }

    /// A boot sector, hand-assembled (16-bit real mode, loaded at 0x7C00
    /// by SeaBIOS, which sets the ICH9 PM base to 0x600 on q35):
    /// - CMOS 0x7E != 0xA5: write 0xA5 there, then `0xFE` to the i8042 command
    ///   port (a reset);
    /// - otherwise: SLP_TYP 0 + SLP_EN to PM1a_CNT 0x604 (S5 on QEMU).
    ///
    /// The same CMOS-mark shape as `ot90_restart_*` in the kernel.
    fn restart_then_s5_sector(reset: &[u8]) -> Vec<u8> {
        let mut code = vec![
            0xB0, 0x7E, // mov al, 0x7E
            0xE6, 0x70, // out 0x70, al
            0xE4, 0x71, // in al, 0x71
            0x3C, 0xA5, // cmp al, 0xA5
            0x74, 0x00, // je s5 (patched below)
            0xB0, 0x7E, // mov al, 0x7E
            0xE6, 0x70, // out 0x70, al
            0xB0, 0xA5, // mov al, 0xA5
            0xE6, 0x71, // out 0x71, al
        ];
        code.extend_from_slice(reset);
        code.push(0xF4); // hlt
        let s5 = code.len();
        code[9] = (s5 - 10) as u8;
        code.extend_from_slice(&[
            0xBA, 0x04, 0x06, // mov dx, 0x604
            0xB8, 0x00, 0x20, // mov ax, 0x2000 (SLP_EN, SLP_TYP 0)
            0xEF, // out dx, ax
            0xF4, 0xEB, 0xFD, // hlt; jmp hlt
        ]);
        code.resize(510, 0);
        code.extend_from_slice(&[0x55, 0xAA]);
        // SeaBIOS can't read a one-sector disk (its geometry comes out as 0
        // cylinders): pad to 1 MiB.
        code.resize(1 << 20, 0);
        code
    }

    fn run_sector(name: &str, sector: &[u8], no_reboot: bool) -> Vec<QmpEvent> {
        let dir = std::env::temp_dir().join(format!("xtask-qmp-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let disk = dir.join("boot.img");
        std::fs::write(&disk, sector).unwrap();
        let recorder = QmpRecorder::start(&dir.join("qmp.log")).unwrap();
        let mut qemu = std::process::Command::new("qemu-system-x86_64");
        qemu.args(["-machine", "q35", "-display", "none", "-monitor", "none"])
            .arg("-drive")
            .arg(format!("file={},format=raw", disk.display()))
            .args(recorder.qemu_args());
        if no_reboot {
            qemu.arg("-no-reboot");
        }
        let mut child = qemu.spawn().expect("qemu-system-x86_64");
        let start = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if start.elapsed() > Duration::from_secs(60) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{name}: QEMU did not stop");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        recorder.finish().unwrap()
    }

    /// Real QEMU: the recorder sees why it stopped. Needs `qemu-system-x86_64`,
    /// so it is opt-in: `cargo test -p xtask qmp -- --ignored`.
    #[test]
    #[ignore]
    fn real_qemu_reports_reset_then_s5() {
        let kbd = [0xB0, 0xFE, 0xE6, 0x64]; // mov al, 0xFE; out 0x64, al
        let cf9 = [0xBA, 0xF9, 0x0C, 0xB0, 0x0F, 0xEE]; // mov dx, 0xCF9; mov al, 0x0F; out dx, al
        for (name, reset) in [("kbd", &kbd[..]), ("cf9", &cf9[..])] {
            let events = run_sector(name, &restart_then_s5_sector(reset), false);
            eprintln!("{name}: {events:?}");
            let relevant: Vec<_> = events
                .iter()
                .filter(|e| e.event == "RESET" || e.event == "SHUTDOWN")
                .cloned()
                .collect();
            // SeaBIOS turns the guest's reset into two RESET events (it hard-
            // reboots once more itself), then the second boot's S5.
            assert_eq!(
                relevant,
                [
                    ev("RESET", Some("guest-reset")),
                    ev("RESET", Some("guest-reset")),
                    ev("SHUTDOWN", Some("guest-shutdown"))
                ],
                "{name}"
            );
            // And `judge` accepts that sequence.
            let serial = "NEO_TEST/1 event=BEGIN name=x\n\
                          stage=boot1 action=restart reset step=1 fadt_register space=1\n\
                          NEO_TEST/1 event=BEGIN name=x\n\
                          stage=boot2 action=power_off power: S5 PM1a=0x604\n";
            let e = PowerExpect::RestartThenPowerOff { fallback: false };
            assert_eq!(judge(e, &events, serial), Ok(()), "{name}");
        }
        // Under -no-reboot the reset itself ends QEMU: SHUTDOWN, guest-reset.
        let events = run_sector("noreboot", &restart_then_s5_sector(&kbd), true);
        assert_eq!(
            events
                .iter()
                .filter(|e| e.event == "SHUTDOWN")
                .cloned()
                .collect::<Vec<_>>(),
            [ev("SHUTDOWN", Some("guest-reset"))]
        );
    }
}
