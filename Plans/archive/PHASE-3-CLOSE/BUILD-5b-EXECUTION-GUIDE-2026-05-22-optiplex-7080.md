# BUILD-5b Execution Guide — OptiPlex 7080 Micro

**Audience:** PL, performing bare-metal verification of Phase 3 NeoFS on real hardware.
**Date filed:** 2026-05-22
**Target hardware:** Dell OptiPlex 7080 Micro, Service Tag D4SHKD3, BIOS 1.37.0, SATA mode AHCI.
**Storage:** Samsung 860 EVO 250 GB on 2.5" SATA bay + Intel E6100p 256 GB NVMe on M.2 SSD-3.
**Authority:** `Plans/PROCEDURE-2026-05-22-phase-3-build-5b-bare-metal-optiplex.md` (formal procedure); this is the operator-facing walkthrough.

---

## What's new in this revision (2026-05-23)

This revision of the procedure is paired with kernel commit `2223617` which adds **block-sink degradation visibility + bare-metal selftest** (Codex dispatch closed 2026-05-23). Two operational changes from the first BUILD-5b attempt:

1. **New possible framebuffer FAIL state.** If the kernel's boot-time log-sink selftest detects that log writes aren't reaching the NVMe (or SATA, for the NVMe scenario), the framebuffer will now show **`FAIL LOG SELFTEST`** in red instead of plain `FAIL`. This means the kernel caught the log path failing *before* running the scenario — the on-disk header v2 degradation fields will tell us what failed.

2. **`neo-extract-log` prints a Degradation block** when one is recorded. Look for it in the tool's output:
   ```
   Degradation detected:
     reason: <Io / InvalidRequest / Unsupported / Other>
     op:     <read_block_for_rmw / write_block_payload / write_block_header_rewrite / device_flush_after_write>
     lba:    <decimal>
     cursor: <decimal>
   ```
   If you see this block, photograph or screenshot it — that's the diagnostic data that unblocks the next fix.

Expected outcomes for this BUILD-5b re-run:

- **`FAIL LOG SELFTEST` on framebuffer + Degradation block in extraction** — the kernel caught the broken primitive early. Diagnostic data tells us which primitive. Most likely outcome given the prior captured_bytes=0 result.
- **Plain `FAIL` on framebuffer + Degradation block in extraction** — selftest passed, but a later flush failed. Also actionable.
- **`PASS` on framebuffer + clean log** — the prior FAIL was an intermittent issue. Less likely.

No procedure-flow changes. Same USB sticks, same dd targets, same boot sequence.

---

## At a glance

You will run **two scenarios** against the OptiPlex. Each scenario writes a NeoFS filesystem to one of the two SSDs, then reads it back and verifies the data. While the scenario runs, the kernel logs are written to the OTHER SSD as a "log sink." After each run, you boot back into Linux and extract the log.

**Both scenarios must PASS** for Phase 3 to formally close.

- **Run 1 (AHCI scenario):** writes/reads on Samsung SATA SSD; logs to Intel NVMe.
- **Run 2 (NVMe scenario):** writes/reads on Intel NVMe; logs to Samsung SATA SSD.

The OptiPlex's SSDs **will be wiped** during this. That's intentional and expected.

---

## Materials needed

Before starting:

