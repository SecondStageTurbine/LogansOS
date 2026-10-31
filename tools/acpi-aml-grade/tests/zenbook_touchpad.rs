// SPDX-License-Identifier: MPL-2.0
//! OT-86 Fix 3 (L4) goal: the Zenbook S14 touchpad described through AML alone.
//!
//! The patched interpreter loads the Zenbook's own tables
//! (`fixtures/acpi/zenbook-s14-ux5406aa-303`), evaluates `ETPD._CRS`, the
//! HID-over-I2C `_DSM` and `I2C5._ADR`, and decodes `_CRS` with its own
//! resource decoder. Every value must equal the rung-1 platform quirk the
//! kernel uses today, `ZENBOOK_S14_QUIRK` in
//! `kernel/src/drivers/i2c_hid/quirk_logic.rs`, mounted here read-only by
//! `#[path]` (as `tools/i2c-hid-logic-tests` does), so the kernel's own
//! constants are the cross-check and the kernel tree is untouched.

use acpi_aml_grade::corpus::load_set;
use acpi_aml_grade::grade_acpi6::fix1;
use acpi_aml_grade::probe::HID_I2C_UUID;
use acpi_fix1::aml::object::{Object, WrappedObject};
use acpi_fix1::aml::resource::{
    InterruptPolarity, InterruptTrigger, Resource, resource_descriptor_list,
};
use std::path::PathBuf;

#[allow(dead_code)]
#[path = "../../../kernel/src/drivers/i2c_hid/quirk_logic.rs"]
mod quirk_logic;

const ETPD: &str = "\\_SB_.PC00.I2C5.ETPD";

fn dsm(rev: u64, func: u64) -> Vec<WrappedObject> {
    vec![
        Object::Buffer(HID_I2C_UUID.to_vec()).wrap(),
        Object::Integer(rev).wrap(),
        Object::Integer(func).wrap(),
        Object::Package(vec![]).wrap(),
    ]
}

#[test]
fn zenbook_touchpad_through_aml_matches_the_kernel_quirk() {
    let q = &quirk_logic::ZENBOOK_S14_QUIRK;
    assert_eq!(
        quirk_logic::HID_I2C_DSM_UUID,
        HID_I2C_UUID,
        "the two UUID copies agree"
    );

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/acpi/zenbook-s14-ux5406aa-303");
    if !dir.exists() {
        // Real-machine sets are kept in the private repo only.
        eprintln!("SKIP: real-machine fixture set {} absent", dir.display());
        return;
    }
    let set = load_set(&dir).unwrap();
    let crs_path = format!("{ETPD}._CRS");
    let dsm_path = format!("{ETPD}._DSM");
    let r = fix1::evaluate_on_set(
        &set,
        vec![
            (crs_path.as_str(), vec![]),
            (dsm_path.as_str(), dsm(1, 1)),
            (dsm_path.as_str(), dsm(1, 0)),
            ("\\_SB_.PC00.I2C5._ADR", vec![]),
        ],
    )
    .unwrap();
    let [crs, desc_reg, functions, adr] = &r[..] else {
        panic!("{r:?}")
    };

    // _CRS: the template bytes the quirk matches on, then the decode.
    let crs = crs.as_ref().unwrap();
    let Object::Buffer(ref bytes) = **crs else {
        panic!("_CRS is not a Buffer: {crs:?}")
    };
    assert_eq!(bytes.as_slice(), q.crs_template);
    let resources = resource_descriptor_list(crs.clone()).unwrap();
    let [Resource::I2cSerialBus(i2c), Resource::Irq(irq)] = &resources[..] else {
        panic!("{resources:?}")
    };
    assert_eq!(i2c.slave_address, u16::from(q.slave_address));
    assert_eq!(i2c.connection_speed, q.bus_speed_hz);
    assert!(!i2c.ten_bit_addressing, "7-bit addressing");
    assert!(!i2c.device_initiated, "controller-initiated");
    assert!(i2c.is_consumer);
    assert_eq!(i2c.resource_source, "\\_SB.PC00.I2C5");
    assert!(i2c.vendor_data.is_empty());
    assert_eq!(irq.irq, q.gsi);
    assert_eq!(irq.trigger == InterruptTrigger::Level, q.level);
    assert_eq!(irq.polarity == InterruptPolarity::ActiveLow, q.active_low);
    assert_eq!(irq.is_wake_capable, q.wake);
    assert!(irq.is_consumer);

    // _DSM function 1: the HID descriptor register; function 0: the supported-functions mask.
    let desc_reg = desc_reg.as_ref().unwrap();
    assert_eq!(
        desc_reg.as_integer().unwrap(),
        u64::from(q.hid_desc_register)
    );
    let functions = functions.as_ref().unwrap();
    assert_eq!(functions.as_buffer().unwrap(), &[0x03]);

    // The controller the resource source names: I2C5's _ADR is the quirk's PCI device/function.
    let adr = adr.as_ref().unwrap().as_integer().unwrap();
    assert_eq!(
        adr,
        (u64::from(q.pci_dev) << 16) | u64::from(q.pci_function)
    );
}
