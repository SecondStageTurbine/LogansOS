// SPDX-License-Identifier: MPL-2.0
//! Pure logic for bringing xHCI up on real silicon (the Zenbook USB input
//! lane, feature `xhci-silicon`): the extended-capability walk (USB Legacy
//! Support + Supported Protocol), the choice of controller when a platform
//! exposes more than one xHCI, and the register values for the BIOS
//! handoff, PCI power state and hub slot context.
//!
//! Why a choice is needed: Intel client parts since Tiger Lake have two
//! xHCIs. The Type-C subsystem's (TCSS, device 0x0d on bus 0) carries only
//! the SuperSpeed lanes of the USB-C ports, while the USB 2 pins of those
//! ports, and the USB-A ports, are on the south xHCI (device 0x14). Bus 0 is
//! scanned in device order, so "the first xHCI" is the TCSS one, which can
//! never see a low/full-speed keyboard or mouse.
//!
//! Every function here is mounted by `tools/xhci-silicon-logic-tests` through
//! `#[path]`, so the host KATs run the exact code the kernel runs. Register
//! layouts are from the xHCI 1.2 specification (sections cited inline) and
//! the PCI Power Management 1.2 specification.

/// xHCI §7: extended capability IDs.
pub const XECP_ID_LEGACY: u8 = 1;
pub const XECP_ID_PROTOCOL: u8 = 2;

/// At most this many Supported Protocol capabilities are recorded; any more
/// are counted in [`ExtCaps::protocols_dropped`] and make the ranges unknown.
pub const MAX_PROTOCOLS: usize = 8;
/// Bound on the capability walk, so a looping or corrupt chain stops.
pub const MAX_EXT_CAPS: usize = 64;

/// One Supported Protocol capability (xHCI §7.2): the revision it speaks and
/// the root ports (1-based, inclusive range `first_port .. first_port+count`)
/// it covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProtocolRange {
    pub major: u8,
    pub minor: u8,
    pub first_port: u8,
    pub count: u8,
}

impl ProtocolRange {
    pub fn contains(&self, port: u8) -> bool {
        self.count != 0
            && port >= self.first_port
            && u16::from(port) < u16::from(self.first_port) + u16::from(self.count)
    }
}

/// Why the capability walk stopped early, if it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkStop {
    /// The next pointer left the mapped register window at this offset.
    LeftWindow(u64),
    /// More than [`MAX_EXT_CAPS`] capabilities: treated as a loop.
    TooMany,
}

/// The extended capabilities this lane uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtCaps {
    /// Byte offset of USBLEGSUP from the capability base, if present.
    pub legacy: Option<u64>,
    pub protocols: [ProtocolRange; MAX_PROTOCOLS],
    pub protocol_count: usize,
    /// Supported Protocol capabilities found past [`MAX_PROTOCOLS`] and not
    /// recorded. Non-zero: the recorded ranges are incomplete.
    pub protocols_dropped: u16,
    pub stop: Option<WalkStop>,
}

impl ExtCaps {
    pub fn protocols(&self) -> &[ProtocolRange] {
        &self.protocols[..self.protocol_count]
    }

    /// The major USB revision of root port `port`, if a protocol covers it.
    pub fn port_major(&self, port: u8) -> Option<u8> {
        self.protocols()
            .iter()
            .find(|range| range.contains(port))
            .map(|range| range.major)
    }

    /// Root ports speaking USB 2 (the only ports a low/full-speed HID device
    /// can appear on).
    pub fn usb2_ports(&self) -> u16 {
        self.protocols()
            .iter()
            .filter(|range| range.major == 2)
            .map(|range| u16::from(range.count))
            .sum()
    }
}

