// SPDX-License-Identifier: MPL-2.0
//! `neo-shell` — Slice 1 of the Arthur userland arc.

#![no_std]
#![no_main]

#[cfg(feature = "installer")]
mod install;
mod shell_logic;

#[cfg(not(feature = "arthur-real-brain-boot"))]
use libneo::sys_model_load;
#[cfg(feature = "arthur-grants")]
use libneo::{CapRights, grant_store, meta_rights, sys_derive, sys_grant, sys_kill, sys_revoke};
use libneo::{
    EACCES, EAGAIN, EINVAL, EISDIR, ENOENT, ENOSYS, ENOTDIR, EPERM, FileRights, Handle,
    KERNEL_OBSERVABILITY_MAX_CPUS, KERNEL_OBSERVABILITY_V1_VERSION, KernelObservabilityV1,
    NEO_DIRENT_KIND_DIRECTORY, NEO_DIRENT_KIND_MODEL, NEO_DIRENT_KIND_REGULAR,
    NEO_DIRENT_KIND_SYMLINK, OPEN_ROOT_TEMPORARY, ProcessHandle, STDIN_FD, STDOUT_FD,
    parse_dirent_v1, sys_chan_create, sys_close, sys_exit, sys_model_drop, sys_observe, sys_open,
    sys_read, sys_read_fd, sys_readdir, sys_recv, sys_send, sys_spawn, sys_spawn_stdio,
    sys_stdio_info, sys_sysstat, sys_wait, sys_write,
};
#[cfg(all(feature = "arthur-grants", feature = "net-dial-cap"))]
use libneo::{NetDialCap, net_dial_rights, sys_net_dial_cap_derive};
use shell_logic::{
    DirEntryKind, HELP_TEXT, MAX_LINE_BYTES, MAX_PATH_BYTES, ObserveStatusV1, ParsedCommand,
    format_dir_entry, parse_command, path_error_message, render_observe_status, render_status,
    resolve_path, status_error_message,
};

const PROMPT: &[u8] = b"neo> ";
const READY: &[u8] = b"[ neo-shell ] ready\n";
const EXITING: &[u8] = b"[ neo-shell ] exit\n";
const UNKNOWN: &[u8] = b"[ neo-shell ] unknown command: ";
const BAD_UTF8: &[u8] = b"[ neo-shell ] invalid input\n";
const INPUT_TOO_LONG: &[u8] = b"[ neo-shell ] input too long\n";
const BAD_ARGS: &[u8] = b"[ neo-shell ] bad arguments: ";
const SPAWN_PREFIX: &[u8] = b"[ neo-shell ] spawn ";
const SPAWN_OK: &[u8] = b" ok pid=";
const SPAWN_FAIL: &[u8] = b" failed errno=";
const RELAY_OK: &[u8] = b"[ neo-shell ] relay child=";
const WAIT_EXIT: &[u8] = b" exit=";
const WAIT_FAIL: &[u8] = b" wait_errno=";
const ARTHUR_SPAWN: &[u8] = b"[ neo-shell ] arthur spawn\n";
const ARTHUR_FAIL: &[u8] = b"[ neo-shell ] arthur failed errno=";
const ARTHUR_EXIT: &[u8] = b"[ neo-shell ] arthur exit=";
#[cfg(not(feature = "console-interactive"))]
const ARTHUR_PROMPT: &[u8] = b"arthur> ";
#[cfg(not(feature = "arthur-real-brain-boot"))]
const ARTHUR_MODEL_PATH: &str = "/neofs/model-qwen2.5-1.5b.neom";
#[cfg(feature = "arthur-memory")]
const ARTHUR_MEMORY_DIR: &str = "/neofs/.arthur/memory";
#[cfg(feature = "arthur-grants")]
const ARTHUR_GRANTS_STORE_PATH: &str = "/neofs/.arthur/grants.json";
const NEWLINE: &[u8] = b"\n";

const CHILD_PING: &[u8] = b"neo-shell:ping";
#[cfg(feature = "arthur-grants")]
const GRANTS_BUF_LEN: usize = 2048;
#[cfg(feature = "arthur-grants")]
const MAX_DERIVED_FILE_CAPS: usize = 8;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    sys_exit(run())
}

/// L1a.3 (`terminal-child`): this shell's stdio is a terminal's (its spawn
/// chose a term page). The terminal starts every child cooked with echo
/// (`docs/THINK-2026-10-31-l1a-terminal-window.md` §3.4, §4): it echoes and
/// edits the line and delivers it at Enter, so the shell writes no echo of
/// its own.
#[cfg(feature = "terminal-child")]
static ON_TERMINAL: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

fn echoes() -> bool {
    #[cfg(feature = "terminal-child")]
    return !ON_TERMINAL.load(core::sync::atomic::Ordering::Relaxed);
    #[cfg(not(feature = "terminal-child"))]
    true
}

