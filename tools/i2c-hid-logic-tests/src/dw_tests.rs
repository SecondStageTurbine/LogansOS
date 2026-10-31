// SPDX-License-Identifier: MPL-2.0
//! The DesignWare master and the LPSS resets, against the register model.

use crate::dw_logic::*;
use crate::hid_tests::HID_DESC;
use crate::model::{Event, Model};

/// Model-clock budgets: generous enough for any legal transfer here, small
/// enough that a timeout test finishes fast.
const BUDGET: Budget = Budget {
    handshake: 200,
    transfer_base: 2_000,
    per_byte: 50,
};

fn configured(model: &mut Model) -> Controller {
    configure(model, LPSS_I2C_CLOCK_HZ, &BUDGET).expect("configure")
}

#[test]
fn scl_counts_from_the_bus_timing() {
    // 100 MHz, fast mode: high = round(100e6 * (600 + 208) ns) = round(80.8)
    // = 81 cycles, less 3 = 78; low = round(100e6 * (1300 + 208) ns) =
    // round(150.8) = 151, less 1 = 150.
    assert_eq!(
        fast_mode_counts(100_000_000),
        SclCounts {
            hcnt: 78,
            lcnt: 150
        }
    );
    // Standard mode: round(100e6 * 4208 ns) = round(420.8) = 421 - 3 = 418;
    // round(100e6 * 4908 ns) = round(490.8) = 491 - 1 = 490.
    assert_eq!(
        standard_mode_counts(100_000_000),
        SclCounts {
            hcnt: 418,
            lcnt: 490
        }
    );
    // 120 MHz (Sunrise Point's clock), fast: round(96.96) = 97 - 3 = 94;
    // round(180.96) = 181 - 1 = 180.
    assert_eq!(
        fast_mode_counts(120_000_000),
        SclCounts {
            hcnt: 94,
            lcnt: 180
        }
    );
    // The 42 ns SDA hold at 100 MHz: round(4.2) = 4 cycles.
    assert_eq!(cycles(100_000_000, SDA_HOLD_NS), 4);
}

#[test]
fn fifo_depths_and_lpss_values() {
    // IC_COMP_PARAM_1 bits 23:16 = TX depth - 1, 15:8 = RX depth - 1.
    assert_eq!(fifo_depths(0x003F_3F00), (64, 64));
    assert_eq!(fifo_depths(0x0007_0F00), (8, 16));
    // The resets write: function bits 1:0 set, iDMA bit 2 clear.
    assert_eq!(lpss_resets_value(), 0b011);
    assert!(lpss_dma_in_reset(0b011));
    assert!(lpss_dma_in_reset(0));
    assert!(!lpss_dma_in_reset(0b111));
    assert!(!lpss_dma_in_reset(0b100));
    assert_eq!(LPSS_RESETS, 0x204);
    assert_eq!(LPSS_CLOCK_PARAMS, 0x200);
    assert_eq!(LPSS_CAPS, 0x2FC);
    assert!(BAR0_MAP_LEN as usize > LPSS_CAPS + 3);
}

#[test]
fn configure_programs_fast_master_polled_with_dma_off() {
    let mut m = Model::new(0x15);
    m.tx_depth = 32;
    m.rx_depth = 16;
    let ctl = configured(&mut m);
    assert!(m.violations.is_empty(), "{:?}", m.violations);
    assert_eq!(ctl.tx_depth, 32);
    assert_eq!(ctl.rx_depth, 16);
    assert_eq!(ctl.version, 0x3230_322A);
    // IC_CON: master (bit 0) | fast (2 << 1) | restart (bit 5) | slave
    // disable (bit 6) = 0x01 | 0x04 | 0x20 | 0x40 = 0x65.
    assert_eq!(m.reg(IC_CON), 0x65);
    assert_eq!(m.reg(IC_FS_SCL_HCNT), 78);
    assert_eq!(m.reg(IC_FS_SCL_LCNT), 150);
    assert_eq!(m.reg(IC_SS_SCL_HCNT), 418);
    assert_eq!(m.reg(IC_SS_SCL_LCNT), 490);
    // Every controller interrupt masked; DMA control 0.
    assert_eq!(m.reg(IC_INTR_MASK), 0);
    assert_eq!(m.reg(IC_DMA_CR), 0);
    // The TX hold raised from the reset value 1 to 4 cycles.
    assert_eq!(m.reg(IC_SDA_HOLD), 4);
    assert_eq!(ctl.sda_hold, 4);
}

