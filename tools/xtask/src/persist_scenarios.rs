// SPDX-License-Identifier: MPL-2.0
//! OT-100 R3: the persistence scenarios (THINK §10, R3's gate). Each boots
//! the persistent image as a `usb-storage` stick on `qemu-xhci` under the
//! pinned OVMF (the image boots from a stick only through UEFI, as on the
//! Zenbook), with the kernel harness `run_usb_persist`, and holds the run to
//! its own expectations here, on the host.
//!
//! - `usb_persist_two_boot`: boot 1 mounts the area read-write and writes
//!   `boot-1`; boot 2, the same stick, finds it and writes `boot-2`. Outside
//!   the area the stick stays byte-identical to the image.
//! - `usb_persist_try`: the "try" entry (`persist=off`): read-only, and the
//!   stick's hash is unchanged.
//! - `usb_persist_absent`: an image without the area: nothing mounted, the
//!   stick unchanged.
//! - `usb_persist_corrupt`: every superblock copy in the area damaged: the
//!   guard arms nothing, nothing is mounted, the stick unchanged.
//! - `usb_persist_second_stick`: a second persistent stick from another
//!   build: the boot stick is read-write, the second is never written.
//! - `usb_persist_two_sticks`: two sticks from one image share the disk
//!   GUID: the boot device is not identified, nothing is read-write, neither
//!   stick changes.
//! - `usb_persist_never_internal` (THINK §5): plus an NVMe disk with a
//!   Windows-shaped GPT and sealed superblocks planted in its data
//!   partition: the census says it is armed with nothing, the raw write is
//!   refused, and its hash is unchanged.
//! - `usb_persist_unplug_midwrite` (THINK §4.2, 4 CPUs): the stick is pulled
//!   mid-write; the volume is lost once and goes read-only. Boot 2, with the
//!   stick back, mounts it read-write at its last committed generation
//!   (`boot-1` is there).

use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    ISO_OUTPUT_REL, ISO_PERSIST_MIB, ISO_PERSIST_TRY_DEFAULT, ObservedTest, ScenarioBundleMode,
    TEST_OUTPUT_REL, TestScenario, blake3_file_hex, build_test_artifacts_for_scenario,
    copy_boot_attempt_log_to_canonical, neo_test_run_id_suffix, persist_image, remove_serial_log,
    require_observed_pass, require_serial_contains, resolve_ovmf, run_qemu_test_invocation,
    scenario_serial_log, vm_image, vm_vars_copy, workspace_root,
};

/// The area's size in the scenarios (just above NeoFS's 64 MiB floor).
const SCENARIO_MIB: u32 = 70;
/// The Windows-shaped internal disk's size.
const INTERNAL_DISK_BYTES: u64 = 128 << 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    TwoBoot,
    NeverInternal,
    Unplug,
    Try,
    Absent,
    Corrupt,
    SecondStick,
    TwoSticks,
}

const KINDS: [(&str, Kind); 8] = [
    ("usb_persist_two_boot", Kind::TwoBoot),
    ("usb_persist_never_internal", Kind::NeverInternal),
    ("usb_persist_unplug_midwrite", Kind::Unplug),
    ("usb_persist_try", Kind::Try),
    ("usb_persist_absent", Kind::Absent),
    ("usb_persist_corrupt", Kind::Corrupt),
    ("usb_persist_second_stick", Kind::SecondStick),
    ("usb_persist_two_sticks", Kind::TwoSticks),
];

fn kind(name: &str) -> Option<Kind> {
    KINDS.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}

pub fn is_persist_scenario(name: &str) -> bool {
    kind(name).is_some()
}

/// The unplug scenario runs on 4 CPUs (THINK §5.1: the read-only flip races
/// mutations on other CPUs).
pub fn scenario_smp(name: &str) -> Option<&'static str> {
    (kind(name) == Some(Kind::Unplug)).then_some("4")
}