fn run() -> i32 {
    #[cfg(feature = "terminal-child")]
    if sys_stdio_info().is_ok_and(|info| info.term.is_some()) {
        ON_TERMINAL.store(true, core::sync::atomic::Ordering::Relaxed);
    }
    write_all(READY);
    #[cfg(feature = "v1-console-surface")]
    run_arthur();

    let mut line = [0u8; MAX_LINE_BYTES];
    let mut cwd = [0u8; MAX_PATH_BYTES];
    cwd[0] = b'/';
    let mut cwd_len = 1usize;
    let mut path = [0u8; MAX_PATH_BYTES];

    loop {
        write_all(PROMPT);
        let len = match read_line(&mut line) {
            Ok(len) => len,
            Err(()) => {
                write_all(INPUT_TOO_LONG);
                continue;
            }
        };
        let text = match core::str::from_utf8(&line[..len]) {
            Ok(text) => text,
            Err(_) => {
                write_all(BAD_UTF8);
                continue;
            }
        };

        match parse_command(text) {
            ParsedCommand::Empty => {}
            ParsedCommand::Exit => {
                write_all(EXITING);
                return 0;
            }
            ParsedCommand::Arthur => run_arthur(),
            ParsedCommand::Spawn { name } => spawn_child(name),
            ParsedCommand::Help => write_all(HELP_TEXT),
            ParsedCommand::Ls { path: requested } => {
                let cwd_text = cwd_text(&cwd, cwd_len);
                list_dir(cwd_text, requested.unwrap_or("."), &mut path);
            }
            ParsedCommand::Cat { path: requested } => {
                let cwd_text = cwd_text(&cwd, cwd_len);
                cat_file(cwd_text, requested, &mut path);
            }
            ParsedCommand::Pwd => {
                write_all(&cwd[..cwd_len]);
                write_all(NEWLINE);
            }
            ParsedCommand::Cd { path: requested } => {
                change_dir(requested, &mut cwd, &mut cwd_len, &mut path);
            }
            ParsedCommand::Echo { text } => {
                write_all(text.as_bytes());
                write_all(NEWLINE);
            }
            ParsedCommand::Status => show_status(),
            #[cfg(feature = "installer")]
            ParsedCommand::Install => install::run(),
            ParsedCommand::BadArgs { command } => {
                write_all(BAD_ARGS);
                write_all(command.as_bytes());
                write_all(NEWLINE);
            }
            ParsedCommand::Unknown { command } => {
                write_all(UNKNOWN);
                write_all(command.as_bytes());
                write_all(NEWLINE);
            }
        }
    }
}

