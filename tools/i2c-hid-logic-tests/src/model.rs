// SPDX-License-Identifier: MPL-2.0
//! A register model of a DesignWare I2C master with one HID-over-I2C device
//! on its bus, for running `dw_logic` on the host.
//!
//! It models what the sequencer depends on and checks what it must not do:
//! - `IC_ENABLE_STATUS` follows `IC_ENABLE` after `enable_lag` reads (or
//!   never, `stuck_enable`); configuration writes while enabled are
//!   violations;
//! - the TX FIFO drains one command per register access (or never,
//!   `stuck_bus`); overflowing either FIFO is a violation;
//! - a message to an address the device does not answer aborts with
//!   `ABRT_7B_ADDR_NOACK`, flushes TX and raises STOP_DET; as on the real
//!   core, TX stays flushed (pushes are dropped) until `IC_CLR_TX_ABRT` is
//!   read;
//! - a read following a write without `RESTART` is a violation (the device
//!   would see STOP+START and lose its register pointer, trap 5);
//! - every access advances the clock by one unit.

use std::collections::{BTreeMap, VecDeque};

use crate::dw_logic::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Start(u8),
    Nak(u8),
    Write(u8),
    Restart,
    Read(u8),
    Stop,
}

/// The HID-over-I2C side: a register file for addressed reads, and an
/// input frame for plain reads.
#[derive(Default)]
pub struct Device {
    pub address: u8,
    pub registers: BTreeMap<u16, Vec<u8>>,
    pub input: Vec<u8>,
    written: Vec<u8>,
    pointer: Option<u16>,
    position: usize,
    /// Every complete write-only message (register + payload), e.g. commands.
    pub commands: Vec<Vec<u8>>,
}

impl Device {
    fn begin(&mut self) {
        self.written.clear();
        self.pointer = None;
        self.position = 0;
    }

    fn write(&mut self, byte: u8) {
        self.written.push(byte);
        if self.written.len() == 2 {
            self.pointer = Some(u16::from_le_bytes([self.written[0], self.written[1]]));
            self.position = 0;
        }
    }

    fn read(&mut self) -> u8 {
        let source = match self.pointer {
            Some(reg) => self.registers.get(&reg).cloned().unwrap_or_default(),
            None => self.input.clone(),
        };
        let b = source.get(self.position).copied().unwrap_or(0);
        self.position += 1;
        b
    }

    fn stop(&mut self, read_any: bool) {
        if !read_any && !self.written.is_empty() {
            self.commands.push(self.written.clone());
        }
        self.begin();
    }
}

pub struct Model {
    regs: BTreeMap<usize, u32>,
    enable_target: bool,
    enable_status: bool,
    lag_left: u32,
    pub enable_lag: u32,
    pub stuck_enable: bool,
    pub stuck_bus: bool,
    pub all_ones: bool,
    pub tx_depth: u32,
    pub rx_depth: u32,
    tx: VecDeque<u32>,
    rx: VecDeque<u8>,
    raw: u32,
    abort_source: u32,
    tx_flushed: bool,
    in_message: bool,
    message_wrote: bool,
    message_read: bool,
    pub clock: u64,
    pub device: Device,
    pub events: Vec<Event>,
    pub violations: Vec<String>,
    pub max_tx_seen: usize,
    pub max_rx_seen: usize,
    pub data_cmds: Vec<u32>,
}

impl Model {
    pub fn new(device_address: u8) -> Self {
        let mut regs = BTreeMap::new();
        regs.insert(IC_COMP_TYPE, COMP_TYPE_DW);
        regs.insert(IC_COMP_VERSION, 0x3230_322A);
        // RX hold 0, TX hold 1: a reset value.
        regs.insert(IC_SDA_HOLD, 0x0000_0001);
        Self {
            regs,
            enable_target: false,
            enable_status: false,
            lag_left: 0,
            enable_lag: 0,
            stuck_enable: false,
            stuck_bus: false,
            all_ones: false,
            tx_depth: 64,
            rx_depth: 64,
            tx: VecDeque::new(),
            rx: VecDeque::new(),
            raw: 0,
            abort_source: 0,
            tx_flushed: false,
            in_message: false,
            message_wrote: false,
            message_read: false,
            clock: 0,
            device: Device {
                address: device_address,
                ..Device::default()
            },
            events: Vec::new(),
            violations: Vec::new(),
            max_tx_seen: 0,
            max_rx_seen: 0,
            data_cmds: Vec::new(),
        }
    }

    pub fn reg(&self, offset: usize) -> u32 {
        self.regs.get(&offset).copied().unwrap_or(0)
    }

    pub fn set_reg(&mut self, offset: usize, value: u32) {
        self.regs.insert(offset, value);
    }

    fn comp_param_1(&self) -> u32 {
        ((self.tx_depth - 1) << 16) | ((self.rx_depth - 1) << 8)
    }

