// SPDX-License-Identifier: MPL-2.0
//! The boot medium as Limine describes it: the kernel file it loaded, the
//! GPT disk GUID of the disk that file came from (OT-87 R4.3, the
//! installer's `BootDisk` refusal), and, under `usb-persistence`, the entry's
//! command line (OT-100 R3, `docs/THINK-2026-11-10-ot100-usb-persistence.md`
//! §2.3, §4.1). Shared by the installer and the persistence mount.

use crate::limine_file_logic;
#[cfg(feature = "usb-persistence")]
use crate::persist_logic;

/// The kernel file as Limine loaded it. SAFETY (for the `'static`): Limine
/// keeps the executable file mapped for the kernel's life; nothing frees it.
pub fn kernel_file() -> Option<&'static limine::file::File> {
    crate::EXECUTABLE_FILE_REQUEST
        .response()
        .map(|r| r.executable_file())
}

/// The boot medium's GPT disk GUID, as GPT stores it (Limine's file
/// response; zero when the medium has no GPT). Read at the protocol's offset,
/// not through the `limine` crate's field (`limine_file_logic`).
pub fn boot_disk_guid() -> Option<[u8; 16]> {
    let file = kernel_file()? as *const limine::file::File as *const u8;
    // SAFETY: Limine's response points at a whole `struct limine_file`
    // (`limine_file_logic::FILE_BYTES`, every revision), mapped for the
    // kernel's life (see `kernel_file`); read unaligned as plain bytes.
    let raw =
        unsafe { core::ptr::read_unaligned(file as *const [u8; limine_file_logic::FILE_BYTES]) };
    limine_file_logic::gpt_disk_guid(&raw)
}

// OT-100 R3: the boot entry's command line (`persist=on` / `persist=off`).
#[cfg(feature = "usb-persistence")]
#[used]
#[unsafe(link_section = ".requests")]
static CMDLINE_REQUEST: limine::request::ExecutableCmdlineRequest =
    limine::request::ExecutableCmdlineRequest::new();

/// The entry's command line as raw bytes, without its NUL. `None` with no
/// response, a null pointer, or no NUL within `persist_logic::CMDLINE_MAX`
/// bytes. Read as bytes (the `limine` crate's accessor panics on non-UTF-8).
#[cfg(feature = "usb-persistence")]
pub fn cmdline() -> Option<&'static [u8]> {
    let response = CMDLINE_REQUEST.response()?;
    let data: &limine::request::ExecutableCmdlineRespData = response;
    // SAFETY: `ExecutableCmdlineRespData` is `#[repr(C)]` with one field, the
    // `const char *` Limine filled in (the protocol's `cmdline`).
    let ptr = unsafe {
        core::ptr::read(
            data as *const limine::request::ExecutableCmdlineRespData as *const *const u8,
        )
    };
    if ptr.is_null() {
        return None;
    }
    // Find the NUL a byte at a time, never past CMDLINE_MAX: Limine's string
    // is mapped for the kernel's life, and nothing is read past its NUL.
    let mut len = 0;
    while len < persist_logic::CMDLINE_MAX {
        // SAFETY: bytes up to and including the NUL are Limine's string.
        if unsafe { ptr.add(len).read() } == 0 {
            // SAFETY: `len` bytes before the NUL, all of the string.
            return Some(unsafe { core::slice::from_raw_parts(ptr, len) });
        }
        len += 1;
    }
    None
}

/// This boot's persistence switch (THINK §2.3): `persist_logic::parse_cmdline`
/// over the entry's command line. Off unless the entry said exactly
/// `persist=on`.
#[cfg(feature = "usb-persistence")]
pub fn persist() -> persist_logic::Persist {
    persist_logic::parse_cmdline(cmdline())
}
