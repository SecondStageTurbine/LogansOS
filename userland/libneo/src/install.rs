// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.4: the installer's syscalls. The ABI and the text are
//! [`crate::install_abi`], re-exported here.

extern crate alloc;

use alloc::vec::Vec;

pub use crate::install_abi::*;
use crate::raw3;

/// `SYS_INSTALL_LIST`: every disk, in registry order. Room for eight first
/// (each call runs a preflight per disk); with more, sizes the buffer from
/// the count and calls again.
pub fn sys_install_list() -> Result<Vec<Row>, i64> {
    let mut buf: Vec<u8> = alloc::vec![0; 8 * ROW_BYTES];
    loop {
        let r = unsafe {
            raw3(
                SYS_INSTALL_LIST,
                buf.as_mut_ptr() as u64,
                buf.len() as u64,
                0,
            )
        };
        if r < 0 {
            return Err(r);
        }
        let count = r as usize;
        if count * ROW_BYTES <= buf.len() {
            return Ok(buf[..count * ROW_BYTES]
                .chunks_exact(ROW_BYTES)
                .filter_map(Row::decode)
                .collect());
        }
        buf.resize(count * ROW_BYTES, 0);
    }
}

/// `SYS_INSTALL_RUN`: install to `row` with its offered action. Returns the
/// status (see the kernel's `install_ui_logic`); negative is an errno.
pub fn sys_install_run(row: &Row) -> i64 {
    unsafe {
        raw3(
            SYS_INSTALL_RUN,
            row.slot as u64,
            row.generation,
            row.action as u64,
        )
    }
}

/// The installer cap's index in this process's handle table, if it holds
/// one.
pub fn installer_cap() -> Option<crate::Handle> {
    (0..256u32)
        .find(|&idx| {
            matches!(
                crate::sys_handle_info(idx),
                Ok(info) if info.kind == crate::handle_info::HandleKind::Installer
            )
        })
        .map(crate::Handle)
}