#[test]
fn configure_keeps_a_larger_firmware_hold_and_the_rx_hold() {
    let mut m = Model::new(0x15);
    m.set_reg(IC_SDA_HOLD, 0x0005_0010);
    configured(&mut m);
    assert_eq!(m.reg(IC_SDA_HOLD), 0x0005_0010);
    let mut m = Model::new(0x15);
    m.set_reg(IC_SDA_HOLD, 0x0005_0002);
    configured(&mut m);
    assert_eq!(m.reg(IC_SDA_HOLD), 0x0005_0004);
}

#[test]
fn configure_refuses_what_is_not_a_live_designware() {
    let mut m = Model::new(0x15);
    m.all_ones = true;
    assert_eq!(
        configure(&mut m, LPSS_I2C_CLOCK_HZ, &BUDGET),
        Err(Error::AllOnes)
    );
    let mut m = Model::new(0x15);
    m.set_reg(IC_COMP_TYPE, 0);
    assert_eq!(
        configure(&mut m, LPSS_I2C_CLOCK_HZ, &BUDGET),
        Err(Error::NotDesignWare(0))
    );
}

#[test]
fn write_then_read_uses_restart_and_one_stop() {
    let mut m = Model::new(0x15);
    m.device.registers.insert(0x0001, HID_DESC.to_vec());
    let ctl = configured(&mut m);
    let mut out = [0u8; 30];
    transfer(&mut m, &ctl, &BUDGET, 0x15, &[0x01, 0x00], &mut out).unwrap();
    assert_eq!(out, HID_DESC);
    assert!(m.violations.is_empty(), "{:?}", m.violations);
    // On the wire: START 0x15, the two register bytes, a repeated start,
    // 30 reads, one STOP at the end.
    let mut want = vec![Event::Start(0x15), Event::Write(0x01), Event::Write(0x00)];
    want.push(Event::Restart);
    want.extend(HID_DESC.iter().map(|&b| Event::Read(b)));
    want.push(Event::Stop);
    assert_eq!(m.events, want);
    // The commands as written: data, data, READ|RESTART, READ x28,
    // READ|STOP.
    assert_eq!(m.data_cmds[0], 0x01);
    assert_eq!(m.data_cmds[1], 0x00);
    assert_eq!(m.data_cmds[2], DATA_CMD_READ | DATA_CMD_RESTART);
    assert!(m.data_cmds[3..31].iter().all(|&c| c == DATA_CMD_READ));
    assert_eq!(m.data_cmds[31], DATA_CMD_READ | DATA_CMD_STOP);
    assert_eq!(m.data_cmds.len(), 32);
    assert_eq!(m.reg(IC_TAR), 0x15);
}

#[test]
fn a_long_read_never_overflows_either_fifo() {
    let mut m = Model::new(0x15);
    m.tx_depth = 8;
    m.rx_depth = 16;
    let desc: Vec<u8> = (0..700u32).map(|i| (i * 7 + 3) as u8).collect();
    m.device.registers.insert(0x0002, desc.clone());
    let ctl = configured(&mut m);
    let budget = Budget {
        transfer_base: 10_000,
        per_byte: 200,
        ..BUDGET
    };
    let mut out = vec![0u8; 700];
    transfer(&mut m, &ctl, &budget, 0x15, &[0x02, 0x00], &mut out).unwrap();
    assert_eq!(out, desc);
    assert!(m.violations.is_empty(), "{:?}", m.violations);
    assert!(m.max_tx_seen <= 8);
    assert!(m.max_rx_seen <= 16);
}

#[test]
fn write_only_and_read_only_messages() {
    let mut m = Model::new(0x15);
    m.device.input = vec![0x00, 0x00];
    let ctl = configured(&mut m);
    // A command (SET_POWER ON to register 5): four writes, STOP on the last.
    transfer(
        &mut m,
        &ctl,
        &BUDGET,
        0x15,
        &[0x05, 0x00, 0x00, 0x08],
        &mut [],
    )
    .unwrap();
    assert_eq!(m.device.commands, vec![vec![0x05, 0x00, 0x00, 0x08]]);
    assert_eq!(*m.data_cmds.last().unwrap(), 0x08 | DATA_CMD_STOP);
    // A plain input read: no register write, no RESTART.
    m.data_cmds.clear();
    let mut out = [0xEEu8; 4];
    transfer(&mut m, &ctl, &BUDGET, 0x15, &[], &mut out).unwrap();
    assert_eq!(out, [0x00, 0x00, 0x00, 0x00]);
    assert_eq!(m.data_cmds[0], DATA_CMD_READ);
    assert_eq!(m.data_cmds[3], DATA_CMD_READ | DATA_CMD_STOP);
    assert!(m.violations.is_empty(), "{:?}", m.violations);
    assert_eq!(
        transfer(&mut m, &ctl, &BUDGET, 0x15, &[], &mut []),
        Err(Error::TooLong)
    );
}