#[cfg(not(feature = "arthur-memory"))]
fn run_arthur() {
    write_all(ARTHUR_SPAWN);
    let (local, child) = match sys_chan_create() {
        Ok(handles) => handles,
        Err(e) => {
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };

    let model = load_arthur_model();
    #[cfg(not(feature = "arthur-grants"))]
    let mut inherited = [Handle(0); 3];
    #[cfg(feature = "arthur-grants")]
    let mut inherited = [Handle(0); 16];
    inherited[0] = child;
    #[cfg(not(feature = "arthur-grants"))]
    let inherited_len = if let Some(model) = model {
        // Current spawn inheritance accepts GRANT-able handles; reserve slot 1
        // for the future fs-read cap without changing the kernel ABI here.
        let (_placeholder_parent, placeholder_child) = match sys_chan_create() {
            Ok(handles) => handles,
            Err(e) => {
                let _ = sys_model_drop(model);
                write_all(ARTHUR_FAIL);
                write_i64(e);
                write_all(NEWLINE);
                return;
            }
        };
        inherited[1] = placeholder_child;
        inherited[2] = Handle(model.0);
        3
    } else {
        1
    };
    #[cfg(feature = "arthur-grants")]
    let mut revoke_table = match derive_arthur_caps() {
        Ok(table) => table,
        Err(e) => {
            if let Some(model) = model {
                let _ = sys_model_drop(model);
            }
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };
    #[cfg(feature = "arthur-grants")]
    let inherited_len = match fill_arthur_inherited_with_grants(&mut inherited, model) {
        Ok(len) => len,
        Err(e) => {
            if let Some(model) = model {
                let _ = sys_model_drop(model);
            }
            close_table_caps(&revoke_table);
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };

    let process = match libneo::sys_spawn_client("arthur", &inherited[..inherited_len]) {
        Ok((_pid, process)) => process,
        Err(e) => {
            if let Some(model) = model {
                let _ = sys_model_drop(model);
            }
            #[cfg(feature = "arthur-grants")]
            close_table_caps(&revoke_table);
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };
    #[cfg(feature = "arthur-grants")]
    if let Err(e) = grant_arthur_caps(local, &revoke_table) {
        let _ = sys_kill(process);
        close_table_caps(&revoke_table);
        write_all(ARTHUR_FAIL);
        write_i64(e);
        write_all(NEWLINE);
        return;
    }

    // E3-revoke: the shell RETAINS the per-scope derived caps (it no longer
    // closes them after granting) so `/revoke <resource>` can later call
    // `sys_revoke` on the exact cap whose grant created Arthur's descendant —
    // the kernel revoke walks the derivation graph and kills Arthur's copy plus
    // any sub-derived holder. Unrevoked caps are closed when Arthur exits.
    #[cfg(all(feature = "arthur-grants", not(feature = "console-interactive")))]
    arthur_relay_with_revoke(local, &mut revoke_table);
    #[cfg(all(feature = "console-interactive", not(feature = "arthur-grants")))]
    let _ = local;
    #[cfg(all(not(feature = "console-interactive"), not(feature = "arthur-grants")))]
    arthur_relay(local);

    #[cfg(feature = "arthur-grants")]
    close_table_caps(&revoke_table);

    let exit_code = wait_for_arthur(process);
    if let Some(model) = model {
        let _ = sys_model_drop(model);
    }
    #[cfg(not(feature = "arthur-real-brain-boot"))]
    let _ = exit_code;
    #[cfg(feature = "arthur-real-brain-boot")]
    if exit_code != 0 {
        sys_exit(exit_code);
    }
}

#[cfg(feature = "arthur-memory")]
fn run_arthur() {
    write_all(ARTHUR_SPAWN);
    let (local, child) = match sys_chan_create() {
        Ok(handles) => handles,
        Err(e) => {
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };

    let model = load_arthur_model();
    let memory = sys_open(
        ARTHUR_MEMORY_DIR,
        OPEN_ROOT_TEMPORARY,
        FileRights::LOOKUP.union(FileRights::READDIR),
    )
    .ok();
    let memory_handle = memory.map(|file| Handle(file.0));
    let mut inherited = [Handle(0); 4];
    inherited[0] = child;
    let inherited_len = match fill_arthur_inherited(&mut inherited, model, memory_handle) {
        Ok(len) => len,
        Err(e) => {
            if let Some(model) = model {
                let _ = sys_model_drop(model);
            }
            if let Some(memory) = memory {
                let _ = sys_close(memory);
            }
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };

    let process = match libneo::sys_spawn_client("arthur", &inherited[..inherited_len]) {
        Ok((_pid, process)) => process,
        Err(e) => {
            if let Some(model) = model {
                let _ = sys_model_drop(model);
            }
            if let Some(memory) = memory {
                let _ = sys_close(memory);
            }
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };

    #[cfg(feature = "console-interactive")]
    let _ = local;
    #[cfg(not(feature = "console-interactive"))]
    arthur_relay(local);
    let exit_code = wait_for_arthur(process);
    if let Some(model) = model {
        let _ = sys_model_drop(model);
    }
    if let Some(memory) = memory {
        let _ = sys_close(memory);
    }
    #[cfg(feature = "arthur-real-brain-boot")]
    if exit_code != 0 {
        sys_exit(exit_code);
    }
}

#[cfg(feature = "arthur-real-brain-boot")]
fn load_arthur_model() -> Option<libneo::ModelHandle> {
    None
}

#[cfg(not(feature = "arthur-real-brain-boot"))]
fn load_arthur_model() -> Option<libneo::ModelHandle> {
    sys_model_load(ARTHUR_MODEL_PATH, OPEN_ROOT_TEMPORARY).ok()
}

#[cfg(feature = "arthur-memory")]
fn fill_arthur_inherited(
    inherited: &mut [Handle; 4],
    model: Option<libneo::ModelHandle>,
    memory: Option<Handle>,
) -> Result<usize, i64> {
    if model.is_none() && memory.is_none() {
        return Ok(1);
    }
    let (_placeholder_parent, placeholder_child) = sys_chan_create()?;
    inherited[1] = placeholder_child;
    let Some(model) = model else {
        let (_model_placeholder_parent, model_placeholder_child) = sys_chan_create()?;
        inherited[2] = model_placeholder_child;
        inherited[3] = memory.expect("memory checked above");
        return Ok(4);
    };
    inherited[2] = Handle(model.0);
    if let Some(memory) = memory {
        inherited[3] = memory;
        Ok(4)
    } else {
        Ok(3)
    }
}

// The plain relay is used by the memory variant and the non-grants path. With
// `arthur-grants` on (and not arthur-memory), the grant path uses
// `arthur_relay_with_revoke` instead, so the plain relay would be dead code.
#[cfg(all(
    not(feature = "console-interactive"),
    any(feature = "arthur-memory", not(feature = "arthur-grants"))
))]
fn arthur_relay(local: Handle) {
    let mut line = [0u8; MAX_LINE_BYTES];
    let mut reply = [0u8; 256];
    loop {
        write_all(ARTHUR_PROMPT);
        let len = match read_line(&mut line) {
            Ok(len) => len,
            Err(()) => {
                write_all(INPUT_TOO_LONG);
                continue;
            }
        };
        if sys_send(local, &line[..len]) < 0 {
            write_all(b"[ neo-shell ] arthur send failed\n");
            return;
        }
        let n = sys_recv(local, &mut reply);
        if n < 0 {
            write_all(b"[ neo-shell ] arthur recv failed\n");
            return;
        }
        write_all(&reply[..n as usize]);
        write_all(NEWLINE);
        if matches!(core::str::from_utf8(&line[..len]), Ok("exit")) {
            return;
        }
    }
}

fn wait_for_arthur(process: ProcessHandle) -> i32 {
    match sys_wait(process, 0) {
        Ok(code) => {
            write_all(ARTHUR_EXIT);
            write_i32(code);
            write_all(NEWLINE);
            code
        }
        Err(e) => {
            write_all(ARTHUR_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            252
        }
    }
}

fn cwd_text(cwd: &[u8; MAX_PATH_BYTES], len: usize) -> &str {
    core::str::from_utf8(&cwd[..len]).unwrap_or("/")
}

fn dir_rights() -> FileRights {
    FileRights::LOOKUP.union(FileRights::READDIR)
}

fn resolve_command_path<'a>(
    command: &[u8],
    cwd: &str,
    requested: &str,
    out: &'a mut [u8; MAX_PATH_BYTES],
) -> Option<&'a str> {
    match resolve_path(cwd, requested, out) {
        Ok(len) => match core::str::from_utf8(&out[..len]) {
            Ok(path) => Some(path),
            Err(_) => {
                write_command_message(command, requested.as_bytes(), b"invalid path");
                None
            }
        },
        Err(error) => {
            write_command_message(command, requested.as_bytes(), path_error_message(error));
            None
        }
    }
}

#[cfg(feature = "arthur-grants")]
fn fill_arthur_inherited_with_grants(
    inherited: &mut [Handle; 16],
    model: Option<libneo::ModelHandle>,
) -> Result<usize, i64> {
    let (_slot1_parent, slot1_child) = sys_chan_create()?;
    inherited[1] = slot1_child;
    if let Some(model) = model {
        inherited[2] = Handle(model.0);
    } else {
        let (_slot2_parent, slot2_child) = sys_chan_create()?;
        inherited[2] = slot2_child;
    }

    Ok(3)
}

/// One retained per-scope cap: the derived handle the shell granted to Arthur,
/// plus the `grants.json` scope/resource that keyed it (so `/revoke <resource>`
/// can find it) and whether it has already been revoked (idempotence). The
/// resource is copied in (owned) — it cannot borrow the transient store buffer.
#[cfg(feature = "arthur-grants")]
#[derive(Copy, Clone)]
struct RevokeEntry {
    handle: Handle,
    scope: grant_store::ScopeKind,
    resource: [u8; MAX_PATH_BYTES],
    resource_len: usize,
    revoked: bool,
}

#[cfg(feature = "arthur-grants")]
impl RevokeEntry {
    const EMPTY: Self = Self {
        handle: Handle(0),
        scope: grant_store::ScopeKind::FileRead,
        resource: [0u8; MAX_PATH_BYTES],
        resource_len: 0,
        revoked: false,
    };

    fn resource_str(&self) -> &str {
        core::str::from_utf8(&self.resource[..self.resource_len]).unwrap_or("")
    }
}

/// The per-session set of retained per-scope caps the shell can revoke.
#[cfg(feature = "arthur-grants")]
struct RevokeTable {
    entries: [RevokeEntry; MAX_DERIVED_FILE_CAPS],
    count: usize,
}

#[cfg(feature = "arthur-grants")]
fn derive_arthur_caps() -> Result<RevokeTable, i64> {
    let mut table = RevokeTable {
        entries: [RevokeEntry::EMPTY; MAX_DERIVED_FILE_CAPS],
        count: 0,
    };
    let mut grants_buf = [0u8; GRANTS_BUF_LEN];
    let store = read_grants_store(&mut grants_buf);
    let config = match store {
        Some(len) => grant_store::GrantConfig::Enabled(Some(&grants_buf[..len])),
        None => grant_store::GrantConfig::Enabled(None),
    };
    let mut entries = [grant_store::GrantEntry::EMPTY; grant_store::MAX_GRANTS];
    let entry_count = grant_store::build_grant_entries(config, &mut entries);

    for entry in &entries[..entry_count] {
        if table.count >= table.entries.len() {
            break;
        }
        match entry.scope {
            grant_store::ScopeKind::FileRead | grant_store::ScopeKind::FileWrite => {
                match derive_file_cap_for_grant(entry) {
                    Some(cap) => {
                        let slot = &mut table.entries[table.count];
                        slot.handle = cap;
                        slot.scope = entry.scope;
                        let rlen = entry.resource.len().min(MAX_PATH_BYTES);
                        slot.resource[..rlen].copy_from_slice(&entry.resource.as_bytes()[..rlen]);
                        slot.resource_len = rlen;
                        slot.revoked = false;
                        table.count += 1;
                    }
                    None => {
                        close_table_caps(&table);
                        return Err(EPERM);
                    }
                }
            }
            #[cfg(feature = "net-dial-cap")]
            grant_store::ScopeKind::Network => {
                let (host, port) = match grant_store::entry_to_grant(entry) {
                    Some(libneo::action::Grant::NetDial { host, port, .. }) => (host, port),
                    _ => {
                        close_table_caps(&table);
                        return Err(EINVAL);
                    }
                };
                let cap = match sys_net_dial_cap_derive(NetDialCap(Handle(0)), host, port) {
                    Ok(cap) => cap,
                    Err(e) => {
                        close_table_caps(&table);
                        return Err(e);
                    }
                };
                let slot = &mut table.entries[table.count];
                slot.handle = cap.0;
                slot.scope = entry.scope;
                let rlen = entry.resource.len().min(MAX_PATH_BYTES);
                slot.resource[..rlen].copy_from_slice(&entry.resource.as_bytes()[..rlen]);
                slot.resource_len = rlen;
                slot.revoked = false;
                table.count += 1;
            }
            #[cfg(not(feature = "net-dial-cap"))]
            grant_store::ScopeKind::Network => {}
            grant_store::ScopeKind::Process
            | grant_store::ScopeKind::Model
            | grant_store::ScopeKind::Memory => {}
        }
    }

    Ok(table)
}

#[cfg(feature = "arthur-grants")]
fn grant_arthur_caps(channel: Handle, table: &RevokeTable) -> Result<(), i64> {
    for entry in &table.entries[..table.count] {
        #[cfg(feature = "net-dial-cap")]
        let offered = if entry.scope == grant_store::ScopeKind::Network {
            // REVOKE is self-narrowing only: the recipient may drop its own
            // dial phase authority, never widen or mint a sibling scope.
            CapRights::new(
                meta_rights::USE | meta_rights::REVOKE,
                net_dial_rights::DIAL,
            )
        } else {
            CapRights::new(meta_rights::USE, 0x7f)
        };
        #[cfg(not(feature = "net-dial-cap"))]
        let offered = CapRights::new(meta_rights::USE, 0x7f);
        let r = sys_grant(channel, entry.handle, offered);
        if r != 0 {
            return Err(r);
        }
    }
    Ok(())
}

/// Close every still-live (un-revoked) retained cap. Revoked caps are already
/// gone from the handle table, so they are skipped.
#[cfg(feature = "arthur-grants")]
fn close_table_caps(table: &RevokeTable) {
    for entry in &table.entries[..table.count] {
        if !entry.revoked {
            #[cfg(feature = "net-dial-cap")]
            if entry.scope == grant_store::ScopeKind::Network {
                let _ = sys_revoke(entry.handle);
                continue;
            }
            let _ = sys_close(libneo::FileHandle(entry.handle.0));
        }
    }
}

#[cfg(feature = "arthur-grants")]
fn read_grants_store(buf: &mut [u8]) -> Option<usize> {
    let file = sys_open(
        ARTHUR_GRANTS_STORE_PATH,
        OPEN_ROOT_TEMPORARY,
        FileRights::READ,
    )
    .ok()?;
    let n = sys_read(file, buf);
    let _ = sys_close(file);
    if n < 0 { None } else { Some(n as usize) }
}

#[cfg(feature = "arthur-grants")]
fn derive_file_cap_for_grant(entry: &grant_store::GrantEntry<'_>) -> Option<Handle> {
    let effective = effective_file_rights(entry);
    if effective.bits() == 0 {
        return None;
    }
    let mut path_buf = [0u8; MAX_PATH_BYTES];
    let path = kernel_path_for_grant_resource(entry.resource, &mut path_buf)?;
    let open_rights = open_rights_for_grant_path(path, effective);
    if open_rights.bits() == 0 {
        return None;
    }
    let parent = sys_open(path, OPEN_ROOT_TEMPORARY, open_rights).ok()?;
    let derived = sys_derive(
        Handle(parent.0),
        CapRights::new(
            meta_rights::USE | meta_rights::GRANT,
            (open_rights.bits() & 0x7f) as u8,
        ),
    )
    .ok();
    let _ = sys_close(parent);

    if let Some(cap) = derived {
        write_all(b"[ neo-shell ] grant_derive resource=");
        write_all(entry.resource.as_bytes());
        write_all(b" rights=0x");
        write_hex_u32(effective.bits());
        write_all(NEWLINE);
        Some(cap)
    } else {
        None
    }
}

#[cfg(feature = "arthur-grants")]
fn effective_file_rights(entry: &grant_store::GrantEntry<'_>) -> FileRights {
    match entry.scope {
        grant_store::ScopeKind::FileRead => {
            FileRights(entry.rights.bits() & !FileRights::WRITE.bits())
        }
        grant_store::ScopeKind::FileWrite => entry.rights,
        _ => FileRights(0),
    }
}

#[cfg(feature = "arthur-grants")]
fn open_rights_for_grant_path(path: &str, rights: FileRights) -> FileRights {
    if path.ends_with('/') || path == "/neofs/.arthur" {
        let dir_rights = FileRights::LOOKUP.union(FileRights::READDIR);
        return FileRights(rights.bits() & dir_rights.bits());
    }
    rights
}

#[cfg(feature = "arthur-grants")]
fn kernel_path_for_grant_resource<'a>(
    resource: &'a str,
    out: &'a mut [u8; MAX_PATH_BYTES],
) -> Option<&'a str> {
    if resource.starts_with('/') {
        return Some(resource);
    }
    if resource.starts_with(grant_store::ARTHUR_HOME) {
        let prefix = b"/neofs/";
        let needed = prefix.len().checked_add(resource.len())?;
        if needed > out.len() {
            return None;
        }
        out[..prefix.len()].copy_from_slice(prefix);
        out[prefix.len()..needed].copy_from_slice(resource.as_bytes());
        return core::str::from_utf8(&out[..needed]).ok();
    }
    None
}

/// The Arthur relay loop with the E3-revoke grant-management surface: a
/// `/revoke <resource>` line is intercepted shell-side (it never reaches
/// Arthur); everything else relays as before. This is the production half of
/// ISC-1 — the user withdraws a previously-granted scope and the kernel
/// enforces it through the derivation graph.
#[cfg(all(feature = "arthur-grants", not(feature = "console-interactive")))]
fn arthur_relay_with_revoke(local: Handle, table: &mut RevokeTable) {
    let mut line = [0u8; MAX_LINE_BYTES];
    let mut reply = [0u8; 256];
    loop {
        write_all(ARTHUR_PROMPT);
        let len = match read_line(&mut line) {
            Ok(len) => len,
            Err(()) => {
                write_all(INPUT_TOO_LONG);
                continue;
            }
        };
        if maybe_handle_revoke(table, &line[..len]) {
            continue;
        }
        if sys_send(local, &line[..len]) < 0 {
            write_all(b"[ neo-shell ] arthur send failed\n");
            return;
        }
        let n = sys_recv(local, &mut reply);
        if n < 0 {
            write_all(b"[ neo-shell ] arthur recv failed\n");
            return;
        }
        write_all(&reply[..n as usize]);
        write_all(NEWLINE);
        if matches!(core::str::from_utf8(&line[..len]), Ok("exit")) {
            return;
        }
    }
}

/// If `bytes` is a `/revoke [<resource>]` command, handle it and return `true`
/// (do not relay to Arthur). Returns `false` for any other line.
#[cfg(feature = "arthur-grants")]
fn maybe_handle_revoke(table: &mut RevokeTable, bytes: &[u8]) -> bool {
    let Ok(text) = core::str::from_utf8(bytes) else {
        return false;
    };
    let text = text.trim();
    let Some(rest) = text.strip_prefix("/revoke") else {
        return false;
    };
    // "/revoked" etc. is not the command — require end or whitespace after.
    if !rest.is_empty() && !rest.as_bytes()[0].is_ascii_whitespace() {
        return false;
    }
    let arg = rest.trim();
    if arg.is_empty() {
        write_all(b"[ neo-shell ] usage: /revoke <resource>\n");
        return true;
    }
    handle_revoke(table, arg);
    true
}

/// Revoke the scope whose `grants.json` resource matches `arg`. Enforcement is
/// the kernel `sys_revoke` on the retained cap (transitive through the
/// derivation graph); the `grants.json` rewrite is audit/persistence only.
/// Idempotent: an unknown or already-revoked scope is a no-op success.
#[cfg(feature = "arthur-grants")]
fn handle_revoke(table: &mut RevokeTable, arg: &str) {
    let mut found = None;
    for i in 0..table.count {
        let entry = &table.entries[i];
        if !entry.revoked && entry.resource_str() == arg {
            found = Some(i);
            break;
        }
    }
    let Some(i) = found else {
        write_all(b"[ neo-shell ] revoke ");
        write_all(arg.as_bytes());
        write_all(b": no-op (not granted or already revoked)\n");
        return;
    };

    let handle = table.entries[i].handle;
    let scope = table.entries[i].scope;
    // Enforcement (the security action): kill the cap + every descendant.
    let r = sys_revoke(handle);
    if r != 0 {
        // Fail-safe: leave the scope live and surface the error — never a
        // half-revoked state that looks revoked but isn't.
        write_all(b"[ neo-shell ] revoke ");
        write_all(arg.as_bytes());
        write_all(b": kernel revoke failed errno=");
        write_i64(r);
        write_all(NEWLINE);
        return;
    }
    table.entries[i].revoked = true;

    // Audit/persistence (NOT enforcement): drop the scope from grants.json so it
    // is not re-derived next spawn. The cap is already dead regardless.
    let removed = rewrite_grants_store_without(scope, arg);

    // Discriminator + audit event on the serial console (scope, cap fd).
    write_all(b"[ neo-shell ] revoke scope=");
    write_all(scope.to_name().as_bytes());
    write_all(b" resource=");
    write_all(arg.as_bytes());
    write_all(b" cap_fd=");
    write_u32(handle.0);
    write_all(b" store_removed=");
    write_all(if removed { b"true" } else { b"false" });
    write_all(NEWLINE);
}

/// Rewrite `grants.json` with the `(scope, resource)` entry removed. Returns
/// whether an entry was actually removed from the store (false for the
/// synthesized D7 default, an absent scope, or any read/parse/write failure —
/// all best-effort, since the kernel cap is already revoked).
#[cfg(feature = "arthur-grants")]
fn rewrite_grants_store_without(scope: grant_store::ScopeKind, resource: &str) -> bool {
    let mut read_buf = [0u8; GRANTS_BUF_LEN];
    let Some(old_len) = read_grants_store(&mut read_buf) else {
        return false;
    };
    let mut new_buf = [0u8; GRANTS_BUF_LEN];
    let outcome =
        match grant_store::revoke_grant(&read_buf[..old_len], scope, resource, &mut new_buf) {
            Ok(outcome) => outcome,
            Err(_) => return false,
        };
    if !outcome.removed {
        return false;
    }
    write_grants_store_padded(&new_buf[..outcome.bytes_written], old_len)
}

/// Overwrite `grants.json` with `content`, padded with trailing newlines to at
/// least `min_len` bytes. libneo has no truncate, so a shorter rewrite would
/// leave a stale tail; padding to the old length guarantees a full overwrite,
/// and the v1 parser skips trailing whitespace so the store still parses.
#[cfg(feature = "arthur-grants")]
fn write_grants_store_padded(content: &[u8], min_len: usize) -> bool {
    let file = match sys_open(
        ARTHUR_GRANTS_STORE_PATH,
        OPEN_ROOT_TEMPORARY,
        FileRights::READ.union(FileRights::WRITE),
    ) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut ok = sys_write(file.0, content.as_ptr(), content.len()) == content.len() as i64;
    if ok && content.len() < min_len {
        let nl = [b'\n'; 64];
        let mut remaining = min_len - content.len();
        while remaining > 0 && ok {
            let chunk = remaining.min(nl.len());
            ok = sys_write(file.0, nl.as_ptr(), chunk) == chunk as i64;
            remaining -= chunk;
        }
    }
    let _ = sys_close(file);
    ok
}

#[cfg(feature = "arthur-grants")]
fn write_hex_u32(mut value: u32) {
    let mut buf = [0u8; 8];
    let mut idx = buf.len();
    loop {
        idx -= 1;
        let digit = (value & 0xf) as u8;
        buf[idx] = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + (digit - 10)
        };
        value >>= 4;
        if value == 0 {
            break;
        }
    }
    write_all(&buf[idx..]);
}

