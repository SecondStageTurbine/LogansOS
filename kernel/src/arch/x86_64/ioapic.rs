// SPDX-License-Identifier: MPL-2.0
//! Minimal I/O APIC routing for one ISA interrupt (the i8042 lane, IRQ1).
//!
//! Scope is one redirection entry: the kernel's other interrupts are MSI or
//! IPIs, and the tick keeps its PIT/8259 or LAPIC-timer route. Nothing else
//! of the I/O APIC is touched: the other pins keep whatever firmware left,
//! which on every platform we boot is masked.
//!
//! The ISA IRQ's GSI, polarity and trigger come from the MADT (an interrupt
//! source override, else identity, edge, active high). The entry is fixed
//! delivery, physical destination, to the BSP's local APIC. The 8259's own
//! IRQ line for the same source stays masked (`pic::init` masks all), so the
//! interrupt is never delivered twice.
//!
//! OT-90 P3b adds [`MaskPlan`]: K5 masks every entry before power-off.
// A `power-stop-cpus` kernel without an input lane uses the mask plan alone.
#![cfg_attr(
    not(any(feature = "ps2-keyboard", feature = "i2c-hid-touchpad")),
    allow(dead_code)
)]

use core::sync::atomic::{AtomicU32, Ordering};

use x86_64::PhysAddr;

use crate::acpi::madt_logic::{self, Madt};
use crate::arch::x86_64::ioapic_logic as logic;

/// Where an ISA IRQ was routed.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(feature = "ps2-keyboard"), allow(dead_code))] // the touchpad lane uses `route_gsi`
pub struct Routed {
    pub gsi: u32,
    pub pin: u32,
    pub active_low: bool,
    pub level: bool,
}

/// Why it was not.
#[derive(Clone, Copy, Debug)]
pub enum RouteError {
    /// No MADT, or no I/O APIC entry in it.
    NoIoApic,
    /// The GSI is outside every I/O APIC's pins.
    NoPin(u32),
    /// The BSP's APIC ID does not fit the 8-bit physical destination (an
    /// x2APIC ID above 255 needs interrupt remapping, which we do not set up).
    DestinationTooWide(u32),
}

/// The routed entry's I/O APIC address and pin (test diagnostics).
#[cfg_attr(not(feature = "ps2-keyboard"), allow(dead_code))] // the touchpad lane uses `route_gsi`
static ROUTED_ADDRESS: AtomicU32 = AtomicU32::new(0);
#[cfg_attr(not(feature = "ps2-keyboard"), allow(dead_code))] // the touchpad lane uses `route_gsi`
static ROUTED_PIN: AtomicU32 = AtomicU32::new(0);

/// Test diagnostic: the routed entry's low dword (delivery status is bit 12,
/// remote IRR bit 14, mask bit 16), if one was routed.
#[cfg(feature = "neo-test-ps2-keyboard")]
pub fn routed_low_for_test() -> Option<u32> {
    let address = ROUTED_ADDRESS.load(Ordering::Acquire);
    if address == 0 {
        return None;
    }
    let pin = ROUTED_PIN.load(Ordering::Acquire);
    // SAFETY: mapped when the entry was routed; a read has no side effect.
    Some(unsafe { read(reg_base(address), logic::redir_low_index(pin)) })
}

fn reg_base(address: u32) -> *mut u32 {
    let phys = PhysAddr::new(address as u64);
    crate::mm::map_kernel_mmio_page(phys);
    crate::mm::phys_to_virt(phys).as_mut_ptr::<u32>()
}

/// SAFETY: `base` is a mapped I/O APIC register block. Callers serialize
/// (boot, one CPU).
unsafe fn read(base: *mut u32, index: u32) -> u32 {
    unsafe {
        core::ptr::write_volatile(base.byte_add(logic::IOREGSEL), index);
        core::ptr::read_volatile(base.byte_add(logic::IOWIN))
    }
}

/// SAFETY: as for [`read`].
unsafe fn write(base: *mut u32, index: u32, value: u32) {
    unsafe {
        core::ptr::write_volatile(base.byte_add(logic::IOREGSEL), index);
        core::ptr::write_volatile(base.byte_add(logic::IOWIN), value);
    }
}

/// OT-90 P3b, K5: every I/O APIC's register block and entry count, read and
/// mapped before K4 (after it, nothing may map a page: a mapping can need a
/// TLB shootdown the parked CPUs would never acknowledge).
#[cfg(feature = "power-stop-cpus")]
#[derive(Clone, Copy, Default)]
pub struct MaskPlan {
    blocks: [(usize, u32); madt_logic::MAX_IOAPICS],
    count: usize,
}

#[cfg(feature = "power-stop-cpus")]
impl MaskPlan {
    pub fn new(madt: &Madt) -> Self {
        let mut plan = Self::default();
        for io in madt.ioapics.iter().take(madt.ioapic_count) {
            let base = reg_base(io.address);
            // SAFETY: just mapped; the version register has no side effect.
            let entries = logic::entries_from_version(unsafe { read(base, logic::REG_VERSION) });
            plan.blocks[plan.count] = (base as usize, entries);
            plan.count += 1;
        }
        plan
    }

    /// Mask every redirection entry (bit 16 of each low dword), keeping the
    /// rest of it. After K4: no other CPU runs, so the IOREGSEL/IOWIN pairs
    /// can't interleave. Returns how many entries were masked.
    pub fn mask_all(&self) -> u32 {
        let mut masked = 0;
        for &(base, entries) in &self.blocks[..self.count] {
            let base = base as *mut u32;
            for pin in 0..entries {
                let index = logic::redir_low_index(pin);
                // SAFETY: a register block `new` mapped; one CPU runs.
                unsafe { write(base, index, read(base, index) | (1 << 16)) };
                masked += 1;
            }
        }
        masked
    }
}

