// SPDX-License-Identifier: MPL-2.0
//! OT-87 R2: Arthur's model from the installed NeoFS volume
//! (`docs/THINK-2026-10-27-ot87-installer.md` §5).
//!
//! A boot module is held to its pin by hashing every byte
//! (`boot_modules::load_model`). A NeoFS file already carries a BLAKE3 content
//! hash, which NeoFS checks against the bytes before it serves them, and the
//! pin is that same quantity (`boot_model_logic`). So a pinned name under
//! `/neofs/` is held to its pin by comparing the verified content hash, with
//! no second pass over 485 MB.
//!
//! - [`pin_for`]: the pin a `/neofs/` path is held to. Unpinned names (other
//!   models, test fixtures) load as before.
//! - [`check`]: the verified content hash against the pin. A mismatch is
//!   `EBADMSG` (Arthur's "Damaged"), as on the boot-module path.
//! - [`recheck`]: after the read, the same hash and generation, so the bytes
//!   read are the bytes checked (NeoFS gives every rewrite a new generation).
//! - [`load`]: open, check, read, re-check, build. The boot preload's NeoFS
//!   source: it runs with interrupts on, on the inference stack.

use crate::boot_model_logic::{self, NeofsPin, Verdict};
use crate::errno::{EBADMSG, EINVAL};
use crate::fs::{FileRights, OpenFile, VerifiedHash};
use crate::serial_println;

/// The pin `path` is held to, if any.
pub fn pin_for(path: &str) -> Option<NeofsPin> {
    // The VFS's normal form drops the leading `/` (`neofs/x`).
    let mut normalized = alloc::string::String::from("/");
    normalized.push_str(&crate::fs::normalize_absolute_path(path).ok()?);
    boot_model_logic::neofs_pin(
        crate::boot_modules::PINS,
        crate::boot_modules::VOCAB_PINS,
        &normalized,
    )
}

/// The verified content hash of `file` against `pin`. Hashes the file once if
/// NeoFS has not verified this generation yet.
pub fn check(file: &OpenFile, path: &str, pin: NeofsPin) -> Result<VerifiedHash, i64> {
    let (pins, module) = match pin {
        NeofsPin::Model(module) => (crate::boot_modules::PINS, module),
        NeofsPin::Vocab(module) => (crate::boot_modules::VOCAB_PINS, module),
    };
    let verified = match file.verified_content_hash() {
        Ok(Some(verified)) => verified,
        Ok(None) => {
            serial_println!("NEOFSMODEL refused path={} reason=no_content_hash", path);
            return Err(EBADMSG);
        }
        Err(e) => {
            serial_println!(
                "NEOFSMODEL refused path={} reason=unverified err={:?}",
                path,
                e
            );
            return Err(EINVAL);
        }
    };
    let verdict = boot_model_logic::verdict(pins, module, &verified.blake3);
    let expected = if verdict == Verdict::Match {
        verified.blake3
    } else {
        boot_model_logic::expected(pins, module).unwrap_or([0; 32])
    };
    let expected = boot_model_logic::hex_digest(&expected);
    let got = boot_model_logic::hex_digest(&verified.blake3);
    serial_println!(
        "NEOFSMODEL digest path={} pin={} expected={} got={} match={}",
        path,
        module,
        core::str::from_utf8(&expected).unwrap_or("?"),
        core::str::from_utf8(&got).unwrap_or("?"),
        verdict == Verdict::Match
    );
    if verdict == Verdict::Match {
        Ok(verified)
    } else {
        Err(EBADMSG)
    }
}

/// After the read: the file still holds the generation [`check`] passed.
pub fn recheck(file: &OpenFile, path: &str, checked: VerifiedHash) -> Result<(), i64> {
    match file.verified_content_hash() {
        Ok(Some(now)) if now == checked => Ok(()),
        _ => {
            serial_println!(
                "NEOFSMODEL refused path={} reason=changed_during_read",
                path
            );
            Err(EBADMSG)
        }
    }
}

/// Load the pinned model at `path` (`/neofs/<file>`): open, check the pin,
/// read, re-check, build. The caller runs it with interrupts on.
pub fn load(
    path: &str,
    owner: crate::sched::process::ProcessId,
) -> Result<crate::ai::model::Model, i64> {
    let Some(pin @ NeofsPin::Model(_)) = pin_for(path) else {
        return Err(EINVAL);
    };
    let file =
        crate::fs::open_root_temporary(path, crate::fs::OPEN_ROOT_TEMPORARY, FileRights::READ)
            .map_err(crate::arch::x86_64::syscall::vfs_errno)?;
    let checked = check(&file, path, pin)?;
    let bytes = crate::arch::x86_64::syscall::read_model_file(&file)?;
    recheck(&file, path, checked)?;
    let provenance = file.provenance_snapshot();
    drop(file);
    crate::mm::boot_census("neofs_model_read");
    let built = crate::ai::model::Model::from_neom_bytes(&bytes, provenance, Some(owner));
    crate::mm::boot_census("neofs_model_built");
    match built {
        Ok(model) => {
            serial_println!("NEOFSMODEL loaded path={} bytes={}", path, bytes.len());
            Ok(model)
        }
        Err(e) => {
            serial_println!("NEOFSMODEL load_failed path={} err={:?}", path, e);
            Err(crate::ai::model::model_errno(e))
        }
    }
}