fn list_dir(cwd: &str, requested: &str, path: &mut [u8; MAX_PATH_BYTES]) {
    let Some(resolved) = resolve_command_path(b"ls", cwd, requested, path) else {
        return;
    };
    let dir = match sys_open(resolved, OPEN_ROOT_TEMPORARY, dir_rights()) {
        Ok(dir) => dir,
        Err(err) => {
            write_fs_error(b"ls", resolved.as_bytes(), err);
            return;
        }
    };

    let mut cursor = 0u64;
    let mut raw = [0u8; 128];
    let mut line = [0u8; 128];
    loop {
        let n = sys_readdir(dir, &mut raw, &mut cursor);
        if n == 0 {
            break;
        }
        if n < 0 {
            write_fs_error(b"ls", resolved.as_bytes(), n);
            break;
        }
        let entry = match parse_dirent_v1(&raw[..n as usize]) {
            Ok(entry) => entry,
            Err(_) => {
                write_command_message(b"ls", resolved.as_bytes(), b"invalid dir entry");
                break;
            }
        };
        match format_dir_entry(entry.name, dir_entry_kind(entry.kind), &mut line) {
            Some(len) => write_all(&line[..len]),
            None => {
                write_command_message(b"ls", resolved.as_bytes(), b"entry name too long");
                break;
            }
        }
    }
    let _ = sys_close(dir);
}