- [ ] **Three USB sticks**, any size ≥ 8 GB each. Stick A will be Ubuntu Live; Stick B will be a NeoPlutarch scenario ISO (you'll reflash it between runs); Stick C will be a regular data drive.
- [ ] **Rufus** (Windows USB flasher). Download from https://rufus.ie/ if you don't have it.
- [ ] **Ubuntu Desktop 24.04 LTS ISO**. Download from https://ubuntu.com/download/desktop (any recent Ubuntu/Fedora/Mint live ISO works; this guide uses Ubuntu).
- [ ] **The OptiPlex 7080 Micro**, powered off, monitor + keyboard + mouse attached.
- [ ] **Camera or phone** for photographing the framebuffer phase indicator (evidence capture).
- [ ] **A notepad** for recording any FAIL diagnostics if the worst happens.

The NeoPlutarch files are already staged for you in this folder:

```
E:\AI Automation\AltimitNeo\BUILD-5b-staging\
├── neo-extract-log              (385 KB) — Linux ELF binary, extracts kernel logs
├── ahci\                        AHCI scenario bundle
│   ├── ahci-scenario.iso        (24.7 MB) — boot ISO for AHCI run
│   ├── ahci-scenario-disk.raw   (128 MB)  — NeoFS image; dd to Samsung SATA
│   └── ahci-log-sink-disk.raw   (288 MB)  — log target; dd to Intel NVMe
└── nvme\                        NVMe scenario bundle
    ├── nvme-scenario.iso        (24.7 MB) — boot ISO for NVMe run
    ├── nvme-scenario-disk.raw   (128 MB)  — NeoFS image; dd to Intel NVMe
    └── nvme-log-sink-disk.raw   (288 MB)  — log target; dd to Samsung SATA
```

---

## Time required

- Pre-flight at your desk: **30–45 minutes** (mostly USB flashing).
- Run 1 at the OptiPlex: **~20 minutes** (provisioning + boot + observation + extraction).
- Run 2 at the OptiPlex: **~20 minutes**.
- Evidence capture and commit: **~15 minutes**.

Total: **about 90 minutes** of focused work.

---

# PRE-FLIGHT — at your desk

## Step 1. Download Ubuntu Desktop ISO

Open browser, go to https://ubuntu.com/download/desktop. Download the latest LTS (Ubuntu 24.04 or newer). Save the .iso file somewhere convenient — Downloads is fine. The file will be around 5–6 GB.

## Step 2. Flash USB Stick A with Ubuntu Live

1. Insert USB Stick A into your Windows computer.
2. Open Rufus.
3. Under **Device**, select USB Stick A. **Triple-check** — Rufus will wipe whatever you point it at. Look at the size; if it doesn't match your USB stick, you've got the wrong drive.
4. Under **Boot selection**, click SELECT and choose the Ubuntu .iso you downloaded.
5. Leave **Partition scheme** = GPT, **Target system** = UEFI (non CSM).
6. Click **START**.
7. If Rufus asks "ISO Image mode" vs "DD Image mode," choose **ISO Image mode**.
8. If Rufus warns about overwriting, click OK.
9. Wait until status reads READY. Eject the stick.
10. **Label Stick A with masking tape: "UBUNTU LIVE"**

## Step 3. Flash USB Stick B with the AHCI scenario ISO

1. Insert USB Stick B into your Windows computer.
2. In Rufus: select USB Stick B as the device.
3. Click SELECT and choose:
   ```
   E:\AI Automation\AltimitNeo\BUILD-5b-staging\ahci\ahci-scenario.iso
   ```
4. **Partition scheme** = GPT, **Target system** = UEFI (non CSM).
5. Click **START**.
6. **IMPORTANT:** When Rufus asks ISO Image mode vs DD Image mode, choose **DD Image mode**. This ISO is a Limine-built hybrid image; it boots correctly only in DD mode.
7. Confirm any overwrite warnings.
8. Wait for READY. Eject.
9. **Label Stick B with masking tape: "AHCI SCENARIO"**

## Step 4. Prepare USB Stick C as a regular data drive

1. Insert USB Stick C into your Windows computer.
2. Open File Explorer; if the drive isn't already formatted FAT32, right-click it → Format → File system = **exFAT** or **FAT32** → Quick Format → Start. (exFAT is recommended for any stick larger than 32 GB.)
3. Once formatted, copy the entire `BUILD-5b-staging` folder to the stick. You can do this in Explorer or PowerShell:

   ```powershell
   xcopy "E:\AI Automation\AltimitNeo\BUILD-5b-staging" "<DRIVE>:\BUILD-5b-staging\" /E /I
   ```

   Where `<DRIVE>` is your USB stick's drive letter (e.g., `F`).
4. Verify the copy completed; all 7 files should be present on the stick.
5. **Label Stick C with masking tape: "DATA"**
6. Safely eject.

You now have three labeled sticks. **You're ready to go to the OptiPlex.**

---

# AT THE OPTIPLEX — Run 1 (AHCI scenario)

## Step 5. Power-on with Ubuntu Live

1. With the OptiPlex powered off, insert **Stick A (UBUNTU LIVE)** into a USB port.
2. Power on the OptiPlex.
3. Immediately press **F12** repeatedly to enter the boot menu.
4. In the boot menu, select **UEFI: <your USB stick name>** (e.g., "UEFI: Kingston DataTraveler 3.0").
5. Ubuntu Live boot menu appears. Select **Try or Install Ubuntu** (top option).
6. Wait for Ubuntu to load. When the Welcome screen appears, click **Try Ubuntu** (do NOT click Install — that would install Ubuntu to the SSD, which we don't want).
7. You should see the Ubuntu desktop with a Files icon, etc.

## Step 6. Open a terminal

Press **Ctrl + Alt + T** to open a terminal.

## Step 7. Insert Stick C and identify all drives

1. Insert **Stick C (DATA)** into a second USB port.
2. In the terminal, type:

   ```bash
   lsblk -o NAME,SIZE,MODEL,SERIAL
   ```

3. You should see output like:

   ```
   NAME        SIZE MODEL                    SERIAL
   sda       232.9G Samsung SSD 860 EVO 250G S5XXXXXXXX
   nvme0n1   238.5G Intel SSDPEKNW256G8       BTXXXXXXXX
   sdb        14.4G <ubuntu live stick model>
   sdc        29.8G <data stick model>
   ```

4. **Write down which device is which.** From the output above:
   - **Samsung SSD 860 EVO 250G** → `/dev/sda` (the SATA SSD; this is what AHCI scenario will write to)
   - **Intel SSDPEKNW256G8** → `/dev/nvme0n1` (the NVMe SSD; this is the AHCI log sink target)
   - The other two `sdb` and `sdc` are your USB sticks. Whichever one matches Stick C's size is your data drive.

5. **Verify before continuing.** If your output shows the Samsung as something other than `/dev/sda`, or the Intel NVMe as something other than `/dev/nvme0n1`, **stop and substitute the correct paths in every subsequent command.** Wrong device path = wrong drive wiped.

## Step 8. Mount the data USB stick

1. Find Stick C's filesystem path. Usually Ubuntu auto-mounts at `/media/ubuntu/<label>`. Run:

   ```bash
   ls /media/ubuntu/
   ```

   You should see a folder name corresponding to your data stick.

2. Set a shell variable to the staging folder for convenience:

   ```bash
   STAGING=/media/ubuntu/<your-stick-name>/BUILD-5b-staging
   ls $STAGING
   ```

   You should see `ahci/`, `nvme/`, `neo-extract-log`.

3. If `ls` doesn't show the expected files, the stick may not be mounted yet. Wait 10 seconds and try again. If still not visible, click the Files icon in the Ubuntu sidebar to trigger auto-mount.

## Step 9. Provision the AHCI scenario disk onto the Samsung SATA SSD

**WARNING:** This command **completely wipes** the Samsung SATA SSD. Confirmed acceptable per BUILD-5b procedure.

1. **Verify one more time** that `/dev/sda` is the Samsung. Run:

   ```bash
   lsblk -o NAME,SIZE,MODEL /dev/sda
   ```

   Should print Samsung. **If not, stop. Substitute the correct path.**

2. Write the scenario disk image:

   ```bash
   sudo dd if=$STAGING/ahci/ahci-scenario-disk.raw of=/dev/sda bs=4M status=progress conv=fsync
   ```

   This takes ~5 seconds. You'll see:
   ```
   128+0 records in
   128+0 records out
   134217728 bytes (134 MB, 128 MiB) copied, X.X s, XX.X MB/s
   ```

## Step 10. Provision the AHCI log-sink disk onto the Intel NVMe

**WARNING:** This wipes the Intel NVMe SSD. Confirmed acceptable.

1. Verify the NVMe device:

   ```bash
   lsblk -o NAME,SIZE,MODEL /dev/nvme0n1
   ```

   Should print Intel.

2. Write the log-sink image:

   ```bash
   sudo dd if=$STAGING/ahci/ahci-log-sink-disk.raw of=/dev/nvme0n1 bs=4M status=progress conv=fsync
   ```

   Takes ~10 seconds (~288 MB).

## Step 11. Sync and verify

1. Force everything to disk:

   ```bash
   sudo sync
   ```

2. Verify both disks have the right "magic" data at the start. Quick check:

   ```bash
   sudo head -c 16 /dev/sda | xxd | head -1
   sudo head -c 16 /dev/nvme0n1 | xxd | head -1
   ```

   `/dev/sda` should print NeoFS-related bytes (look for ASCII text starting with `NEOFS` somewhere).
   `/dev/nvme0n1` should print the BlockDeviceLogSink header (look for `NEO_LOG_v1` or similar).

   If both look populated (not all zeros, not all ones), provisioning succeeded.

## Step 12. Shut down the OptiPlex

1. Click the Ubuntu power icon (top-right corner) → Power Off → Power Off.
2. Once the OptiPlex fully powers off, **remove Stick A (UBUNTU LIVE)** from the USB port. **Leave Stick C (DATA) plugged in** if it's in a separate port (it won't interfere). If Stick C is in the only available port, remove it too — you'll re-insert later.

## Step 13. Insert the AHCI scenario boot USB

1. Insert **Stick B (AHCI SCENARIO)** into a USB port.
2. Power on the OptiPlex.
3. Immediately press **F12** repeatedly for the boot menu.
4. Select **UEFI: <Stick B's USB model name>**.
5. The NeoPlutarch scenario kernel boots.

## Step 14. Watch the framebuffer phase indicator

As the scenario runs, the lower-right corner of the screen shows a large (800×200 pixel) status box. The text in the box advances through phases:

```
PROBE             → kernel is probing PCI devices, registering AHCI + NVMe drivers
LOG_SINK_READY    → block-device log sink initialized on the NVMe (log target)
MOUNT_REGISTERED  → NeoFS mount on the Samsung SATA is active
READ              → reading from the NeoFS image
WRITE             → writing to the NeoFS image
READBACK          → re-reading what was just written to verify
PASS              → all scenario checks passed; kernel halts here (green)
```

If anything goes wrong, the indicator shows **FAIL** in red and the kernel halts.

The scenario takes about **30–60 seconds** from boot to PASS or FAIL.

## Step 15. Photograph the result

1. **Take a clear photo of the framebuffer phase indicator** showing the final PASS (or FAIL) state.
2. Save the photo somewhere convenient (your phone is fine; you'll move it to the project later).
3. Name it something like `build5b-ahci-pass.jpg` or `build5b-ahci-fail.jpg`.

**If FAIL:** continue with extraction (Step 17 onward). The log will explain what failed.
**If PASS:** continue with extraction (Step 17 onward) to capture the PASS log as evidence.

## Step 16. Power off

1. Press the OptiPlex power button briefly (hard power-off — the kernel has halted by design).
2. Once fully off, **remove Stick B (AHCI SCENARIO)** from the USB port.

## Step 17. Extract the AHCI log

1. Insert **Stick A (UBUNTU LIVE)** and **Stick C (DATA)** back into the OptiPlex.
2. Power on, press F12, boot Ubuntu Live again (same as Step 5).
3. Open a terminal (Ctrl+Alt+T).
4. Set the staging path again:

   ```bash
   STAGING=/media/ubuntu/<your-stick-name>/BUILD-5b-staging
   ```

5. Make the extraction tool executable (only needed first time):

   ```bash
   chmod +x $STAGING/neo-extract-log
   ```

6. Verify the NVMe still has the log-sink data (don't dd anything; just inspect):

   ```bash
   lsblk -o NAME,SIZE,MODEL /dev/nvme0n1
   ```

7. Run the extraction tool against the NVMe:

   ```bash
   sudo $STAGING/neo-extract-log --device /dev/nvme0n1 --output /tmp/ahci-run-log.txt
   ```

   This reads from offset 0x10000000 (256 MiB into the NVMe), validates the NEO_LOG_v1 magic header, and writes the captured kernel log to `/tmp/ahci-run-log.txt`. The tool prints a summary including a Degradation block if `degradation_reason != 0` (means the kernel detected and recorded a sink failure on this run).

8. View the last lines of the log to confirm the PASS sentinel:

   ```bash
   tail -30 /tmp/ahci-run-log.txt
   ```

   On a successful run you'll see lines like:
   ```
   [ phase ] READBACK
   [ scenario ] verification: pattern matched
   [ scenario ] PASS
   ```

   On FAIL you'll see the failure message. Read it carefully; it tells you exactly what went wrong.

9. Copy the log to your data stick for later commit:

   ```bash
   cp /tmp/ahci-run-log.txt $STAGING/ahci-run-log.txt
   sync
   ```

10. The AHCI run is complete. Move on to Run 2.

---

# AT THE OPTIPLEX — Run 2 (NVMe scenario)

The flow mirrors Run 1, with these substitutions:
- Scenario disk goes to **Intel NVMe** (not Samsung SATA).
- Log sink goes to **Samsung SATA** (not Intel NVMe).
- Boot stick (Stick B) needs to be **reflashed with `nvme-scenario.iso`**.

## Step 18. Reflash Stick B for the NVMe scenario

You can do this either back at your desk or directly in Ubuntu Live (Ubuntu can flash USB sticks too — easier than walking back to your desk). Below is the Ubuntu Live flash path.

1. With Ubuntu Live still running (continuing from Step 17), insert **Stick B (currently AHCI SCENARIO)** into a free USB port.
2. Find the device:

   ```bash
   lsblk -o NAME,SIZE,MODEL,SERIAL | grep -i <Stick B's model>
   ```

   Note the device path (e.g., `/dev/sdd`).
3. Reflash it with the NVMe scenario ISO:

   ```bash
   sudo dd if=$STAGING/nvme/nvme-scenario.iso of=/dev/sdd bs=4M status=progress conv=fsync
   sync
   ```

   (Substitute the actual device path for `/dev/sdd`.)
4. **Re-label Stick B** with a fresh piece of tape: "NVMe SCENARIO" (overwrite the old label).

## Step 19. Provision the NVMe scenario disk onto the Intel NVMe

While still in Ubuntu Live:

```bash
sudo dd if=$STAGING/nvme/nvme-scenario-disk.raw of=/dev/nvme0n1 bs=4M status=progress conv=fsync
```

## Step 20. Provision the NVMe log-sink disk onto the Samsung SATA

```bash
sudo dd if=$STAGING/nvme/nvme-log-sink-disk.raw of=/dev/sda bs=4M status=progress conv=fsync
sync
```

## Step 21. Shut down, swap to Stick B

1. Power off Ubuntu (Step 12 procedure).
2. Remove Stick A (Ubuntu) and Stick C (Data).
3. Leave Stick B (now NVMe SCENARIO) inserted.

## Step 22. Boot the NVMe scenario

1. Power on, F12, select Stick B.
2. NVMe scenario kernel boots.
3. Watch the framebuffer phase indicator. Same phase sequence as AHCI:
   ```
   PROBE → LOG_SINK_READY → MOUNT_REGISTERED → READ → WRITE → READBACK → PASS
   ```
4. Photograph the result (PASS or FAIL).

## Step 23. Extract the NVMe log

1. Power off, remove Stick B, insert Stick A + Stick C, boot Ubuntu Live.
2. Open terminal, set `STAGING` path.
3. Run extraction against the Samsung SATA this time (because that's where the NVMe scenario's log sink lives):

   ```bash
   sudo $STAGING/neo-extract-log --device /dev/sda --output /tmp/nvme-run-log.txt
   tail -30 /tmp/nvme-run-log.txt
   ```

4. Copy to data stick:

   ```bash
   cp /tmp/nvme-run-log.txt $STAGING/nvme-run-log.txt
   sync
   ```

5. Power off Ubuntu, remove all sticks. **You're done at the hardware.**

---

# AFTER BOTH RUNS — back at your desk

## Step 24. Copy evidence back to the workspace

1. Plug Stick C (DATA) into your Windows machine.
2. Copy the log files and your photos to the workspace:

   ```powershell
   $stick = "F:\BUILD-5b-staging"   # adjust to your actual drive letter
   $dest = "E:\AI Automation\AltimitNeo\Plans\evidence-2026-05-XX-build-5b-bare-metal"
   New-Item -ItemType Directory -Path $dest -Force | Out-Null
   Copy-Item "$stick\ahci-run-log.txt" $dest -Force
   Copy-Item "$stick\nvme-run-log.txt" $dest -Force
   # Also copy your photos from your phone here as build5b-ahci-pass.jpg, build5b-nvme-pass.jpg
   ```

3. Let me know in chat that both runs are done; I'll file the BUILD-5b evidence artifact and the Phase 3 close PM-DISPOSITION.

---

# What success looks like

Both runs PASS. Both extracted logs end with the PASS sentinel. The framebuffer photos clearly show PASS in green for each scenario.

When you tell me both runs passed, I'll:
1. File `Plans/PHASE-3-BUILD-5b-EVIDENCE-2026-05-XX-bare-metal-optiplex.md` with the log contents and photo references.
2. File `Plans/PM-DISPOSITION-2026-05-XX-phase-3-formal-close.md` closing Phase 3 formally.
3. INDEX update transitioning BUILD-5b PROCEDURE row from `active` to `closed`.
4. Phase 4 PLAN drafting becomes immediately schedulable.

---

# Troubleshooting

## "The OptiPlex doesn't boot from the USB"

- Did you disable Secure Boot? Check BIOS (F2 during boot). Secure Boot must be **disabled** for NeoPlutarch ISOs to boot. This was already done during the boot-smoke test on 2026-05-22, but if the BIOS has been reset, redo it.
- Is the boot menu showing your USB? If not, the Rufus flash may have failed or used the wrong mode. Reflash with DD Image mode.
- Is the OptiPlex set to UEFI boot (not Legacy)? In BIOS → Boot Sequence, ensure UEFI is selected.

## "The framebuffer phase indicator shows FAIL"

- The log will tell you exactly what failed. Extract the log following Step 17 (or Step 23 for NVMe).
- Common failures:
  - **PROBE failure:** the SATA or NVMe device wasn't detected. Check that both SSDs are properly seated and recognized in BIOS.
  - **LOG_SINK_READY failure:** the log sink device's NEO_LOG_v1 magic wasn't found at offset 0x10000000. Most likely cause: the log-sink raw wasn't dd'd correctly. Redo Step 10 (or Step 20).
  - **READ/WRITE/READBACK failure:** the NeoFS image isn't readable or writes aren't sticking. Could indicate real hardware fault. Try a different SSD or rerun after a fresh dd.

## "lsblk doesn't show the SSDs"

- Boot into BIOS (F2 during power-on). Confirm both SSDs are listed under storage devices.
- If one or both are missing, reseat them physically (power off, open the OptiPlex, reseat the M.2 NVMe and check the SATA cable on the 2.5" SSD).

## "dd says 'No space left on device' or similar"

- Check the actual device size with `lsblk -o NAME,SIZE`. The .raw files are 128 MB and 288 MB — both SSDs are way larger than that. If you see "No space left," you're targeting the wrong device (probably a small USB stick).

## "The scenario kernel halts immediately with LOG_SINK_MISSING or similar"

- This is the strict-halt policy: the kernel refuses to run the scenario if the log sink isn't properly initialized. Means the log-sink .raw wasn't written to the right device. Redo provisioning (Steps 9–11 or 19–20) being careful about which device gets which file.

## "I see PASS but no log was captured"

- The extraction tool requires the log-sink device to still contain the NEO_LOG_v1 magic. If you accidentally ran something else against the log-sink device after the scenario, the log is gone. The PASS photo is still valid evidence.

## "I want to redo a scenario"

- Just re-dd the scenario disk and log-sink disk for that scenario (Steps 9–11 or 19–20), then boot the scenario USB again. The scenario disk is restored to the starting NeoFS image each time you dd it.

## "Something else went wrong"

- Photograph or note what you see.
- Power down the OptiPlex.
- Tell me what happened — describe the framebuffer text, any serial output if you happened to capture it, and the photo. I'll diagnose and propose next steps.

---

# Quick reference card (print this if helpful)

| Step | Command | Where you are |
|------|---------|---------------|
| Provision AHCI scenario | `sudo dd if=$STAGING/ahci/ahci-scenario-disk.raw of=/dev/sda bs=4M status=progress conv=fsync` | Ubuntu Live |
| Provision AHCI log sink | `sudo dd if=$STAGING/ahci/ahci-log-sink-disk.raw of=/dev/nvme0n1 bs=4M status=progress conv=fsync` | Ubuntu Live |
| Provision NVMe scenario | `sudo dd if=$STAGING/nvme/nvme-scenario-disk.raw of=/dev/nvme0n1 bs=4M status=progress conv=fsync` | Ubuntu Live |
| Provision NVMe log sink | `sudo dd if=$STAGING/nvme/nvme-log-sink-disk.raw of=/dev/sda bs=4M status=progress conv=fsync` | Ubuntu Live |
| Extract AHCI log | `sudo $STAGING/neo-extract-log --device /dev/nvme0n1 --output /tmp/ahci-run-log.txt` | Ubuntu Live |
| Extract NVMe log | `sudo $STAGING/neo-extract-log --device /dev/sda --output /tmp/nvme-run-log.txt` | Ubuntu Live |
| Sync (always after dd) | `sudo sync` | Ubuntu Live |
| Identify drives | `lsblk -o NAME,SIZE,MODEL,SERIAL` | Ubuntu Live |

**Phase sequence on framebuffer:**
`PROBE → LOG_SINK_READY → MOUNT_REGISTERED → READ → WRITE → READBACK → PASS`

**Drive assignments (memorize):**
- AHCI scenario: **scenario → SATA (sda)**, **log → NVMe (nvme0n1)**
- NVMe scenario: **scenario → NVMe (nvme0n1)**, **log → SATA (sda)**

The scenario always writes data to the SSD that **matches its name**. The log goes to the **other** SSD. That's the inversion that makes the test honest: the scenario can't accidentally pollute its own log.

---

**End of execution guide.**

*If something doesn't match this document, prefer the document and tell me what's different. We'll figure out where the slip is.*