/// Route ISA `irq` to `vector` on the BSP, unmasked. Boot-time, BSP only.
#[cfg_attr(not(feature = "ps2-keyboard"), allow(dead_code))] // the touchpad lane uses `route_gsi`
pub fn route_isa_irq(madt: &Madt, irq: u8, vector: u8) -> Result<Routed, RouteError> {
    if madt.ioapic_count == 0 {
        return Err(RouteError::NoIoApic);
    }
    let route = madt_logic::route_isa_irq(madt, irq);
    let mut entries = [0u32; madt_logic::MAX_IOAPICS];
    for (i, slot) in entries.iter_mut().enumerate().take(madt.ioapic_count) {
        let base = reg_base(madt.ioapics[i].address);
        // SAFETY: just mapped; boot-time and single-threaded here.
        *slot = logic::entries_from_version(unsafe { read(base, logic::REG_VERSION) });
    }
    let (index, pin) = madt_logic::ioapic_for_gsi(madt, &entries[..madt.ioapic_count], route.gsi)
        .ok_or(RouteError::NoPin(route.gsi))?;
    let dest = crate::arch::x86_64::apic::lapic_id_of(0);
    if dest > 0xff {
        return Err(RouteError::DestinationTooWide(dest));
    }
    let (low, high) = logic::redirection(vector, dest as u8, route.active_low, route.level, false);
    let base = reg_base(madt.ioapics[index].address);
    // SAFETY: as above. Masked first, the destination, then the unmasked
    // low half, so the pin never fires half-programmed.
    unsafe {
        write(base, logic::redir_low_index(pin), 1 << 16);
        write(base, logic::redir_high_index(pin), high);
        write(base, logic::redir_low_index(pin), low);
    }
    ROUTED_PIN.store(pin, Ordering::Release);
    ROUTED_ADDRESS.store(madt.ioapics[index].address, Ordering::Release);
    Ok(Routed {
        gsi: route.gsi,
        pin,
        active_low: route.active_low,
        level: route.level,
    })
}

/// A routed GSI's redirection entry, for masking and unmasking it after
/// boot (the I2C-HID touchpad lane: a level-triggered line the ISR masks
/// and the service thread unmasks).
///
/// `IOREGSEL`/`IOWIN` is a two-write pair, so two writers interleaving on
/// it corrupt each other. This handle stays safe without a lock because of
/// the protocol its one user follows: the ISR writes only while the pin is
/// unmasked (that is the only time it can fire) and the thread writes only
/// while it is masked, with interrupts off on its CPU, and each write
/// selects the same index. Nothing else touches the I/O APIC after boot.
#[cfg(feature = "i2c-hid-touchpad")]
#[derive(Clone, Copy, Debug)]
pub struct GsiPin {
    address: u32,
    pub pin: u32,
    pub gsi: u32,
    low: u32,
}

#[cfg(feature = "i2c-hid-touchpad")]
impl GsiPin {
    fn base(&self) -> *mut u32 {
        // Mapped by `route_gsi`.
        crate::mm::phys_to_virt(PhysAddr::new(self.address as u64)).as_mut_ptr::<u32>()
    }

    /// Mask (`true`) or unmask the entry: one IOREGSEL/IOWIN pair.
    pub fn set_masked(&self, masked: bool) {
        let low = if masked {
            self.low | (1 << 16)
        } else {
            self.low
        };
        // SAFETY: the routed entry's register block, mapped by `route_gsi`;
        // see the type docs for why the pair cannot interleave.
        unsafe { write(self.base(), logic::redir_low_index(self.pin), low) }
    }

    /// The entry's low dword (mask bit 16, remote IRR bit 14, delivery
    /// status bit 12). Callers read it only while the pin is masked.
    pub fn read_low(&self) -> u32 {
        // SAFETY: as for `set_masked`.
        unsafe { read(self.base(), logic::redir_low_index(self.pin)) }
    }
}

/// Route `gsi` to `vector` on the BSP with the given polarity and trigger,
/// left MASKED. Boot-time, BSP only.
#[cfg(feature = "i2c-hid-touchpad")]
pub fn route_gsi(
    madt: &Madt,
    gsi: u32,
    vector: u8,
    active_low: bool,
    level: bool,
) -> Result<GsiPin, RouteError> {
    if madt.ioapic_count == 0 {
        return Err(RouteError::NoIoApic);
    }
    let mut entries = [0u32; madt_logic::MAX_IOAPICS];
    for (i, slot) in entries.iter_mut().enumerate().take(madt.ioapic_count) {
        let base = reg_base(madt.ioapics[i].address);
        // SAFETY: just mapped; boot-time and single-threaded here.
        *slot = logic::entries_from_version(unsafe { read(base, logic::REG_VERSION) });
    }
    let (index, pin) = madt_logic::ioapic_for_gsi(madt, &entries[..madt.ioapic_count], gsi)
        .ok_or(RouteError::NoPin(gsi))?;
    let dest = crate::arch::x86_64::apic::lapic_id_of(0);
    if dest > 0xff {
        return Err(RouteError::DestinationTooWide(dest));
    }
    let (low, high) = logic::redirection(vector, dest as u8, active_low, level, false);
    let address = madt.ioapics[index].address;
    let base = reg_base(address);
    // SAFETY: as above. Masked first, then the destination, then the low
    // half, still masked.
    unsafe {
        write(base, logic::redir_low_index(pin), 1 << 16);
        write(base, logic::redir_high_index(pin), high);
        write(base, logic::redir_low_index(pin), low | (1 << 16));
    }
    Ok(GsiPin {
        address,
        pin,
        gsi,
        low,
    })
}