/// Walk the extended capability list (xHCI §7), reading 32-bit registers at
/// byte offsets from the capability base through `read`. `hcc1` is
/// HCCPARAMS1; its bits 31:16 are xECP in dwords. `window` is the mapped
/// size: a pointer at or past it stops the walk without a read.
pub fn walk_ext_caps(read: &mut impl FnMut(u64) -> u32, hcc1: u32, window: u64) -> ExtCaps {
    let mut caps = ExtCaps::default();
    let mut offset = u64::from(hcc1 >> 16) << 2;
    let mut seen = 0usize;
    while offset != 0 {
        if offset.saturating_add(12) > window {
            caps.stop = Some(WalkStop::LeftWindow(offset));
            break;
        }
        if seen == MAX_EXT_CAPS {
            caps.stop = Some(WalkStop::TooMany);
            break;
        }
        seen += 1;
        let header = read(offset);
        match (header & 0xFF) as u8 {
            XECP_ID_LEGACY if caps.legacy.is_none() => caps.legacy = Some(offset),
            XECP_ID_PROTOCOL if caps.protocol_count < MAX_PROTOCOLS => {
                // DWORD0: [23:16] minor revision, [31:24] major revision.
                // DWORD2: [7:0] compatible port offset, [15:8] port count.
                let ports = read(offset + 8);
                caps.protocols[caps.protocol_count] = ProtocolRange {
                    major: (header >> 24) as u8,
                    minor: (header >> 16) as u8,
                    first_port: ports as u8,
                    count: (ports >> 8) as u8,
                };
                caps.protocol_count += 1;
            }
            XECP_ID_PROTOCOL => caps.protocols_dropped += 1,
            _ => {}
        }
        let next = u64::from((header >> 8) & 0xFF);
        if next == 0 {
            break;
        }
        offset += next << 2;
    }
    caps
}

/// What the census saw on one xHCI before any reset: whether its registers
/// read back (a powered-off controller reads all-ones), whether its Supported
/// Protocol ranges were read completely, how many root ports speak USB 2,
/// and how many ports (all, and USB 2 devices) had a device connected
/// (PORTSC.CCS) as firmware left them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Candidate {
    pub readable: bool,
    /// The capability walk finished and found at least one Supported
    /// Protocol range. False: `usb2_ports` is a lower bound only.
    pub protocols_known: bool,
    pub usb2_ports: u16,
    pub usb2_connected: u16,
    pub connected: u16,
}

/// Whether a walk's protocol ranges are complete enough to say which ports
/// speak USB 2: the walk finished, found a range, and recorded every range.
pub fn protocols_known(caps: &ExtCaps) -> bool {
    caps.stop.is_none() && caps.protocol_count > 0 && caps.protocols_dropped == 0
}

/// Whether a connected root port holds a USB 2 device (low, full or high
/// speed). The port's Supported Protocol range decides when it is known; a
/// port no range covers falls back to PORTSC Port Speed (bits 13:10), whose
/// default IDs are 1 = full, 2 = low, 3 = high, 4+ = SuperSpeed (xHCI
/// §7.2.1; the defaults apply when no protocol capability redefines them).
pub fn port_holds_usb2_device(port_major: Option<u8>, speed_id: u8) -> bool {
    match port_major {
        Some(major) => major == 2,
        None => (1..=3).contains(&speed_id),
    }
}

/// Why a controller was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceReason {
    /// The platform has one xHCI: it is used, as before this lane.
    Only,
    /// The first controller with a device on a USB 2 root port.
    Usb2Connected,
    /// No USB 2 device anywhere: the controller with the most USB 2 ports,
    /// where a keyboard or mouse plugged in later will appear.
    MostUsb2Ports,
    /// No USB 2 device, no known USB 2 port, and at least one controller
    /// whose protocol ranges could not be read: the first such controller.
    /// A controller known to be SuperSpeed-only is never chosen over it.
    /// Ambiguous, and named so on screen.
    ProtocolsUnknown,
    /// Every readable controller's ranges are known and none has a USB 2
    /// port: the first readable controller (no keyboard can appear anywhere).
    FirstReadable,
    /// Nothing reads back: the first controller, as before this lane.
    NoneReadable,
}

