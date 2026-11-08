// SPDX-License-Identifier: MPL-2.0
//! OT-90 P4: the `SYS_POWER` ABI, one source for the kernel and libneo
//! (`#[path]`), pinned by `tools/acpi-power-logic-tests`. Pure.
//!
//! `SYS_POWER(action, flags)`: `action` is [`ACTION_POWER_OFF`] or
//! [`ACTION_RESTART`], `flags` must be 0. It returns 0 once the shutdown has
//! started (the machine powers off or restarts soon after), else `EPERM`
//! without a `PowerControl` cap holding the action's right, `EINVAL` for an
//! unknown action or a non-zero flag, `EBUSY` while a shutdown already runs.

/// The syscall number (the first free base slot).
pub const SYS_POWER: u64 = 25;
/// Power off (ACPI S5).
pub const ACTION_POWER_OFF: u64 = 1;
/// Restart.
pub const ACTION_RESTART: u64 = 2;

/// The `PowerControl` cap's semantic rights.
pub const RIGHT_POWER_OFF: u8 = 1 << 0;
pub const RIGHT_RESTART: u8 = 1 << 1;
/// Reserved for sleep (THINK §10); never minted yet.
pub const RIGHT_SLEEP: u8 = 1 << 2;

/// What a `SYS_POWER` call asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    PowerOff,
    Restart,
}

/// Why a `SYS_POWER` call is refused, before any errno mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No cap holding the action's right.
    NotPermitted,
    /// An unknown action or a non-zero flag.
    Invalid,
    /// A shutdown is already running.
    Busy,
}

/// Decide a call: `rights` are the caller's `PowerControl` semantic rights
/// (0 without the cap), `running` whether a shutdown has started. The
/// arguments are checked first, so a bad call is `Invalid` whoever makes it.
pub fn decide(action: u64, flags: u64, rights: u8, running: bool) -> Result<Request, Refusal> {
    if flags != 0 {
        return Err(Refusal::Invalid);
    }
    let (request, right) = match action {
        ACTION_POWER_OFF => (Request::PowerOff, RIGHT_POWER_OFF),
        ACTION_RESTART => (Request::Restart, RIGHT_RESTART),
        _ => return Err(Refusal::Invalid),
    };
    if rights & right == 0 {
        return Err(Refusal::NotPermitted);
    }
    if running {
        return Err(Refusal::Busy);
    }
    Ok(request)
}