fn cat_file(cwd: &str, requested: &str, path: &mut [u8; MAX_PATH_BYTES]) {
    let Some(resolved) = resolve_command_path(b"cat", cwd, requested, path) else {
        return;
    };
    let file = match sys_open(resolved, OPEN_ROOT_TEMPORARY, FileRights::READ) {
        Ok(file) => file,
        Err(err) => {
            write_fs_error(b"cat", resolved.as_bytes(), err);
            return;
        }
    };

    let mut buf = [0u8; 128];
    loop {
        let n = sys_read(file, &mut buf);
        if n == 0 {
            break;
        }
        if n < 0 {
            write_fs_error(b"cat", resolved.as_bytes(), n);
            break;
        }
        write_all(&buf[..n as usize]);
    }
    let _ = sys_close(file);
}

fn change_dir(
    requested: &str,
    current: &mut [u8; MAX_PATH_BYTES],
    current_len: &mut usize,
    path: &mut [u8; MAX_PATH_BYTES],
) {
    let cwd = cwd_text(current, *current_len);
    let Some(resolved) = resolve_command_path(b"cd", cwd, requested, path) else {
        return;
    };
    let dir = match sys_open(resolved, OPEN_ROOT_TEMPORARY, dir_rights()) {
        Ok(dir) => dir,
        Err(err) => {
            write_fs_error(b"cd", resolved.as_bytes(), err);
            return;
        }
    };
    let _ = sys_close(dir);
    let len = resolved.len();
    current[..len].copy_from_slice(resolved.as_bytes());
    *current_len = len;
}

