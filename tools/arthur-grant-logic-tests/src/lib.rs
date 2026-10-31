// SPDX-License-Identifier: MPL-2.0
//! Host tests for libneo's Arthur grant store (OT-18 Slice E1).
//!
//! The crate root supplies std-compatible shims for the libneo types/syscalls
//! that `action.rs` and `grant_store.rs` reference via `crate::…`, then
//! `#[path]`-mounts both real source files so they compile unchanged against the
//! shims. The tests exercise:
//!   * the hand-laid `grants.json` byte oracle (field-by-field, NOT round-trip),
//!   * the D7 least-privilege default,
//!   * fail-safe degradation of a missing/corrupt store,
//!   * the `arthur-grants` kill-switch equivalence, and
//!   * the **authority boundary**: a `Grant` in the set with no backing kernel
//!     cap still fails `EPERM` at the syscall — `grants.json` is never authority.

#![allow(dead_code)]

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct Handle(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct NetDialCap(pub Handle);

pub const NO_NET_DIAL_CAP: NetDialCap = NetDialCap(Handle(u32::MAX));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct ProcessHandle(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct FileHandle(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct TcpSocketHandle(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct TensorHandle(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct ModelHandle(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct FileRights(pub u32);

impl FileRights {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const LOOKUP: Self = Self(1 << 2);
    pub const READDIR: Self = Self(1 << 3);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CapRights {
    pub meta: u8,
    pub semantic: u8,
}

impl CapRights {
    pub const fn new(meta: u8, semantic: u8) -> Self {
        Self { meta, semantic }
    }
}

pub mod meta_rights {
    pub const USE: u8 = 1 << 0;
}

pub mod process_rights {
    pub const KILL: u8 = 1 << 0;
    pub const WAIT: u8 = 1 << 1;
    pub const INSPECT: u8 = 1 << 2;
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct ProcessSnapshotV1 {
    pub pid: u32,
    pub exit_code: i32,
}

pub const OPEN_ROOT_TEMPORARY: u32 = 0;

/// errno mirror — `EPERM` is what the kernel returns when Arthur lacks the
/// capability for a resource (the boundary the tests assert against).
pub const EPERM: i64 = -1;

pub fn sys_open(_path: &str, _flags: u32, _requested: FileRights) -> Result<FileHandle, i64> {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_read(_handle: FileHandle, _buf: &mut [u8]) -> i64 {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_write(_fd: u32, _buf: *const u8, _len: usize) -> i64 {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_readdir(_handle: FileHandle, _out: &mut [u8], _cursor: &mut u64) -> i64 {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_close(_handle: FileHandle) -> i64 {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_spawn(_name: &str, _handles: &[Handle]) -> Result<(u32, ProcessHandle), i64> {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_kill(_handle: ProcessHandle) -> i64 {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_wait(_handle: ProcessHandle, _flags: u64) -> Result<i32, i64> {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_inspect(_handle: ProcessHandle) -> Result<ProcessSnapshotV1, i64> {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_net_dial(_cap: NetDialCap, _host: &str, _port: u16) -> Result<TcpSocketHandle, i64> {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_tcp_send(_handle: TcpSocketHandle, _data: &[u8]) -> i64 {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_tcp_recv(_handle: TcpSocketHandle, _out: &mut [u8]) -> i64 {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_infer_run(
    _model: ModelHandle,
    _inputs: &[TensorHandle],
    _outputs: &mut [TensorHandle],
    _flags: u32,
) -> Result<usize, i64> {
    panic!("host tests use ActionSyscalls mocks")
}

pub fn sys_model_load(_path: &str, _flags: u32) -> Result<ModelHandle, i64> {
    panic!("host tests use ActionSyscalls mocks")
}

#[path = "../../../userland/libneo/src/action.rs"]
mod action;
#[path = "../../../userland/libneo/src/grant_store.rs"]
mod grant_store;

#[cfg(test)]
mod tests {
    use super::action::{
        Action, ActionError, ActionKind, ActionOutcome, ActionSyscalls, Grant, RequiredCapability,
        execute_action_with,
    };
    use super::grant_store::{
        ARTHUR_HOME, GrantConfig, GrantEntry, GrantParseError, MAX_GRANTS, ScopeKind,
        build_grant_entries, default_grant_entries, entries_to_grants, entry_to_grant,
        parse_grants,
    };
    #[cfg(feature = "arthur-grant-audit")]
    use super::grant_store::{
        ActionLog, ActionLogEntry, GrantListingEntry, build_grant_listing, format_grant_listing,
    };
    #[cfg(feature = "arthur-grant-rerequest")]
    use super::grant_store::{
        ApplyResult, DeniedScopes, GrantDecision, ReRequestConfig, ReRequestOutcome, ScopeRequest,
        apply_grant_decision, handle_denied, infer_scope, parent_dir, serialize_grants,
    };
    #[cfg(feature = "arthur-grant-revoke")]
    use super::grant_store::{RevokeError, revoke_grant};
    use super::*;

    /// The hand-authored canonical store. This text is the byte/text oracle: it
    /// is asserted field-by-field against the loader below, NOT produced by a
    /// writer and read back (a writer+reader built together agree even when both
    /// implement the spec wrong — CLAUDE.md "independent byte oracle").
    const ORACLE: &[u8] = br#"{
  "version": 1,
  "grants": [
    { "scope": "FileRead", "resource": "/home/user/project", "rights": ["read"], "granted_by": "user" },
    { "scope": "FileWrite", "resource": "/home/user/project/notes", "rights": ["write"], "granted_by": "user" },
    { "scope": "Network", "resource": "api.example.com:443", "rights": [], "granted_by": "user" }
  ]
}"#;

    const PLACEHOLDER_GRANT: Grant<'static> = Grant::File {
        path: "",
        rights: FileRights(0),
    };

    #[test]
    fn byte_oracle_parses_field_by_field() {
        let mut out = [GrantEntry::EMPTY; MAX_GRANTS];
        let count = parse_grants(ORACLE, &mut out).expect("oracle parses");
        assert_eq!(count, 3);

        assert_eq!(
            out[0],
            GrantEntry {
                scope: ScopeKind::FileRead,
                resource: "/home/user/project",
                rights: FileRights::READ,
                granted_by: "user",
            }
        );
        assert_eq!(
            out[1],
            GrantEntry {
                scope: ScopeKind::FileWrite,
                resource: "/home/user/project/notes",
                rights: FileRights::WRITE,
                granted_by: "user",
            }
        );
        assert_eq!(
            out[2],
            GrantEntry {
                scope: ScopeKind::Network,
                resource: "api.example.com:443",
                rights: FileRights(0),
                granted_by: "user",
            }
        );
    }

    #[test]
    fn entry_to_grant_maps_stable_e5_scopes_and_defers_session_handles() {
        let read = GrantEntry {
            scope: ScopeKind::FileRead,
            resource: "/home/user/project",
            rights: FileRights::READ,
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&read),
            Some(Grant::File {
                path: "/home/user/project",
                rights: FileRights::READ,
            })
        );

        let write = GrantEntry {
            scope: ScopeKind::FileWrite,
            resource: "/scratch",
            rights: FileRights::WRITE,
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&write),
            Some(Grant::File {
                path: "/scratch",
                rights: FileRights::WRITE,
            })
        );

        let process = GrantEntry {
            scope: ScopeKind::Process,
            resource: "worker",
            rights: FileRights(0),
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&process),
            Some(Grant::SpawnProcess { name: "worker" })
        );

        let network = GrantEntry {
            scope: ScopeKind::Network,
            resource: "api.example.com:443",
            rights: FileRights(0),
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&network),
            Some(Grant::NetDial {
                host: "api.example.com",
                port: 443,
                cap: NO_NET_DIAL_CAP,
            })
        );

        let model = GrantEntry {
            scope: ScopeKind::Model,
            resource: "/models/a3.neom",
            rights: FileRights(0),
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&model),
            Some(Grant::ModelLoad {
                path: "/models/a3.neom",
            })
        );

        let entry = GrantEntry {
            scope: ScopeKind::Memory,
            resource: "whatever",
            rights: FileRights(0),
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&entry),
            None,
            "memory grants are a deferred seam"
        );
        let session_process = GrantEntry {
            scope: ScopeKind::Process,
            resource: "kill:4242",
            rights: FileRights(0),
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&session_process),
            None,
            "process handles are session-only"
        );
        let socket = GrantEntry {
            scope: ScopeKind::Network,
            resource: "api.example.com",
            rights: FileRights(0),
            granted_by: "user",
        };
        assert_eq!(
            entry_to_grant(&socket),
            None,
            "network stores must carry host:port"
        );
    }

    #[test]
    fn multi_token_rights_resolve_to_bitset() {
        let store = br#"{"version":1,"grants":[
            {"scope":"FileRead","resource":"/d","rights":["read","lookup","readdir"],"granted_by":"user"}]}"#;
        let mut out = [GrantEntry::EMPTY; 4];
        let count = parse_grants(store, &mut out).expect("parses");
        assert_eq!(count, 1);
        assert_eq!(
            out[0].rights,
            FileRights::READ
                .union(FileRights::LOOKUP)
                .union(FileRights::READDIR)
        );
    }

    #[test]
    fn d7_default_is_filewrite_free_read_on_arthur_home() {
        let mut out = [GrantEntry::EMPTY; 4];
        let count = default_grant_entries(&mut out);
        assert_eq!(count, 1);
        assert_eq!(out[0].scope, ScopeKind::FileRead);
        assert_eq!(out[0].resource, ARTHUR_HOME);
        assert_eq!(
            out[0].rights,
            FileRights::READ
                .union(FileRights::LOOKUP)
                .union(FileRights::READDIR)
        );
        // D7 stays narrow: read + list only, never WRITE (D5 guard, OT-19 unbuilt).
        assert_eq!(out[0].rights.bits() & FileRights::WRITE.bits(), 0);
        assert_eq!(out[0].granted_by, "default");
    }

    #[test]
    fn missing_or_corrupt_store_degrades_to_d7_default() {
        // No store file present (first boot) → D7 default only.
        let mut out = [GrantEntry::EMPTY; MAX_GRANTS];
        let count = build_grant_entries(GrantConfig::Enabled(None), &mut out);
        assert_eq!(count, 1);
        assert_eq!(out[0].resource, ARTHUR_HOME);

        // Corrupt store → D7 default only (fail-safe: no crash, no widening).
        let mut out = [GrantEntry::EMPTY; MAX_GRANTS];
        let count = build_grant_entries(GrantConfig::Enabled(Some(b"{ not json")), &mut out);
        assert_eq!(count, 1);
        assert_eq!(out[0].resource, ARTHUR_HOME);
    }

    #[test]
    fn enabled_store_yields_d7_default_plus_parsed_grants() {
        let mut out = [GrantEntry::EMPTY; MAX_GRANTS];
        let count = build_grant_entries(GrantConfig::Enabled(Some(ORACLE)), &mut out);
        // D7 default (1) + the three parsed entries.
        assert_eq!(count, 4);
        assert_eq!(out[0].resource, ARTHUR_HOME); // D7 default first
        assert_eq!(out[1].resource, "/home/user/project");
        assert_eq!(out[2].resource, "/home/user/project/notes");
        assert_eq!(out[3].resource, "api.example.com:443");
    }

    #[test]
    fn kill_switch_disabled_consults_no_store_and_yields_no_grants() {
        // Disabled ignores even a valid store — the store cannot leak in.
        let mut out = [GrantEntry::EMPTY; MAX_GRANTS];
        assert_eq!(build_grant_entries(GrantConfig::Disabled, &mut out), 0);

        // The ONLY difference is the config flag: Enabled with the same world
        // yields grants. This is the kill-switch equivalence (≡ pre-E1 when off).
        let mut out = [GrantEntry::EMPTY; MAX_GRANTS];
        assert!(build_grant_entries(GrantConfig::Enabled(Some(ORACLE)), &mut out) > 0);
    }

    // ── OT-18 E3-revoke: revoke_grant store rewrite (the audit/persistence half) ──
    //
    // These exercise ONLY the store/audit layer. The enforcement (the kernel
    // `revoke_handle` on the per-scope cap) is proven by the `arthur_grant_revoke`
    // QEMU scenario; revoke_grant never enforces — it rewrites grants.json so the
    // scope is not re-derived next spawn.

    #[cfg(feature = "arthur-grant-revoke")]
    const REVOKE_STORE: &[u8] = br#"{
  "version": 1,
  "grants": [
    { "scope": "FileRead", "resource": "/home/user/project", "rights": ["read"], "granted_by": "user" },
    { "scope": "FileWrite", "resource": "/home/user/notes", "rights": ["write"], "granted_by": "user" }
  ]
}"#;

    // Hand-laid expected rewrite after removing FileRead /home/user/project — the
    // independent oracle for the rewritten store (canonical serialize_grants
    // form), asserted byte-for-byte, NOT produced by re-reading the writer.
    #[cfg(feature = "arthur-grant-revoke")]
    const REVOKE_EXPECTED_AFTER_FILEREAD: &[u8] = b"{\n  \"version\": 1,\n  \"grants\": [\n    { \"scope\": \"FileWrite\", \"resource\": \"/home/user/notes\", \"rights\": [\"write\"], \"granted_by\": \"user\" }\n  ]\n}";

    #[cfg(feature = "arthur-grant-revoke")]
    #[test]
    fn revoke_removes_named_scope_byte_for_byte() {
        let mut out = [0u8; 512];
        let outcome = revoke_grant(
            REVOKE_STORE,
            ScopeKind::FileRead,
            "/home/user/project",
            &mut out,
        )
        .expect("valid store rewrites");
        assert!(
            outcome.removed,
            "the FileRead scope was present and removed"
        );
        assert_eq!(outcome.remaining, 1);
        assert_eq!(
            &out[..outcome.bytes_written],
            REVOKE_EXPECTED_AFTER_FILEREAD,
            "rewritten store must match the hand-laid oracle byte-for-byte"
        );
        // And it re-parses to exactly the surviving FileWrite entry.
        let mut entries = [GrantEntry::EMPTY; MAX_GRANTS];
        let n = parse_grants(&out[..outcome.bytes_written], &mut entries).expect("roundtrips");
        assert_eq!(n, 1);
        assert_eq!(entries[0].scope, ScopeKind::FileWrite);
        assert_eq!(entries[0].resource, "/home/user/notes");
    }

    #[cfg(feature = "arthur-grant-revoke")]
    #[test]
    fn revoke_absent_scope_is_noop_success() {
        let mut out = [0u8; 512];
        let outcome = revoke_grant(REVOKE_STORE, ScopeKind::FileRead, "/not/granted", &mut out)
            .expect("valid store");
        assert!(!outcome.removed, "absent scope ⇒ no-op success, not error");
        assert_eq!(outcome.remaining, 2, "both entries survive");
        let mut entries = [GrantEntry::EMPTY; MAX_GRANTS];
        let n = parse_grants(&out[..outcome.bytes_written], &mut entries).expect("roundtrips");
        assert_eq!(n, 2);
    }

    #[cfg(feature = "arthur-grant-revoke")]
    #[test]
    fn revoke_is_idempotent_second_call_is_noop() {
        let mut out = [0u8; 512];
        let first = revoke_grant(
            REVOKE_STORE,
            ScopeKind::FileRead,
            "/home/user/project",
            &mut out,
        )
        .expect("first revoke");
        assert!(first.removed);
        // Re-revoking the SAME scope on the rewritten store is a no-op success
        // (the cap is already dead at the kernel; the store has nothing to drop).
        let mut out2 = [0u8; 512];
        let second = revoke_grant(
            &out[..first.bytes_written],
            ScopeKind::FileRead,
            "/home/user/project",
            &mut out2,
        )
        .expect("second revoke");
        assert!(!second.removed);
        assert_eq!(second.remaining, 1);
    }

    #[cfg(feature = "arthur-grant-revoke")]
    #[test]
    fn revoke_is_minimal_same_path_other_scope_and_other_path_survive() {
        // FileRead and FileWrite on the SAME path, plus FileRead on another path.
        let store = br#"{"version":1,"grants":[
            {"scope":"FileRead","resource":"/p","rights":["read"],"granted_by":"user"},
            {"scope":"FileWrite","resource":"/p","rights":["write"],"granted_by":"user"},
            {"scope":"FileRead","resource":"/q","rights":["read"],"granted_by":"user"}]}"#;
        let mut out = [0u8; 512];
        let outcome = revoke_grant(store, ScopeKind::FileRead, "/p", &mut out).expect("rewrites");
        assert!(outcome.removed);
        assert_eq!(outcome.remaining, 2, "FileWrite /p and FileRead /q survive");
        let mut entries = [GrantEntry::EMPTY; MAX_GRANTS];
        let n = parse_grants(&out[..outcome.bytes_written], &mut entries).expect("roundtrips");
        assert_eq!(n, 2);
        // FileWrite /p is untouched; FileRead /q is untouched; FileRead /p is gone.
        assert!(
            entries[..n]
                .iter()
                .any(|e| e.scope == ScopeKind::FileWrite && e.resource == "/p")
        );
        assert!(
            entries[..n]
                .iter()
                .any(|e| e.scope == ScopeKind::FileRead && e.resource == "/q")
        );
        assert!(
            !entries[..n]
                .iter()
                .any(|e| e.scope == ScopeKind::FileRead && e.resource == "/p")
        );
    }

    #[cfg(feature = "arthur-grant-revoke")]
    #[test]
    fn revoke_malformed_store_is_error_not_panic() {
        let mut out = [0u8; 512];
        assert_eq!(
            revoke_grant(b"{ not json", ScopeKind::FileRead, "/x", &mut out),
            Err(RevokeError::Malformed)
        );
    }

    #[cfg(feature = "arthur-grant-revoke")]
    #[test]
    fn revoke_buffer_too_small_is_error() {
        let mut out = [0u8; 8]; // far too small for the rewritten store
        assert_eq!(
            revoke_grant(
                REVOKE_STORE,
                ScopeKind::FileWrite,
                "/home/user/notes",
                &mut out
            ),
            Err(RevokeError::BufferTooSmall)
        );
    }

    // ── format rejection (fail-safe inputs) ──────────────────────────────────

    #[test]
    fn version_mismatch_is_rejected() {
        let store = br#"{"version":2,"grants":[]}"#;
        let mut out = [GrantEntry::EMPTY; 4];
        assert_eq!(
            parse_grants(store, &mut out),
            Err(GrantParseError::BadVersion)
        );
    }

    #[test]
    fn unknown_scope_is_rejected() {
        let store =
            br#"{"version":1,"grants":[{"scope":"Telepathy","resource":"/x","rights":["read"],"granted_by":"user"}]}"#;
        let mut out = [GrantEntry::EMPTY; 4];
        assert_eq!(
            parse_grants(store, &mut out),
            Err(GrantParseError::UnknownScope)
        );
    }

    #[test]
    fn unknown_right_is_rejected() {
        let store =
            br#"{"version":1,"grants":[{"scope":"FileRead","resource":"/x","rights":["teleport"],"granted_by":"user"}]}"#;
        let mut out = [GrantEntry::EMPTY; 4];
        assert_eq!(
            parse_grants(store, &mut out),
            Err(GrantParseError::UnknownRight)
        );
    }

    #[test]
    fn missing_field_is_rejected() {
        let store = br#"{"version":1,"grants":[{"scope":"FileRead","resource":"/x"}]}"#;
        let mut out = [GrantEntry::EMPTY; 4];
        assert_eq!(
            parse_grants(store, &mut out),
            Err(GrantParseError::MissingField)
        );
    }

    #[test]
    fn escape_sequences_and_control_chars_are_rejected() {
        let escaped =
            br#"{"version":1,"grants":[{"scope":"FileRead","resource":"/a\/b","rights":["read"],"granted_by":"user"}]}"#;
        let mut out = [GrantEntry::EMPTY; 4];
        assert_eq!(
            parse_grants(escaped, &mut out),
            Err(GrantParseError::Malformed)
        );
    }

    #[test]
    fn trailing_garbage_is_rejected() {
        let store = br#"{"version":1,"grants":[]} trailing"#;
        let mut out = [GrantEntry::EMPTY; 4];
        assert_eq!(
            parse_grants(store, &mut out),
            Err(GrantParseError::Malformed)
        );
    }

    #[test]
    fn empty_input_is_rejected() {
        let mut out = [GrantEntry::EMPTY; 4];
        assert_eq!(parse_grants(b"", &mut out), Err(GrantParseError::Empty));
    }

    #[test]
    fn overflowing_caller_buffer_is_rejected() {
        let store =
            br#"{"version":1,"grants":[{"scope":"FileRead","resource":"/a","rights":["read"],"granted_by":"u"},{"scope":"FileRead","resource":"/b","rights":["read"],"granted_by":"u"}]}"#;
        let mut out = [GrantEntry::EMPTY; 1];
        assert_eq!(
            parse_grants(store, &mut out),
            Err(GrantParseError::TooManyGrants)
        );
    }

    // ── the authority boundary (ISC-2) ───────────────────────────────────────

    /// A mock that models the kernel capability check: `open` succeeds only for
    /// resources Arthur actually holds a cap for. Everything else is `EPERM` —
    /// exactly what the real kernel returns when the cap is absent.
    struct CapModelingSyscalls {
        held: Vec<&'static str>,
    }

    impl ActionSyscalls for CapModelingSyscalls {
        fn open(&mut self, path: &str, _rights: FileRights) -> Result<FileHandle, i64> {
            // A held kernel cap for this exact resource → open succeeds; anything
            // else is EPERM (an explicit loop: clippy's `contains` rewrite does
            // not typecheck here — `&'static str` element vs `&str` argument).
            for held in &self.held {
                if *held == path {
                    return Ok(FileHandle(7));
                }
            }
            Err(EPERM)
        }

        fn read(&mut self, _handle: FileHandle, out: &mut [u8]) -> i64 {
            let payload = b"secret-bytes";
            let n = payload.len().min(out.len());
            out[..n].copy_from_slice(&payload[..n]);
            n as i64
        }

        fn close(&mut self, _handle: FileHandle) -> i64 {
            0
        }

        fn write(&mut self, _handle: FileHandle, _data: &[u8]) -> i64 {
            panic!("unused in boundary test")
        }
        fn readdir(&mut self, _handle: FileHandle, _out: &mut [u8], _cursor: &mut u64) -> i64 {
            panic!("unused in boundary test")
        }
        fn spawn(&mut self, _name: &str, _handles: &[Handle]) -> Result<(u32, ProcessHandle), i64> {
            panic!("unused in boundary test")
        }
        fn kill(&mut self, _handle: ProcessHandle) -> i64 {
            panic!("unused in boundary test")
        }
        fn inspect(&mut self, _handle: ProcessHandle) -> Result<ProcessSnapshotV1, i64> {
            panic!("unused in boundary test")
        }
        fn wait(&mut self, _handle: ProcessHandle, _flags: u64) -> Result<i32, i64> {
            panic!("unused in boundary test")
        }
        fn net_dial(
            &mut self,
            _cap: NetDialCap,
            _host: &str,
            _port: u16,
        ) -> Result<TcpSocketHandle, i64> {
            panic!("unused in boundary test")
        }
        fn tcp_send(&mut self, _handle: TcpSocketHandle, _data: &[u8]) -> i64 {
            panic!("unused in boundary test")
        }
        fn tcp_recv(&mut self, _handle: TcpSocketHandle, _out: &mut [u8]) -> i64 {
            panic!("unused in boundary test")
        }
        fn infer(
            &mut self,
            _model: ModelHandle,
            _inputs: &[TensorHandle],
            _outputs: &mut [TensorHandle],
            _flags: u32,
        ) -> Result<usize, i64> {
            panic!("unused in boundary test")
        }
        fn model_load(&mut self, _path: &str, _flags: u32) -> Result<ModelHandle, i64> {
            panic!("unused in boundary test")
        }
    }

    /// Build the full execute_action grant set from a store that grants
    /// `FileRead` on `/secret`, into the caller's buffer.
    fn secret_grant_set(grants: &mut [Grant<'static>]) -> usize {
        const SECRET_STORE: &[u8] =
            br#"{"version":1,"grants":[{"scope":"FileRead","resource":"/secret","rights":["read"],"granted_by":"user"}]}"#;
        let mut entries = [GrantEntry::EMPTY; MAX_GRANTS];
        let n = build_grant_entries(GrantConfig::Enabled(Some(SECRET_STORE)), &mut entries);
        entries_to_grants(&entries[..n], grants)
    }

    #[test]
    fn grant_in_set_without_kernel_cap_still_eperm() {
        // The policy mirror SAYS read /secret is allowed...
        let mut grants = [PLACEHOLDER_GRANT; MAX_GRANTS];
        let g = secret_grant_set(&mut grants);
        assert!(g >= 2); // D7 default (.arthur/) + /secret

        // ...but Arthur holds NO kernel cap for /secret. The kernel is the
        // boundary: the syscall fails EPERM regardless of grants.json.
        let mut sys = CapModelingSyscalls { held: vec![] };
        let mut buf = [0u8; 32];
        let err = execute_action_with(
            &mut sys,
            &grants[..g],
            Action::ReadFile {
                path: "/secret",
                out: &mut buf,
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            ActionError::Syscall {
                action: ActionKind::ReadFile,
                errno: EPERM,
            }
        );
    }

    #[test]
    fn grant_with_backing_kernel_cap_succeeds() {
        let mut grants = [PLACEHOLDER_GRANT; MAX_GRANTS];
        let g = secret_grant_set(&mut grants);

        // Same grant set, but now Arthur holds the kernel cap → the read works.
        // The kernel cap — not grants.json — is what authorises the action.
        let mut sys = CapModelingSyscalls {
            held: vec!["/secret"],
        };
        let mut buf = [0u8; 32];
        let outcome = execute_action_with(
            &mut sys,
            &grants[..g],
            Action::ReadFile {
                path: "/secret",
                out: &mut buf,
            },
        )
        .expect("held cap authorises the read");
        assert_eq!(outcome, ActionOutcome::ReadFile { bytes: 12 });
        assert_eq!(&buf[..12], b"secret-bytes");
    }

    #[test]
    fn absent_grant_is_denied_before_any_syscall() {
        // With only the D7 default (.arthur/) granted, a /secret read never
        // reaches the kernel — the policy mirror denies it first.
        let mut entries = [GrantEntry::EMPTY; MAX_GRANTS];
        let n = build_grant_entries(GrantConfig::Enabled(None), &mut entries);
        let mut grants = [PLACEHOLDER_GRANT; MAX_GRANTS];
        let g = entries_to_grants(&entries[..n], &mut grants);

        let mut sys = CapModelingSyscalls {
            held: vec!["/secret"], // even though the cap is held
        };
        let mut buf = [0u8; 32];
        let err = execute_action_with(
            &mut sys,
            &grants[..g],
            Action::ReadFile {
                path: "/secret",
                out: &mut buf,
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            ActionError::Denied {
                action: ActionKind::ReadFile,
                required: RequiredCapability::File {
                    path: "/secret",
                    rights: FileRights::READ,
                },
            }
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // OT-18 E2 — on-miss re-request host KATs
    // ═══════════════════════════════════════════════════════════════════════

    /// The three `GrantEntry` values the ORACLE fixture encodes — used by the
    /// writer byte-oracle test (serialize these → must equal ORACLE bytes).
    #[cfg(feature = "arthur-grant-rerequest")]
    const ORACLE_ENTRIES: [GrantEntry<'static>; 3] = [
        GrantEntry {
            scope: ScopeKind::FileRead,
            resource: "/home/user/project",
            rights: FileRights::READ,
            granted_by: "user",
        },
        GrantEntry {
            scope: ScopeKind::FileWrite,
            resource: "/home/user/project/notes",
            rights: FileRights::WRITE,
            granted_by: "user",
        },
        GrantEntry {
            scope: ScopeKind::Network,
            resource: "api.example.com:443",
            rights: FileRights(0),
            granted_by: "user",
        },
    ];

    // ── ISC-1: scope inference ──────────────────────────────────────────────

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn scope_inference_file_parent_dir() {
        // Deep path → parent directory with trailing slash.
        let req = RequiredCapability::File {
            path: "/home/user/cal/june.cal",
            rights: FileRights::READ,
        };
        let inferred = infer_scope(&req).expect("File is inferable");
        assert_eq!(inferred.scope, ScopeKind::FileRead);
        assert_eq!(inferred.resource, "/home/user/cal/");
        assert_eq!(inferred.rights, FileRights::READ);

        // Root-level file → parent is "/".
        let req = RequiredCapability::File {
            path: "/hello.txt",
            rights: FileRights::READ,
        };
        let inferred = infer_scope(&req).expect("File is inferable");
        assert_eq!(inferred.scope, ScopeKind::FileRead);
        assert_eq!(inferred.resource, "/");

        // /etc/shadow → /etc/ (the THINK's edge case — inference is correct,
        // the user's "no" is the safety).
        let req = RequiredCapability::File {
            path: "/etc/shadow",
            rights: FileRights::READ,
        };
        let inferred = infer_scope(&req).expect("File is inferable");
        assert_eq!(inferred.resource, "/etc/");

        // Write action → FileWrite scope.
        let req = RequiredCapability::File {
            path: "/home/user/notes",
            rights: FileRights::WRITE,
        };
        let inferred = infer_scope(&req).expect("File is inferable");
        assert_eq!(inferred.scope, ScopeKind::FileWrite);
        assert_eq!(inferred.resource, "/home/user/");

        // ListDir (LOOKUP|READDIR, no WRITE) → FileRead scope.
        let req = RequiredCapability::File {
            path: "/home/user/docs",
            rights: FileRights::LOOKUP.union(FileRights::READDIR),
        };
        let inferred = infer_scope(&req).expect("File is inferable");
        assert_eq!(inferred.scope, ScopeKind::FileRead);
        assert_eq!(inferred.resource, "/home/user/");
    }

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn scope_inference_non_file_respects_durable_resource_boundary() {
        let req = RequiredCapability::NetDial {
            host: "example.com",
            port: 443,
        };
        assert_eq!(infer_scope(&req), None);

        let req = RequiredCapability::SpawnProcess { name: "worker" };
        let inferred = infer_scope(&req).expect("spawn names are durable process resources");
        assert_eq!(inferred.scope, ScopeKind::Process);
        assert_eq!(inferred.resource, "worker");
        assert_eq!(inferred.rights, FileRights(0));

        let req = RequiredCapability::ModelLoad { path: "/m.neom" };
        let inferred = infer_scope(&req).expect("model paths are durable model resources");
        assert_eq!(inferred.scope, ScopeKind::Model);
        assert_eq!(inferred.resource, "/m.neom");
        assert_eq!(inferred.rights, FileRights(0));

        let req = RequiredCapability::Process {
            handle: ProcessHandle(4242),
            rights: CapRights::new(meta_rights::USE, process_rights::KILL),
        };
        assert_eq!(infer_scope(&req), None);
    }

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn parent_dir_edge_cases() {
        assert_eq!(parent_dir("/"), "/");
        assert_eq!(parent_dir("/hello.txt"), "/");
        assert_eq!(parent_dir("/etc/shadow"), "/etc/");
        assert_eq!(parent_dir("/a/b/c/d"), "/a/b/c/");
        assert_eq!(parent_dir(".arthur/grants.json"), ".arthur/");
    }

    // ── ISC-3: remember-the-denial (anti-nag) ───────────────────────────────

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn remember_the_denial_anti_nag() {
        let mut denied = DeniedScopes::new();
        let req = RequiredCapability::File {
            path: "/etc/shadow",
            rights: FileRights::READ,
        };

        // First Denied → Arthur offers the scope request.
        let outcome = handle_denied(&req, &mut denied, ReRequestConfig::Enabled);
        match outcome {
            ReRequestOutcome::Request(sr) => {
                assert_eq!(sr.scope, ScopeKind::FileRead);
                assert_eq!(sr.resource, "/etc/");
            }
            _ => panic!("first Denied should produce a Request"),
        }

        // User says No → record the denial.
        let scope = ScopeRequest {
            scope: ScopeKind::FileRead,
            resource: "/etc/",
            rights: FileRights::READ,
        };
        let result = apply_grant_decision(GrantDecision::No, &scope, &mut denied, &[], &mut []);
        assert_eq!(result, ApplyResult::Denied);
        assert!(denied.is_denied("/etc/", ScopeKind::FileRead));

        // Second Denied on the same scope → AlreadyDenied (no request — anti-nag).
        let outcome = handle_denied(&req, &mut denied, ReRequestConfig::Enabled);
        assert_eq!(outcome, ReRequestOutcome::AlreadyDenied);
    }

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn denied_scopes_are_per_scope_kind() {
        // Denying FileRead /etc/ does NOT deny FileWrite /etc/ — different scope.
        let mut denied = DeniedScopes::new();
        denied.deny("/etc/", ScopeKind::FileRead);
        assert!(denied.is_denied("/etc/", ScopeKind::FileRead));
        assert!(!denied.is_denied("/etc/", ScopeKind::FileWrite));
    }

    // ── ISC-5: kill-switch equality (off ≡ E1a) ─────────────────────────────

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn kill_switch_disabled_no_escalation() {
        // Kill-switch off → on Denied, Arthur stops. No ScopeRequest, no writer.
        let mut denied = DeniedScopes::new();
        let req = RequiredCapability::File {
            path: "/etc/shadow",
            rights: FileRights::READ,
        };
        let outcome = handle_denied(&req, &mut denied, ReRequestConfig::Disabled);
        assert_eq!(outcome, ReRequestOutcome::Disabled);
        // The denied set is untouched — no scope was recorded.
        assert!(!denied.is_denied("/etc/", ScopeKind::FileRead));
    }

    // ── ISC-4: the writer byte-oracle + round-trip ──────────────────────────

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn writer_reproduces_oracle_byte_for_byte() {
        // The independent byte oracle: serialize the ORACLE_ENTRIES and assert
        // the output equals the hand-laid ORACLE fixture byte-for-byte. This
        // catches a writer+reader that agree with each other but both implement
        // the spec wrong (CLAUDE.md "independent byte oracle" rule).
        let mut out = [0u8; 512];
        let n = serialize_grants(&ORACLE_ENTRIES, &mut out).expect("buffer is large enough");
        assert_eq!(
            &out[..n],
            ORACLE,
            "writer output must match the hand-laid ORACLE byte-for-byte"
        );
    }

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn writer_round_trips_through_parser() {
        // Parse the ORACLE → entries → serialize → must equal ORACLE (round-trip).
        let mut entries = [GrantEntry::EMPTY; MAX_GRANTS];
        let count = parse_grants(ORACLE, &mut entries).expect("oracle parses");
        let mut out = [0u8; 512];
        let n = serialize_grants(&entries[..count], &mut out).expect("buffer is large enough");
        assert_eq!(&out[..n], ORACLE, "round-trip: parse → serialize == ORACLE");

        // And parse the writer's output back → same entries.
        let mut entries2 = [GrantEntry::EMPTY; MAX_GRANTS];
        let count2 = parse_grants(&out[..n], &mut entries2).expect("writer output parses");
        assert_eq!(count, count2);
        assert_eq!(&entries[..count], &entries2[..count2]);
    }

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn writer_empty_entries_produces_valid_empty_store() {
        let mut out = [0u8; 64];
        let n = serialize_grants(&[], &mut out).expect("buffer is large enough");
        // Must round-trip through the parser (0 entries).
        let mut entries = [GrantEntry::EMPTY; 4];
        let count = parse_grants(&out[..n], &mut entries).expect("empty store parses");
        assert_eq!(count, 0);
    }

    // ── ISC-3/ISC-4: grant decisions (Session/Always/No) ────────────────────

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn grant_decision_no_records_denial() {
        let mut denied = DeniedScopes::new();
        let scope = ScopeRequest {
            scope: ScopeKind::FileRead,
            resource: "/etc/",
            rights: FileRights::READ,
        };
        let result = apply_grant_decision(GrantDecision::No, &scope, &mut denied, &[], &mut []);
        assert_eq!(result, ApplyResult::Denied);
        assert!(denied.is_denied("/etc/", ScopeKind::FileRead));
    }

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn grant_decision_session_records_intent_only() {
        let mut denied = DeniedScopes::new();
        let mut out = [0u8; 64];
        let scope = ScopeRequest {
            scope: ScopeKind::FileRead,
            resource: "/etc/",
            rights: FileRights::READ,
        };
        let result =
            apply_grant_decision(GrantDecision::Session, &scope, &mut denied, &[], &mut out);
        // Session records intent only — no bytes written, no denial recorded.
        assert_eq!(result, ApplyResult::SessionIntent);
        assert!(!denied.is_denied("/etc/", ScopeKind::FileRead));
    }

    #[cfg(feature = "arthur-grant-rerequest")]
    #[test]
    fn grant_decision_always_serializes_to_store() {
        let mut denied = DeniedScopes::new();
        let mut out = [0u8; 512];

        // Start with the D7 default entry.
        let mut existing = [GrantEntry::EMPTY; 1];
        let existing_count = default_grant_entries(&mut existing);

        let scope = ScopeRequest {
            scope: ScopeKind::FileRead,
            resource: "/home/user/project",
            rights: FileRights::READ,
        };
        let result = apply_grant_decision(
            GrantDecision::Always,
            &scope,
            &mut denied,
            &existing[..existing_count],
            &mut out,
        );

        match result {
            ApplyResult::Always(Some(n)) => {
                // Parse the serialized store back — the new entry must be present.
                let mut entries = [GrantEntry::EMPTY; MAX_GRANTS];
                let count =
                    parse_grants(&out[..n], &mut entries).expect("Always output must parse");
                // D7 default + the new grant = 2 entries.
                assert_eq!(count, 2);
                assert_eq!(entries[0].resource, ARTHUR_HOME); // D7 default
                assert_eq!(entries[1].scope, ScopeKind::FileRead);
                assert_eq!(entries[1].resource, "/home/user/project");
                assert_eq!(entries[1].rights, FileRights::READ);
                assert_eq!(entries[1].granted_by, "user");
            }
            _ => panic!("Always must serialize: {result:?}"),
        }

        // Always does NOT record a denial — the user granted it.
        assert!(!denied.is_denied("/home/user/project", ScopeKind::FileRead));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // OT-18 E3-audit — inspection surface host KATs
    // ═══════════════════════════════════════════════════════════════════════

    // ── ISC-1: grant-set listing ────────────────────────────────────────────

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn grant_listing_groups_by_scope_in_stable_order() {
        // Build entries in a non-sorted order (Network before FileRead).
        let entries = [
            GrantEntry {
                scope: ScopeKind::Network,
                resource: "api.example.com:443",
                rights: FileRights(0),
                granted_by: "user",
            },
            GrantEntry {
                scope: ScopeKind::FileRead,
                resource: "/home/project",
                rights: FileRights::READ,
                granted_by: "user",
            },
            GrantEntry {
                scope: ScopeKind::FileWrite,
                resource: "/home/notes",
                rights: FileRights::WRITE,
                granted_by: "user",
            },
        ];
        let mut listing = [GrantListingEntry {
            scope: ScopeKind::FileRead,
            resource: "",
            rights: FileRights(0),
            granted_by: "",
        }; 8];
        let n = build_grant_listing(&entries, &mut listing);
        assert_eq!(n, 3);
        // Stable order: FileRead, FileWrite, then Network — NOT the input order.
        assert_eq!(listing[0].scope, ScopeKind::FileRead);
        assert_eq!(listing[0].resource, "/home/project");
        assert_eq!(listing[1].scope, ScopeKind::FileWrite);
        assert_eq!(listing[2].scope, ScopeKind::Network);

        // granted_by (the OT-20 seam) is carried through.
        assert_eq!(listing[0].granted_by, "user");
    }

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn grant_listing_includes_d7_default() {
        let mut entries = [GrantEntry::EMPTY; 4];
        let n = build_grant_entries(GrantConfig::Enabled(None), &mut entries);
        let mut listing = [GrantListingEntry {
            scope: ScopeKind::FileRead,
            resource: "",
            rights: FileRights(0),
            granted_by: "",
        }; 4];
        let ln = build_grant_listing(&entries[..n], &mut listing);
        assert_eq!(ln, 1);
        assert_eq!(listing[0].scope, ScopeKind::FileRead);
        assert_eq!(listing[0].resource, ARTHUR_HOME);
        assert_eq!(listing[0].granted_by, "default");
    }

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn grant_listing_empty_set() {
        let mut listing = [GrantListingEntry {
            scope: ScopeKind::FileRead,
            resource: "",
            rights: FileRights(0),
            granted_by: "",
        }; 4];
        let n = build_grant_listing(&[], &mut listing);
        assert_eq!(n, 0);
    }

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn format_grant_listing_renders_human_readable() {
        let entries = [
            GrantEntry {
                scope: ScopeKind::FileRead,
                resource: ".arthur/",
                rights: FileRights::READ
                    .union(FileRights::LOOKUP)
                    .union(FileRights::READDIR),
                granted_by: "default",
            },
            GrantEntry {
                scope: ScopeKind::FileRead,
                resource: "/home/project",
                rights: FileRights::READ,
                granted_by: "user",
            },
            GrantEntry {
                scope: ScopeKind::FileWrite,
                resource: "/home/notes",
                rights: FileRights::WRITE,
                granted_by: "user",
            },
        ];
        let mut out = [0u8; 512];
        let n = format_grant_listing(&entries, &mut out).expect("buffer is large enough");
        let text = core::str::from_utf8(&out[..n]).expect("valid utf8");
        // Header with count.
        assert!(text.starts_with("Arthur grant set (3):\n"));
        // Grouped by scope — FileRead header + 2 entries, then FileWrite header.
        assert!(text.contains("  FileRead:\n"));
        assert!(text.contains("    .arthur/  rights=read|lookup|readdir  by=default\n"));
        assert!(text.contains("    /home/project  rights=read  by=user\n"));
        assert!(text.contains("  FileWrite:\n"));
        assert!(text.contains("    /home/notes  rights=write  by=user\n"));
    }

    // ── ISC-2: action log (in-memory ring) ──────────────────────────────────

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn action_log_records_and_lists_newest_first() {
        let mut log = ActionLog::new();
        log.record(ActionKind::ReadFile, "/hello.txt", true, "arthur");
        log.record(ActionKind::WriteFile, "/etc/", false, "arthur");
        log.record(ActionKind::ListDir, "/", true, "arthur");

        // Collect entries newest-first.
        let mut collected: Vec<ActionLogEntry> = Vec::new();
        log.for_each_newest_first(|e| {
            collected.push(*e);
            true
        });
        assert_eq!(collected.len(), 3);
        // Newest first: ListDir, WriteFile, ReadFile.
        assert_eq!(collected[0].kind, ActionKind::ListDir);
        assert_eq!(collected[0].resource_bytes(), b"/");
        assert!(collected[0].succeeded);
        assert_eq!(collected[1].kind, ActionKind::WriteFile);
        assert_eq!(collected[1].resource_bytes(), b"/etc/");
        assert!(!collected[1].succeeded);
        assert_eq!(collected[2].kind, ActionKind::ReadFile);
        assert_eq!(collected[2].resource_bytes(), b"/hello.txt");

        // Principal (OT-20 seam) is recorded.
        assert_eq!(collected[0].principal_bytes(), b"arthur");

        // Total counter is monotonic.
        assert_eq!(log.total_recorded(), 3);
        assert_eq!(log.retained(), 3);
    }

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn action_log_ring_eviction_when_full() {
        // Use a small capacity to make eviction observable. The real capacity
        // is MAX_ACTION_LOG_ENTRIES (64); here we fill past it.
        let mut log = ActionLog::new();
        // Record MAX + 5 actions — the oldest 5 should be evicted.
        for i in 0..(super::grant_store::MAX_ACTION_LOG_ENTRIES + 5) {
            let path = format!("/file{i}");
            log.record(ActionKind::ReadFile, &path, true, "arthur");
        }
        // Retained count is capped at capacity.
        assert_eq!(log.retained(), super::grant_store::MAX_ACTION_LOG_ENTRIES);
        // Total is the full count (monotonic — never resets).
        assert_eq!(
            log.total_recorded(),
            (super::grant_store::MAX_ACTION_LOG_ENTRIES + 5) as u64
        );

        // The newest entry should be the last recorded.
        let mut first: Option<ActionLogEntry> = None;
        log.for_each_newest_first(|e| {
            first = Some(*e);
            false
        });
        let first = first.expect("at least one entry");
        let expected_last = super::grant_store::MAX_ACTION_LOG_ENTRIES + 5 - 1;
        assert_eq!(
            first.resource_bytes(),
            format!("/file{expected_last}").as_bytes()
        );

        // The oldest retained entry should NOT be /file0 (evicted).
        let mut last: Option<ActionLogEntry> = None;
        log.for_each_newest_first(|e| {
            last = Some(*e);
            true
        });
        let last = last.expect("at least one entry");
        // The oldest retained is the 5th recorded (indices 0-4 were evicted).
        assert_eq!(last.resource_bytes(), b"/file5");
    }

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn action_log_format_renders_human_readable() {
        let mut log = ActionLog::new();
        log.record(ActionKind::ReadFile, "/hello.txt", true, "arthur");
        log.record(ActionKind::WriteFile, "/etc/", false, "arthur");

        let mut out = [0u8; 512];
        let n = log.format_log(&mut out).expect("buffer is large enough");
        let text = core::str::from_utf8(&out[..n]).expect("valid utf8");
        // Header.
        assert!(text.starts_with("Arthur action log (2 retained, 2 total):\n"));
        // Newest first: WriteFile (denied), then ReadFile (ok).
        assert!(text.contains("  [denied] WriteFile /etc/  by=arthur\n"));
        assert!(text.contains("  [ok] ReadFile /hello.txt  by=arthur\n"));
    }

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn action_log_empty_format() {
        let log = ActionLog::new();
        let mut out = [0u8; 128];
        let n = log.format_log(&mut out).expect("buffer is large enough");
        let text = core::str::from_utf8(&out[..n]).expect("valid utf8");
        assert!(text.starts_with("Arthur action log (0 retained, 0 total):\n"));
    }

    // ── ISC-4: kill-switch equality (off ≡ E1a/E2) ──────────────────────────
    //
    // When `arthur-grant-audit` is off, the audit types/functions are not
    // compiled at all — there is no listing, no log, no surface. This is
    // verified by the OFF build: the crate compiles without the feature and
    // the E1a/E2 tests pass unchanged (18 or 30 tests). The equality
    // regression is structural: the code does not exist when the feature is off.

    #[cfg(feature = "arthur-grant-audit")]
    #[test]
    fn audit_surface_compiles_only_with_feature() {
        // This test only exists when the feature is ON. The OFF build
        // compilation (18 E1a tests or 30 E1a+E2 tests, no audit code) is
        // the kill-switch equality proof — if the feature were not gating
        // correctly, the off-build would fail to compile or would carry
        // audit code it shouldn't.
        let log = ActionLog::new();
        assert_eq!(log.retained(), 0);
        assert_eq!(log.total_recorded(), 0);
    }
}