impl ChoiceReason {
    pub fn name(self) -> &'static str {
        match self {
            Self::Only => "only-controller",
            Self::Usb2Connected => "usb2-device-connected",
            Self::MostUsb2Ports => "most-usb2-ports",
            Self::ProtocolsUnknown => "AMBIGUOUS-protocols-unknown",
            Self::FirstReadable => "no-usb2-port-anywhere",
            Self::NoneReadable => "none-readable-first",
        }
    }
}

/// Pick the xHCI to bring up. `None` only for an empty census. Ties go to the
/// earlier controller in PCI scan order.
pub fn choose(candidates: &[Candidate]) -> Option<(usize, ChoiceReason)> {
    match candidates.len() {
        0 => return None,
        1 => return Some((0, ChoiceReason::Only)),
        _ => {}
    }
    let readable = || candidates.iter().enumerate().filter(|(_, c)| c.readable);
    if let Some((index, _)) = readable().find(|(_, c)| c.usb2_connected > 0) {
        return Some((index, ChoiceReason::Usb2Connected));
    }
    let mut best: Option<(usize, u16)> = None;
    for (index, candidate) in readable() {
        if candidate.usb2_ports > 0 && best.is_none_or(|(_, ports)| candidate.usb2_ports > ports) {
            best = Some((index, candidate.usb2_ports));
        }
    }
    if let Some((index, _)) = best {
        return Some((index, ChoiceReason::MostUsb2Ports));
    }
    if let Some((index, _)) = readable().find(|(_, c)| !c.protocols_known) {
        return Some((index, ChoiceReason::ProtocolsUnknown));
    }
    if let Some((index, _)) = readable().next() {
        return Some((index, ChoiceReason::FirstReadable));
    }
    Some((0, ChoiceReason::NoneReadable))
}

// --- USB Legacy Support (xHCI §7.1) -----------------------------------------------

/// USBLEGSUP: HC BIOS Owned Semaphore.
pub const LEGSUP_BIOS_OWNED: u32 = 1 << 16;
/// USBLEGSUP: HC OS Owned Semaphore.
pub const LEGSUP_OS_OWNED: u32 = 1 << 24;

/// The USBLEGSUP write that requests ownership.
pub fn legsup_request(value: u32) -> u32 {
    value | LEGSUP_OS_OWNED
}

/// The USBLEGSUP write that takes ownership when firmware did not release it
/// in time (a firmware bug; the OS proceeds rather than leave the controller
/// shared with SMM).
pub fn legsup_force(value: u32) -> u32 {
    (value | LEGSUP_OS_OWNED) & !LEGSUP_BIOS_OWNED
}

/// USBLEGCTLSTS (USBLEGSUP + 4), xHCI §7.1.2. The SMI enables (RW): USB SMI
/// (0), SMI on Host System Error (4), on OS Ownership (13), on PCI Command
/// (14), on BAR (15).
pub const LEGCTL_SMI_ENABLES: u32 = (1 << 0) | (1 << 4) | (1 << 13) | (1 << 14) | (1 << 15);
/// The RsvdP fields (3:1, 12:5, 19:17), preserved on write.
pub const LEGCTL_RSVDP: u32 = (0x7 << 1) | (0xFF << 5) | (0x7 << 17);
/// The RW1C SMI status bits (31:29: on BAR, on PCI Command, on OS Ownership
/// Change), cleared by writing 1.
pub const LEGCTL_SMI_EVENTS: u32 = 0x7 << 29;

/// The USBLEGCTLSTS write that turns every firmware SMI source off and clears
/// the latched SMI events: preserved reserved bits, zero enables, RsvdZ and
/// RO bits written 0, events written 1.
pub fn legctl_disable_smis(value: u32) -> u32 {
    (value & LEGCTL_RSVDP) | LEGCTL_SMI_EVENTS
}