fn show_status() {
    match sys_sysstat() {
        Ok(stats) => {
            if stats.version != shell_logic::SYSSTAT_V1_VERSION {
                write_command_message(b"status", b"sysstat", b"unsupported version");
                return;
            }
            let stats = shell_logic::SysStatV1 {
                version: stats.version,
                _pad0: stats._pad0,
                process_count: stats.process_count,
                frames_used: stats.frames_used,
                memory_used_bytes: stats.memory_used_bytes,
                cores_online: stats.cores_online,
                _pad1: stats._pad1,
                uptime_ticks: stats.uptime_ticks,
            };
            let mut line = [0u8; 128];
            match render_status(stats, &mut line) {
                Some(len) => write_all(&line[..len]),
                None => write_command_message(b"status", b"sysstat", b"output too long"),
            }
            show_observe_status();
        }
        Err(errno) => {
            write_all(b"[ neo-shell ] status: ");
            match status_error_message(errno) {
                Some(message) => write_all(message),
                None => {
                    write_all(b"errno=");
                    write_i64(errno);
                }
            }
            write_all(NEWLINE);
        }
    }
}

fn show_observe_status() {
    let stats = match sys_observe() {
        Ok(stats) => stats,
        Err(ENOSYS) => return,
        Err(_) => return,
    };
    if stats.version != KERNEL_OBSERVABILITY_V1_VERSION {
        return;
    }
    let summary = aggregate_observe_status(stats);
    let mut line = [0u8; 256];
    if let Some(len) = render_observe_status(summary, &mut line) {
        write_all(&line[..len]);
    }
}

