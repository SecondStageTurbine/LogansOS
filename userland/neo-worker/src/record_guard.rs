// SPDX-License-Identifier: MPL-2.0
//! OT-66 L1 (Dexter #186 item 4), `login_record_guard`: a process that is not
//! the WM tries the sign-in record. neo-init spawns it with no handles after
//! the account exists. Every open of a mount's `.system/` must be `EPERM`
//! (kernel `system-store`): the record for reading and for writing, through a
//! `..` path, the directory itself, and a new file in it. An open of `/neofs`
//! itself must still work (the probe can open; only `.system/` is closed).
//! Dexter #187 qualifier B adds the spellings the normalizer folds (`//`,
//! `./`), a relative path (refused before any check: `EINVAL`), the root
//! mount's own `/.system` (qualifier A), and `SYS_MODEL_LOAD` of the record.
//! `SYS_FILE_REPLACE` takes no path: a write probe that opens the record
//! replaces its bytes with it (or, on a kernel without it, writes over them),
//! and the WM checks both copies afterwards.
//! It logs `RECORDGUARD` with each result and exits 0.

use libneo::{
    FileRights, OPEN_CREATE_REGULAR, OPEN_ROOT_TEMPORARY, sys_close, sys_file_replace, sys_open,
};

/// What a write probe that got the record does with it: replace it whole.
const PLANTED: &[u8] = b"planted by record-guard-probe";

fn put_errno(put: &mut impl FnMut(&[u8]), e: i64) {
    put(b"-");
    let mut digits = [0u8; 20];
    let mut n = e.unsigned_abs();
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    put(&digits[i..]);
}

pub fn run() -> i32 {
    let read = FileRights::READ;
    let rw = FileRights::READ.union(FileRights::WRITE);
    let dir = FileRights::LOOKUP.union(FileRights::READDIR);
    let probes: [(&[u8], &str, u32, FileRights); 10] = [
        (
            b" read=",
            "/neofs/.system/account.cred.a",
            OPEN_ROOT_TEMPORARY,
            read,
        ),
        (
            b" write=",
            "/neofs/.system/account.cred.a",
            OPEN_ROOT_TEMPORARY,
            rw,
        ),
        (
            b" dotdot=",
            "/neofs/x/../.system/account.cred.b",
            OPEN_ROOT_TEMPORARY,
            read,
        ),
        (b" dir=", "/neofs/.system", OPEN_ROOT_TEMPORARY, dir),
        (
            b" create=",
            "/neofs/.system/planted",
            OPEN_CREATE_REGULAR,
            rw,
        ),
        (
            b" slashes=",
            "//neofs//.system/account.cred.a",
            OPEN_ROOT_TEMPORARY,
            rw,
        ),
        (
            b" dot=",
            "/neofs/./.system/account.cred.a",
            OPEN_ROOT_TEMPORARY,
            rw,
        ),
        (
            b" relative=",
            "neofs/.system/account.cred.a",
            OPEN_ROOT_TEMPORARY,
            rw,
        ),
        (
            b" root=",
            "/.system/account.cred.a",
            OPEN_ROOT_TEMPORARY,
            read,
        ),
        (b" control=", "/neofs", OPEN_ROOT_TEMPORARY, dir),
    ];
    let mut line = [0u8; 320];
    let mut len = 0;
    let mut put = |bytes: &[u8]| {
        line[len..len + bytes.len()].copy_from_slice(bytes);
        len += bytes.len();
    };
    put(b"RECORDGUARD");
    for (label, path, flags, rights) in probes {
        put(label);
        match sys_open(path, flags, rights) {
            Ok(handle) => {
                // A record opened for writing is replaced (`SYS_FILE_REPLACE`):
                // the WM's after-check catches it.
                if rights.bits() & FileRights::WRITE.bits() != 0
                    && path.contains("account.cred")
                    && sys_file_replace(handle, PLANTED) < 0
                {
                    let _ = libneo::sys_write(handle.0, PLANTED.as_ptr(), PLANTED.len());
                }
                let _ = sys_close(handle);
                put(b"ok");
            }
            Err(e) => put_errno(&mut put, e),
        }
    }
    put(b" model=");
    match libneo::sys_model_load("/neofs/.system/account.cred.a", 0) {
        Ok(_) => put(b"ok"),
        Err(e) => put_errno(&mut put, e),
    }
    put(b"\n");
    libneo::sys_write(1, line.as_ptr(), len);
    0
}