// --- PCI Power Management (PCI PM 1.2 §3.2.4) ---------------------------------------

/// PMCSR PowerState field (bits 1:0): 0 = D0 … 3 = D3hot.
pub fn pmcsr_state(pmcsr: u16) -> u8 {
    (pmcsr & 0x3) as u8
}

/// The PMCSR write that moves the function to D0. PME_Status (bit 15) is
/// RW1C and is written 0 so a pending wake is not cleared as a side effect.
pub fn pmcsr_to_d0(pmcsr: u16) -> u16 {
    pmcsr & !0x3 & !(1 << 15)
}

// --- Hub slot context (xHCI §6.2.2) -----------------------------------------------

/// Slot Context DWORD0 Hub bit and Multi-TT bit.
pub const SLOT_HUB: u32 = 1 << 26;
pub const SLOT_MTT: u32 = 1 << 25;

/// The input Slot Context dwords 0..=2 that mark a slot as a hub, built from
/// the slot's current OUTPUT context dwords (so Route String, Speed, Context
/// Entries, Root Hub Port Number and the TT fields the controller already
/// holds are kept). Sets Hub, Number of Ports (DWORD1 31:24), and for a
/// high-speed hub the TT Think Time (DWORD2 17:16) from the hub descriptor's
/// wHubCharacteristics bits 6:5 (USB 2.0 §11.23.2.1, same encoding: 0 = 8
/// FS bit times … 3 = 32). MTT is set only for a multi-TT hub; a full-speed
/// hub gets MTT and TTT cleared (xHCI §6.2.2: both 0 for a non-HS hub).
pub fn hub_slot_context(
    out: [u32; 3],
    ports: u8,
    high_speed: bool,
    multi_tt: bool,
    hub_characteristics: u16,
) -> [u32; 3] {
    let mut dword0 = out[0] | SLOT_HUB;
    if high_speed && multi_tt {
        dword0 |= SLOT_MTT;
    } else {
        dword0 &= !SLOT_MTT;
    }
    let dword1 = (out[1] & 0x00FF_FFFF) | (u32::from(ports) << 24);
    let mut dword2 = out[2] & !(0x3 << 16);
    if high_speed {
        dword2 |= u32::from((hub_characteristics >> 5) & 0x3) << 16;
    }
    [dword0, dword1, dword2]
}

/// Whether a hub should be switched to multiple-TT operation: a high-speed
/// hub whose device descriptor's bDeviceProtocol is 2 (USB 2.0 §11.23.1). It
/// runs single-TT until SET_INTERFACE selects alternate setting 1, so the slot
/// context's MTT is set only when that request succeeds.
pub fn wants_multi_tt(high_speed: bool, device_protocol: u8) -> bool {
    high_speed && device_protocol == 2
}

/// Whether hub slot setup needs a Configure Endpoint command. xHCI 1.0 and
/// later evaluate the Hub, MTT, Number of Ports and TTT fields only on
/// Configure Endpoint; Evaluate Context changes only Max Exit Latency,
/// Interrupter Target and EP0's packet size (§4.6.7), so a hub marked that
/// way is not a hub to the controller. Version 0.95 and earlier took them on
/// Evaluate Context.
pub fn hub_needs_configure_endpoint(hci_version: u16) -> bool {
    hci_version > 0x0095
}

// --- Time without a calibrated clock -----------------------------------------------

/// TSC cycles that last at least `ms` milliseconds on any TSC up to 10 GHz.
/// USB bring-up runs before the timer is calibrated, so the budget is sized
/// for the fastest plausible TSC: on a 2 GHz part it waits 5x longer, never
/// shorter.
pub const TSC_HZ_BOUND: u64 = 10_000_000_000;

pub fn tsc_cycles_for_ms(ms: u64) -> u64 {
    ms.saturating_mul(TSC_HZ_BOUND / 1000)
}