fn aggregate_observe_status(stats: KernelObservabilityV1) -> ObserveStatusV1 {
    let active = min_usize(stats.cpu_count as usize, KERNEL_OBSERVABILITY_MAX_CPUS);
    let mut out = ObserveStatusV1 {
        cpu_count: stats.cpu_count,
        context_switches: 0,
        dispatches: 0,
        timer_preemptions: 0,
        voluntary_yields: 0,
        ready_queue_depth: 0,
        ready_queue_high_water: 0,
        run_ticks: 0,
        idle_ticks: 0,
        heap_current_bytes: stats.heap_current_bytes,
        heap_high_water_bytes: stats.heap_high_water_bytes,
        heap_alloc_failures: stats.heap_alloc_failures,
        frames_used: stats.frames_used,
        frames_free: stats.buddy_free_pages,
        buddy_largest_free_order: stats.buddy_largest_free_order,
    };
    for cpu in stats.sched.iter().take(active) {
        out.context_switches = out.context_switches.saturating_add(cpu.context_switches);
        out.dispatches = out.dispatches.saturating_add(cpu.dispatches);
        out.timer_preemptions = out.timer_preemptions.saturating_add(cpu.timer_preemptions);
        out.voluntary_yields = out.voluntary_yields.saturating_add(cpu.voluntary_yields);
        out.ready_queue_depth = out.ready_queue_depth.saturating_add(cpu.ready_queue_depth);
        out.ready_queue_high_water = out.ready_queue_high_water.max(cpu.ready_queue_high_water);
        out.run_ticks = out.run_ticks.saturating_add(cpu.run_ticks);
        out.idle_ticks = out.idle_ticks.saturating_add(cpu.idle_ticks);
    }
    out
}