/// The QEMU arguments: OVMF, `qemu-xhci`, the boot stick (`hotstick`, the
/// id the HMP unplug step names; boot index 0), any second stick, and an
/// optional NVMe disk.
fn qemu_args(
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    stick: &Path,
    second: Option<&Path>,
    nvme: Option<&Path>,
) -> Vec<String> {
    let mut a = vec![
        "-drive".to_string(),
        format!(
            "if=pflash,format=raw,unit=0,readonly=on,file={}",
            ovmf.code.display()
        ),
        "-drive".to_string(),
        format!("if=pflash,format=raw,unit=1,file={}", vars.display()),
        "-device".to_string(),
        "qemu-xhci,id=xhci0".to_string(),
        "-drive".to_string(),
        format!("if=none,id=usbstick,format=raw,file={}", stick.display()),
        "-device".to_string(),
        "usb-storage,drive=usbstick,bus=xhci0.0,id=hotstick,bootindex=0".to_string(),
    ];
    if let Some(second) = second {
        a.push("-drive".to_string());
        a.push(format!(
            "if=none,id=usbstick2,format=raw,file={}",
            second.display()
        ));
        a.push("-device".to_string());
        a.push("usb-storage,drive=usbstick2,bus=xhci0.0,id=secondstick".to_string());
    }
    if let Some(nvme) = nvme {
        a.push("-drive".to_string());
        a.push(format!(
            "if=none,id=internal,format=raw,file={}",
            nvme.display()
        ));
        a.push("-device".to_string());
        a.push("nvme,drive=internal,serial=WINDOWS-NVME".to_string());
    }
    a
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

/// The harness's `USB_PERSIST mount=… counter_before=… counter_after=…`.
fn mount_line(observed: &ObservedTest) -> Result<(String, i64, i64), String> {
    let serial = fs::read_to_string(&observed.serial_log)
        .map_err(|e| format!("read {}: {e}", observed.serial_log.display()))?;
    let line = serial
        .lines()
        .find_map(|l| l.split_once("USB_PERSIST mount=").map(|(_, r)| r))
        .ok_or("no `USB_PERSIST mount=` line")?;
    let mut words = line.split_whitespace();
    let mode = words.next().unwrap_or("").to_string();
    let field = |w: Option<&str>, key: &str| -> Result<i64, String> {
        w.and_then(|w| w.strip_prefix(key))
            .and_then(|v| v.parse().ok())
            .ok_or(format!("bad `USB_PERSIST mount=` line: {line}"))
    };
    let before = field(words.next(), "counter_before=")?;
    let after = field(words.next(), "counter_after=")?;
    Ok((mode, before, after))
}

fn serial_count(observed: &ObservedTest, needle: &str) -> Result<usize, String> {
    let serial = fs::read_to_string(&observed.serial_log)
        .map_err(|e| format!("read {}: {e}", observed.serial_log.display()))?;
    Ok(serial.matches(needle).count())
}

fn expect(
    scenario: TestScenario,
    observed: &ObservedTest,
    want: (&str, i64, i64),
) -> Result<(), String> {
    let got = mount_line(observed)?;
    if (got.0.as_str(), got.1, got.2) != want {
        return Err(format!(
            "`{}`: mount={} counter {}->{}, want mount={} counter {}->{}; serial={}",
            scenario.name,
            got.0,
            got.1,
            got.2,
            want.0,
            want.1,
            want.2,
            observed.serial_log.display()
        ));
    }
    Ok(())
}

fn same(scenario: TestScenario, what: &str, path: &Path, digest: &str) -> Result<(), String> {
    let now = blake3_file_hex(path)?;
    if now != digest {
        return Err(format!(
            "`{}`: {what} changed (blake3 {digest} -> {now}); nothing may write it",
            scenario.name
        ));
    }
    Ok(())
}

/// One persistence scenario, one attempt.
pub fn run(scenario: TestScenario, attempt: usize) -> Result<(), String> {
    use std::sync::atomic::Ordering::SeqCst;
    let kind = kind(scenario.name).ok_or("not a persistence scenario")?;
    ISO_PERSIST_MIB.store(
        if kind == Kind::Absent {
            0
        } else {
            SCENARIO_MIB
        },
        SeqCst,
    );
    ISO_PERSIST_TRY_DEFAULT.store(kind == Kind::Try, SeqCst);
    let built = build_test_artifacts_for_scenario(scenario, ScenarioBundleMode::Vmware);
    ISO_PERSIST_MIB.store(0, SeqCst);
    ISO_PERSIST_TRY_DEFAULT.store(false, SeqCst);
    built?;

    let ovmf = resolve_ovmf(true)?;
    let dir = workspace_root().join(TEST_OUTPUT_REL).join(scenario.name);
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let image = read(&workspace_root().join(ISO_OUTPUT_REL))?;
    let stick = dir.join("stick.raw");
    let mut stick_bytes = image.clone();
    if kind == Kind::Corrupt {
        let (first, _) = persist_image::area_span(&image).ok_or("the image has no area")?;
        for copy in [0u64, 1 << 20, 64 << 20] {
            let at = (first * 512 + copy) as usize;
            stick_bytes[at + 100] ^= 0xff;
        }
    }
    write(&stick, &stick_bytes)?;
    let second: Option<PathBuf> = match kind {
        Kind::SecondStick => {
            let mut other = image.clone();
            persist_image::set_disk_guid(&mut other, [0x2b; 16])?;
            let path = dir.join("second.raw");
            write(&path, &other)?;
            Some(path)
        }
        Kind::TwoSticks => {
            let path = dir.join("second.raw");
            write(&path, &image)?;
            Some(path)
        }
        _ => None,
    };
    let nvme: Option<PathBuf> = if kind == Kind::NeverInternal {
        let (first, _) = persist_image::area_span(&image).ok_or("the image has no area")?;
        let at = (first * 512) as usize;
        let planted: [u8; 512] = image[at..at + 512].try_into().unwrap();
        let disk = persist_image::windows_disk(INTERNAL_DISK_BYTES, &planted);
        if persist_image::guard_claims(&disk) != 0 {
            return Err("the internal disk fixture is armed on the host".to_string());
        }
        let path = dir.join("internal-nvme.raw");
        write(&path, &disk)?;
        Some(path)
    } else {
        None
    };
    let stick_digest = blake3_file_hex(&stick)?;
    let second_digest = second.as_deref().map(blake3_file_hex).transpose()?;
    let nvme_digest = nvme.as_deref().map(blake3_file_hex).transpose()?;
    let vars = dir.join("vars.fd");

    let boot = |n: usize| -> Result<ObservedTest, String> {
        let log = scenario_serial_log(&format!(
            "{}{}.attempt{}.boot{}.serial.log",
            scenario.name,
            neo_test_run_id_suffix(),
            attempt,
            n
        ));
        remove_serial_log(&log)?;
        vm_vars_copy(&ovmf, &vars)?;
        let args = qemu_args(&ovmf, &vars, &stick, second.as_deref(), nvme.as_deref());
        let result = run_qemu_test_invocation(
            scenario,
            ScenarioBundleMode::Vmware,
            log.clone(),
            &[],
            Some(&args),
        );
        let _ = copy_boot_attempt_log_to_canonical(scenario.name, n, &log);
        let observed = result?;
        require_observed_pass(scenario, &observed)?;
        Ok(observed)
    };

    match kind {
        Kind::TwoBoot => {
            let first = boot(1)?;
            expect(scenario, &first, ("rw", 0, 1))?;
            require_serial_contains(
                scenario,
                &first,
                "persist: boot device identified",
                "the boot stick must be identified by its GPT disk GUID",
            )?;
            let second_boot = boot(2)?;
            expect(scenario, &second_boot, ("rw", 1, 2))?;
            // Outside the area the stick is the image, byte for byte.
            let after = read(&stick)?;
            let (a, b) = persist_image::area_span(&image).ok_or("no area")?;
            let (lo, hi) = ((a * 512) as usize, ((b + 1) * 512) as usize);
            if after.len() != image.len()
                || after[..lo] != image[..lo]
                || after[hi..] != image[hi..]
            {
                return Err(format!(
                    "`{}`: the stick changed outside the area ({lo:#x}..{hi:#x})",
                    scenario.name
                ));
            }
            if after[lo..hi] == image[lo..hi] {
                return Err(format!("`{}`: the area was never written", scenario.name));
            }
        }
        Kind::Try => {
            let observed = boot(1)?;
            expect(scenario, &observed, ("ro", 0, 0))?;
            require_serial_contains(
                scenario,
                &observed,
                "persist: cmdline persist=off",
                "the try entry boots with persist=off",
            )?;
            require_serial_contains(scenario, &observed, "(persist=off)", "the ro reason")?;
            same(scenario, "the stick", &stick, &stick_digest)?;
        }
        Kind::Absent | Kind::Corrupt => {
            let observed = boot(1)?;
            expect(scenario, &observed, ("none", -1, -1))?;
            same(scenario, "the stick", &stick, &stick_digest)?;
        }
        Kind::SecondStick => {
            let observed = boot(1)?;
            expect(scenario, &observed, ("rw", 0, 1))?;
            require_serial_contains(
                scenario,
                &observed,
                "persist: boot device identified",
                "the boot stick is identified; the second has another GUID",
            )?;
            same(
                scenario,
                "the second stick",
                second.as_deref().unwrap(),
                second_digest.as_deref().unwrap(),
            )?;
        }
        Kind::TwoSticks => {
            let observed = boot(1)?;
            expect(scenario, &observed, ("ro", 0, 0))?;
            require_serial_contains(
                scenario,
                &observed,
                "persist: boot device not identified (guid=known, matches=2)",
                "two sticks from one image share the GUID: not identified",
            )?;
            same(scenario, "the boot stick", &stick, &stick_digest)?;
            same(
                scenario,
                "the second stick",
                second.as_deref().unwrap(),
                second_digest.as_deref().unwrap(),
            )?;
        }
        Kind::NeverInternal => {
            let observed = boot(1)?;
            expect(scenario, &observed, ("rw", 0, 1))?;
            require_serial_contains(
                scenario,
                &observed,
                "driver=nvme index=",
                "the census names the internal NVMe",
            )?;
            let serial = fs::read_to_string(&observed.serial_log)
                .map_err(|e| format!("read {}: {e}", observed.serial_log.display()))?;
            let census = serial
                .lines()
                .find(|l| l.contains("persist: guard census") && l.contains("driver=nvme"))
                .ok_or("no census line for the NVMe")?;
            if !census.contains("armed=none") || !census.contains("table=gpt") {
                return Err(format!("`{}`: NVMe census: {census}", scenario.name));
            }
            let raw = serial
                .lines()
                .find(|l| l.contains("USB_PERSIST raw-write") && l.contains("driver=nvme"))
                .ok_or("no raw-write line for the NVMe")?;
            if !raw.contains("result=refused") {
                return Err(format!("`{}`: NVMe raw write: {raw}", scenario.name));
            }
            same(
                scenario,
                "the internal NVMe",
                nvme.as_deref().unwrap(),
                nvme_digest.as_deref().unwrap(),
            )?;
        }
        Kind::Unplug => {
            let first = boot(1)?;
            expect(scenario, &first, ("rw", 0, 1))?;
            require_serial_contains(
                scenario,
                &first,
                "USB_PERSIST after-loss",
                "the stick was pulled and the mount went read-only",
            )?;
            let lost = serial_count(&first, "persist: volume lost (")?;
            if lost != 1 {
                return Err(format!(
                    "`{}`: `persist: volume lost` printed {lost} times, want exactly 1",
                    scenario.name
                ));
            }
            // Boot 2, the stick back: the last committed generation mounts
            // read-write, with boot 1's synced `boot-1`.
            let second_boot = boot(2)?;
            let (mode, before, _) = mount_line(&second_boot)?;
            if mode != "rw" || before < 1 {
                return Err(format!(
                    "`{}` boot 2: mount={mode} counter_before={before}, want rw and >= 1",
                    scenario.name
                ));
            }
        }
    }
    eprintln!("[xtask] neo-test: `{}` PASS (host checks)", scenario.name);
    Ok(())
}