    /// One bus step: the next TX command.
    fn step(&mut self) {
        if self.enable_status != self.enable_target && !self.stuck_enable {
            if self.lag_left == 0 {
                self.enable_status = self.enable_target;
            } else {
                self.lag_left -= 1;
            }
        }
        if self.stuck_bus || !self.enable_status {
            return;
        }
        let Some(cmd) = self.tx.pop_front() else {
            return;
        };
        let target = (self.reg(IC_TAR) & 0x7F) as u8;
        if !self.in_message {
            if target != self.device.address {
                self.events.push(Event::Nak(target));
                self.abort_source = ABRT_7B_ADDR_NOACK | ((self.tx.len() as u32 + 1) << 23);
                self.raw |= INTR_TX_ABRT | INTR_STOP_DET;
                self.tx.clear();
                self.tx_flushed = true;
                return;
            }
            self.events.push(Event::Start(target));
            self.in_message = true;
            self.message_wrote = false;
            self.message_read = false;
            self.device.begin();
        }
        if cmd & DATA_CMD_RESTART != 0 {
            self.events.push(Event::Restart);
        }
        if cmd & DATA_CMD_READ != 0 {
            if self.message_wrote && !self.message_read && cmd & DATA_CMD_RESTART == 0 {
                self.violations
                    .push("read after a write without RESTART (STOP+START on the wire)".into());
            }
            self.message_read = true;
            let b = self.device.read();
            self.events.push(Event::Read(b));
            if self.rx.len() as u32 >= self.rx_depth {
                self.violations.push("RX FIFO overflow".into());
            } else {
                self.rx.push_back(b);
                self.max_rx_seen = self.max_rx_seen.max(self.rx.len());
            }
        } else {
            let b = (cmd & 0xFF) as u8;
            self.message_wrote = true;
            self.events.push(Event::Write(b));
            self.device.write(b);
        }
        if cmd & DATA_CMD_STOP != 0 {
            self.events.push(Event::Stop);
            self.device.stop(self.message_read);
            self.in_message = false;
            self.raw |= INTR_STOP_DET;
        }
    }

    fn config_write(&mut self, name: &str) {
        if self.enable_status {
            self.violations
                .push(format!("{name} written while IC_ENABLE_STATUS.IC_EN = 1"));
        }
    }
}

impl Regs for Model {
    fn read(&mut self, offset: usize) -> u32 {
        self.clock += 1;
        self.step();
        if self.all_ones {
            return u32::MAX;
        }
        match offset {
            IC_DATA_CMD => self.rx.pop_front().map(|b| b as u32).unwrap_or(0),
            IC_RXFLR => self.rx.len() as u32,
            IC_TXFLR => self.tx.len() as u32,
            IC_RAW_INTR_STAT => self.raw,
            IC_TX_ABRT_SOURCE => self.abort_source,
            IC_CLR_TX_ABRT => {
                self.raw &= !INTR_TX_ABRT;
                self.tx_flushed = false;
                0
            }
            IC_CLR_STOP_DET => {
                self.raw &= !INTR_STOP_DET;
                0
            }
            IC_CLR_INTR => {
                self.raw = 0;
                self.tx_flushed = false;
                0
            }
            IC_ENABLE_STATUS => self.enable_status as u32,
            IC_COMP_PARAM_1 => self.comp_param_1(),
            IC_STATUS => {
                if self.in_message {
                    STATUS_MST_ACTIVITY
                } else {
                    0
                }
            }
            other => self.reg(other),
        }
    }

    fn write(&mut self, offset: usize, value: u32) {
        self.clock += 1;
        self.step();
        match offset {
            IC_ENABLE => {
                if value & ENABLE_ABORT != 0 {
                    self.tx.clear();
                    self.in_message = false;
                    self.abort_source = ABRT_USER_ABRT;
                    self.raw |= INTR_TX_ABRT;
                    self.tx_flushed = true;
                }
                let target = value & ENABLE != 0;
                if target != self.enable_target {
                    self.lag_left = self.enable_lag;
                }
                self.enable_target = target;
                if !target {
                    self.tx.clear();
                    self.rx.clear();
                }
            }
            IC_DATA_CMD => {
                self.data_cmds.push(value);
                if !self.enable_status {
                    self.violations
                        .push("IC_DATA_CMD written while disabled".into());
                }
                if self.tx_flushed {
                    // Dropped by the core until the abort is cleared.
                } else if self.tx.len() as u32 >= self.tx_depth {
                    self.violations.push("TX FIFO overflow".into());
                } else {
                    self.tx.push_back(value);
                    self.max_tx_seen = self.max_tx_seen.max(self.tx.len());
                }
            }
            IC_CON => self.config_write("IC_CON"),
            IC_TAR => self.config_write("IC_TAR"),
            IC_SS_SCL_HCNT | IC_SS_SCL_LCNT | IC_FS_SCL_HCNT | IC_FS_SCL_LCNT => {
                self.config_write("SCL count")
            }
            IC_SDA_HOLD => self.config_write("IC_SDA_HOLD"),
            _ => {}
        }
        if offset != IC_DATA_CMD && offset != IC_ENABLE {
            self.regs.insert(offset, value);
        }
    }

    fn now(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }
}