fn min_usize(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

fn dir_entry_kind(kind: u8) -> DirEntryKind {
    match kind {
        NEO_DIRENT_KIND_DIRECTORY => DirEntryKind::Directory,
        NEO_DIRENT_KIND_REGULAR => DirEntryKind::Regular,
        NEO_DIRENT_KIND_SYMLINK => DirEntryKind::Symlink,
        NEO_DIRENT_KIND_MODEL => DirEntryKind::Model,
        _ => DirEntryKind::Other,
    }
}

fn write_command_message(command: &[u8], path: &[u8], message: &[u8]) {
    write_all(b"[ neo-shell ] ");
    write_all(command);
    write_all(b": ");
    write_all(path);
    write_all(b": ");
    write_all(message);
    write_all(NEWLINE);
}

fn write_fs_error(command: &[u8], path: &[u8], errno: i64) {
    match errno_message(errno) {
        Some(message) => write_command_message(command, path, message),
        None => {
            write_all(b"[ neo-shell ] ");
            write_all(command);
            write_all(b": ");
            write_all(path);
            write_all(b": errno=");
            write_i64(errno);
            write_all(NEWLINE);
        }
    }
}

fn errno_message(errno: i64) -> Option<&'static [u8]> {
    match errno {
        ENOENT => Some(b"not found"),
        ENOTDIR => Some(b"not a directory"),
        EACCES | EPERM => Some(b"permission denied"),
        EISDIR => Some(b"is a directory"),
        EINVAL => Some(b"invalid path"),
        _ => None,
    }
}

fn read_line(out: &mut [u8]) -> Result<usize, ()> {
    let mut len = 0;
    let mut overflow = false;
    loop {
        let byte = read_stdin_byte();
        match byte {
            b'\r' | b'\n' => {
                if echoes() {
                    write_all(NEWLINE);
                }
                return if overflow { Err(()) } else { Ok(len) };
            }
            0x08 | 0x7f if len > 0 => {
                len -= 1;
                if echoes() {
                    write_all(b"\x08 \x08");
                }
            }
            0x08 | 0x7f => {}
            0x00..=0x1f => {}
            byte if len < out.len() => {
                out[len] = byte;
                len += 1;
                if echoes() {
                    write_byte(byte);
                }
            }
            _ => overflow = true,
        }
    }
}

fn read_stdin_byte() -> u8 {
    let mut byte = [0u8; 1];
    loop {
        match sys_read_fd(STDIN_FD, &mut byte) {
            1 => return byte[0],
            EAGAIN => core::hint::spin_loop(),
            _ => sys_exit(2),
        }
    }
}

fn spawn_child(name: &str) {
    write_all(SPAWN_PREFIX);
    write_all(name.as_bytes());

    let (local, child) = match sys_chan_create() {
        Ok(handles) => handles,
        Err(e) => {
            write_all(SPAWN_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };
    let (pid, process) = match spawn_with_my_stdio(name, child) {
        Ok(spawned) => spawned,
        Err(e) => {
            write_all(SPAWN_FAIL);
            write_i64(e);
            write_all(NEWLINE);
            return;
        }
    };

    write_all(SPAWN_OK);
    write_u32(pid);
    write_all(NEWLINE);
    relay_to_child(local);
    wait_for_child(process);
}

/// L1a.1 step 2b (`docs/THINK-2026-11-08-l1a1-stdio-selection.md` §2.2):
/// stdio is never inherited implicitly, so a shell whose own stdin and stdout
/// were chosen at spawn (a terminal's pipes) names them for its child: the
/// child's channel in slot 0, then this shell's stdin and stdout. With no
/// selection (or a kernel without `pipe`: `ENOSYS`), the child is spawned as
/// before, its stdio the console.
fn spawn_with_my_stdio(name: &str, child: Handle) -> Result<(u32, ProcessHandle), i64> {
    let Ok(info) = sys_stdio_info() else {
        return sys_spawn(name, &[child]);
    };
    let mut handles = [child, Handle(0), Handle(0)];
    let mut n = 1usize;
    let mut pick = |slot: Option<u32>| {
        slot.map(|s| {
            handles[n] = Handle(s);
            n += 1;
            (n - 1) as u8
        })
    };
    let stdin = pick(info.stdin);
    let stdout = pick(info.stdout);
    if stdin.is_none() && stdout.is_none() {
        return sys_spawn(name, &[child]);
    }
    sys_spawn_stdio(name, &handles[..n], stdin, stdout, None)
}

fn relay_to_child(local: Handle) {
    let sent = sys_send(local, CHILD_PING);
    if sent < 0 {
        write_all(RELAY_OK);
        write_all(b"send_failed errno=");
        write_i64(sent);
        write_all(NEWLINE);
        return;
    }

    let mut reply = [0u8; 64];
    let n = sys_recv(local, &mut reply);
    write_all(RELAY_OK);
    if n < 0 {
        write_all(b"recv_failed errno=");
        write_i64(n);
    } else {
        write_all(&reply[..n as usize]);
    }
    write_all(NEWLINE);
}

fn wait_for_child(process: ProcessHandle) {
    match sys_wait(process, 0) {
        Ok(code) => {
            write_all(WAIT_EXIT);
            write_i32(code);
        }
        Err(e) => {
            write_all(WAIT_FAIL);
            write_i64(e);
        }
    }
    write_all(NEWLINE);
}

fn write_byte(byte: u8) {
    let buf = [byte; 1];
    write_all(&buf);
}

fn write_all(mut bytes: &[u8]) {
    while !bytes.is_empty() {
        let n = sys_write(STDOUT_FD, bytes.as_ptr(), bytes.len());
        if n <= 0 {
            return;
        }
        bytes = &bytes[n as usize..];
    }
}

fn write_i32(value: i32) {
    if value < 0 {
        write_all(b"-");
        write_u32(value.unsigned_abs());
    } else {
        write_u32(value as u32);
    }
}

fn write_i64(value: i64) {
    if value < 0 {
        write_all(b"-");
        write_u64(value.unsigned_abs());
    } else {
        write_u64(value as u64);
    }
}

fn write_u32(value: u32) {
    write_u64(value as u64);
}

fn write_u64(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut idx = buf.len();
    loop {
        idx -= 1;
        buf[idx] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    write_all(&buf[idx..]);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(253);
}