// --- Port service decisions (feature `usb-hotplug`) ---------------------------------

/// Enumeration attempts per connection before a port is left alone until it
/// disconnects.
pub const MAX_ATTEMPTS: u8 = 3;

/// One root or hub port as the port service tracks it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PortTrack {
    pub attached: bool,
    pub attempts: u8,
}

/// What the port service does with one port this pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortAction {
    Nothing,
    /// A device is connected and not yet enumerated: try now.
    Enumerate,
    /// The enumerated device is gone: forget it so a re-plug enumerates.
    Detached,
    /// Connected, but every attempt failed: wait for a disconnect.
    GaveUp,
}

pub fn port_action(track: PortTrack, connected: bool) -> PortAction {
    match (connected, track.attached) {
        (true, true) => PortAction::Nothing,
        (true, false) if track.attempts < MAX_ATTEMPTS => PortAction::Enumerate,
        (true, false) => PortAction::GaveUp,
        (false, true) => PortAction::Detached,
        (false, false) => PortAction::Nothing,
    }
}

/// The tracking state after `action`, given whether an enumeration attempt
/// succeeded. A disconnect resets the attempt count.
pub fn port_after(track: PortTrack, connected: bool, action: PortAction, ok: bool) -> PortTrack {
    match action {
        PortAction::Enumerate if ok => PortTrack {
            attached: true,
            attempts: 0,
        },
        PortAction::Enumerate => PortTrack {
            attached: false,
            attempts: track.attempts.saturating_add(1),
        },
        PortAction::Detached => PortTrack::default(),
        PortAction::Nothing if !connected => PortTrack::default(),
        _ => track,
    }
}

/// Devices the stack finished enumerating, by (root port, route string) —
/// how the port service knows a hub's child is attached. A hub port's
/// PORT_ENABLE is not that: it is set by the port reset, before Enable Slot
/// and Address Device, so a child whose enumeration failed after the reset
/// would look attached and never be retried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressedSet {
    entries: [(u32, u32); ADDRESSED_MAX],
    len: usize,
}

/// Bound on [`AddressedSet`]: the controller enables at most 16 slots here,
/// and hubs count.
pub const ADDRESSED_MAX: usize = 64;

impl AddressedSet {
    pub const fn new() -> Self {
        Self {
            entries: [(0, 0); ADDRESSED_MAX],
            len: 0,
        }
    }

    pub fn contains(&self, root_port: u32, route: u32) -> bool {
        self.entries[..self.len].contains(&(root_port, route))
    }

    /// Record a finished enumeration. Returns false when the set is full.
    pub fn insert(&mut self, root_port: u32, route: u32) -> bool {
        if self.contains(root_port, route) {
            return true;
        }
        if self.len == ADDRESSED_MAX {
            return false;
        }
        self.entries[self.len] = (root_port, route);
        self.len += 1;
        true
    }

    /// Forget one device and everything below it on the same root port
    /// (its route string is a prefix: the lower tiers of `route`).
    pub fn remove_subtree(&mut self, root_port: u32, route: u32) {
        let mut index = 0;
        while index < self.len {
            let (root, entry) = self.entries[index];
            if root == root_port && route_is_under(entry, route) {
                self.len -= 1;
                self.entries[index] = self.entries[self.len];
            } else {
                index += 1;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for AddressedSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether `route` is `parent` or below it. A route string holds one hub
/// port per 4-bit tier, lowest tier first (xHCI §8.9); `parent`'s non-zero
/// tiers must match and the tiers after it are free.
pub fn route_is_under(route: u32, parent: u32) -> bool {
    let mut tiers = 0;
    while tiers < 5 && (parent >> (tiers * 4)) & 0xF != 0 {
        tiers += 1;
    }
    let mask = if tiers == 0 {
        0
    } else {
        (1u32 << (tiers * 4)) - 1
    };
    route & mask == parent & mask
}