#[test]
fn an_address_nak_aborts_with_its_source() {
    let mut m = Model::new(0x15);
    let ctl = configured(&mut m);
    let mut out = [0u8; 30];
    let r = transfer(&mut m, &ctl, &BUDGET, 0x2C, &[0x01, 0x00], &mut out);
    let Err(Error::Abort(source)) = r else {
        panic!("{r:?}")
    };
    assert_ne!(source & ABRT_7B_ADDR_NOACK, 0);
    assert_eq!(abort_reason(source), "addr-nak");
    assert_eq!(m.events, vec![Event::Nak(0x2C)]);
    // The controller is left disabled and usable: the next message works.
    m.device.registers.insert(0x0001, HID_DESC.to_vec());
    transfer(&mut m, &ctl, &BUDGET, 0x15, &[0x01, 0x00], &mut out).unwrap();
    assert_eq!(out, HID_DESC);
}

#[test]
fn abort_reasons_decode() {
    assert_eq!(abort_reason(ABRT_7B_ADDR_NOACK), "addr-nak");
    assert_eq!(abort_reason(ABRT_TXDATA_NOACK), "data-nak");
    assert_eq!(abort_reason(ABRT_ARB_LOST), "arb-lost");
    assert_eq!(abort_reason(ABRT_MASTER_DIS), "master-disabled");
    assert_eq!(abort_reason(ABRT_USER_ABRT), "user-abort");
    assert_eq!(abort_reason(1 << 30), "other");
    // Bits per the register map: 7-bit NAK 0, data NAK 3, arbitration 12.
    assert_eq!(ABRT_7B_ADDR_NOACK, 0x1);
    assert_eq!(ABRT_TXDATA_NOACK, 0x8);
    assert_eq!(ABRT_ARB_LOST, 0x1000);
}

#[test]
fn enable_status_lag_is_waited_out() {
    let mut m = Model::new(0x15);
    m.enable_lag = 20;
    m.device.registers.insert(0x0001, HID_DESC.to_vec());
    let ctl = configured(&mut m);
    let mut out = [0u8; 30];
    transfer(&mut m, &ctl, &BUDGET, 0x15, &[0x01, 0x00], &mut out).unwrap();
    assert_eq!(out, HID_DESC);
    // No configuration register (IC_TAR included) was written while the
    // status still said enabled.
    assert!(m.violations.is_empty(), "{:?}", m.violations);
}

#[test]
fn a_controller_that_never_enables_times_out_within_its_budget() {
    let mut m = Model::new(0x15);
    let ctl = configured(&mut m);
    m.stuck_enable = true;
    let start = m.clock;
    let mut out = [0u8; 30];
    let r = transfer(&mut m, &ctl, &BUDGET, 0x15, &[0x01, 0x00], &mut out);
    assert_eq!(r, Err(Error::Timeout(Stage::Enable)));
    // Bounded: the handshake budget plus a few accesses, never more.
    let spent = m.clock - start;
    assert!(spent >= BUDGET.handshake, "{spent}");
    assert!(spent <= BUDGET.handshake * 3 + 20, "{spent}");
}

#[test]
fn a_wedged_bus_is_aborted_not_spun_on() {
    let mut m = Model::new(0x15);
    let ctl = configured(&mut m);
    m.stuck_bus = true;
    let start = m.clock;
    let mut out = [0u8; 30];
    let r = transfer(&mut m, &ctl, &BUDGET, 0x15, &[0x01, 0x00], &mut out);
    assert_eq!(r, Err(Error::Timeout(Stage::Transfer)));
    let spent = m.clock - start;
    let deadline = BUDGET.transfer_base + BUDGET.per_byte * 32;
    assert!(spent >= deadline, "{spent}");
    assert!(spent <= deadline * 2 + BUDGET.handshake * 2, "{spent}");
    // The abort was requested and the controller disabled.
    assert!(m.violations.is_empty(), "{:?}", m.violations);
}

#[test]
fn an_all_ones_controller_mid_transfer_is_reported() {
    let mut m = Model::new(0x15);
    let ctl = configured(&mut m);
    m.all_ones = true;
    let mut out = [0u8; 2];
    assert_eq!(
        transfer(&mut m, &ctl, &BUDGET, 0x15, &[], &mut out),
        Err(Error::AllOnes)
    );
}
