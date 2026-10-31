// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.4: `install`, the console installer (THINK §4.5's console
//! fallback): a numbered list of every disk, refused ones shown with their
//! reason and no number; the user types a number, confirms on one screen,
//! and the kernel installs (`SYS_INSTALL_RUN`). Fresh installs only: a blank
//! disk, or Finish installing an incomplete one.
//!
//! The kernel decides every verdict. The shell only shows the rows and sends
//! back the token of the row the user chose; the kernel re-checks it.

extern crate alloc;

use alloc::string::String;

use libneo::install::{self, Choice, Row};

use crate::{MAX_LINE_BYTES, read_line, write_all};

const TITLE: &[u8] = b"LogansOS installer. Disks:\n";
const NONE: &[u8] = b"[ neo-shell ] install: no disks found\n";
const NOTHING_OFFERED: &[u8] = b"No disk here can take a fresh install.\n";
const PICK: &[u8] = b"Type a disk's number, or press Enter to go back: ";
const NOT_A_CHOICE: &[u8] = b"That isn't the number of a disk you can choose.\n";
const BACK: &[u8] = b"[ neo-shell ] install: back\n";
const CONFIRM_TAIL: &[u8] = b"Type y to go ahead, anything else to go back: ";
const WORKING: &[u8] =
    b"[ neo-shell ] install: installing; don't remove the disk or the USB until it says it is done\n";

fn say(text: &str) {
    write_all(text.as_bytes());
}

fn line(buf: &mut [u8]) -> Option<&str> {
    let len = read_line(buf).ok()?;
    core::str::from_utf8(&buf[..len]).ok()
}

fn row_line(number: Option<usize>, row: &Row) -> String {
    let n = match number {
        Some(n) => alloc::format!("{n:>2}"),
        None => String::from(" -"),
    };
    alloc::format!(
        "{n}  {}  ({})  {}\n",
        install::name_text(row),
        install::detail_text(row),
        install::verdict_text(row)
    )
}

pub fn run() {
    let rows = match install::sys_install_list() {
        Ok(rows) => rows,
        Err(e) => {
            say(&alloc::format!(
                "[ neo-shell ] install: unavailable errno={e}\n"
            ));
            return;
        }
    };
    if rows.is_empty() {
        write_all(NONE);
        return;
    }
    write_all(TITLE);
    let numbers = install::numbers(&rows);
    for (row, number) in rows.iter().zip(&numbers) {
        say(&row_line(*number, row));
    }
    if numbers.iter().all(Option::is_none) {
        write_all(NOTHING_OFFERED);
        return;
    }
    let mut buf = [0u8; MAX_LINE_BYTES];
    let row = loop {
        write_all(PICK);
        let Some(text) = line(&mut buf) else {
            write_all(NOT_A_CHOICE);
            continue;
        };
        match install::choose(&rows, text) {
            Choice::Back => {
                write_all(BACK);
                return;
            }
            Choice::Row(i) => break &rows[i],
            Choice::Invalid => write_all(NOT_A_CHOICE),
        }
    };
    say(&alloc::format!(
        "{} LogansOS on {} ({})?\n",
        install::action_text(row.action),
        install::name_text(row),
        install::detail_text(row)
    ));
    write_all(CONFIRM_TAIL);
    if line(&mut buf).map(str::trim) != Some("y") {
        write_all(BACK);
        return;
    }
    write_all(WORKING);
    let status = install::sys_install_run(row);
    say(&alloc::format!(
        "[ neo-shell ] install: status={status}\n{}\n",
        install::status_text(status)
    ));
}
