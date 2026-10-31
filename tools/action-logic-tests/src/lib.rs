// SPDX-License-Identifier: MPL-2.0
//! Host tests for libneo's action dispatcher.

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

#[path = "../../../kernel/src/sched/net_dial_cap_logic.rs"]
mod net_dial_cap_logic;

#[path = "../../../kernel/src/sched/gpu_submit_cap_logic.rs"]
mod gpu_submit_cap_logic;

#[path = "../../../kernel/src/sched/gpu_compute_cap_logic.rs"]
mod gpu_compute_cap_logic;

#[cfg(test)]
mod tests {
    use super::action::{
        Action, ActionError, ActionKind, ActionOutcome, ActionSyscalls, Grant, RequiredCapability,
        TcpRights, execute_action_with,
    };
    use super::*;

    #[derive(Default)]
    struct MockSyscalls {
        calls: Vec<String>,
    }

    impl ActionSyscalls for MockSyscalls {
        fn open(&mut self, path: &str, rights: FileRights) -> Result<FileHandle, i64> {
            self.calls
                .push(format!("open:{path}:0x{:x}", rights.bits()));
            Ok(FileHandle(90))
        }

        fn read(&mut self, handle: FileHandle, out: &mut [u8]) -> i64 {
            self.calls.push(format!("read:{}", handle.0));
            let payload = b"hello";
            out[..payload.len()].copy_from_slice(payload);
            payload.len() as i64
        }

        fn write(&mut self, handle: FileHandle, data: &[u8]) -> i64 {
            self.calls
                .push(format!("write:{}:{}", handle.0, data.len()));
            data.len() as i64
        }

        fn readdir(&mut self, handle: FileHandle, _out: &mut [u8], cursor: &mut u64) -> i64 {
            self.calls.push(format!("readdir:{}", handle.0));
            *cursor = 16;
            16
        }

        fn close(&mut self, handle: FileHandle) -> i64 {
            self.calls.push(format!("close:{}", handle.0));
            0
        }

        fn spawn(&mut self, name: &str, handles: &[Handle]) -> Result<(u32, ProcessHandle), i64> {
            self.calls.push(format!("spawn:{name}:{}", handles.len()));
            Ok((7, ProcessHandle(8)))
        }

        fn kill(&mut self, handle: ProcessHandle) -> i64 {
            self.calls.push(format!("kill:{}", handle.0));
            0
        }

        fn inspect(&mut self, handle: ProcessHandle) -> Result<ProcessSnapshotV1, i64> {
            self.calls.push(format!("inspect:{}", handle.0));
            Ok(ProcessSnapshotV1 {
                pid: 7,
                exit_code: 0,
            })
        }

        fn wait(&mut self, handle: ProcessHandle, flags: u64) -> Result<i32, i64> {
            self.calls.push(format!("wait:{}:{flags}", handle.0));
            Ok(42)
        }

        fn net_dial(
            &mut self,
            cap: NetDialCap,
            host: &str,
            port: u16,
        ) -> Result<TcpSocketHandle, i64> {
            self.calls.push(format!("dial:{}:{host}:{port}", cap.0.0));
            Ok(TcpSocketHandle(31))
        }

        fn tcp_send(&mut self, handle: TcpSocketHandle, data: &[u8]) -> i64 {
            self.calls
                .push(format!("tcp_send:{}:{}", handle.0, data.len()));
            data.len() as i64
        }

        fn tcp_recv(&mut self, handle: TcpSocketHandle, out: &mut [u8]) -> i64 {
            self.calls.push(format!("tcp_recv:{}", handle.0));
            out[0] = b'x';
            1
        }

        fn infer(
            &mut self,
            model: ModelHandle,
            inputs: &[TensorHandle],
            outputs: &mut [TensorHandle],
            flags: u32,
        ) -> Result<usize, i64> {
            self.calls
                .push(format!("infer:{}:{}:{flags}", model.0, inputs.len()));
            outputs[0] = TensorHandle(77);
            Ok(1)
        }

        fn model_load(&mut self, path: &str, flags: u32) -> Result<ModelHandle, i64> {
            self.calls.push(format!("model_load:{path}:{flags}"));
            Ok(ModelHandle(55))
        }
    }

    fn expect_denied<'a>(
        action: Action<'a>,
        grants: &[Grant<'a>],
        action_kind: ActionKind,
        required: RequiredCapability<'a>,
    ) {
        let mut syscalls = MockSyscalls::default();
        let err = execute_action_with(&mut syscalls, grants, action).unwrap_err();
        assert_eq!(
            err,
            ActionError::Denied {
                action: action_kind,
                required
            }
        );
        assert!(syscalls.calls.is_empty());
    }

    #[test]
    fn denies_file_actions_before_syscall() {
        let mut read_buf = [0u8; 8];
        expect_denied(
            Action::ReadFile {
                path: "/hello.txt",
                out: &mut read_buf,
            },
            &[],
            ActionKind::ReadFile,
            RequiredCapability::File {
                path: "/hello.txt",
                rights: FileRights::READ,
            },
        );

        let read_only = [Grant::File {
            path: "/scratch",
            rights: FileRights::READ,
        }];
        expect_denied(
            Action::WriteFile {
                path: "/scratch",
                data: b"blocked",
            },
            &read_only,
            ActionKind::WriteFile,
            RequiredCapability::File {
                path: "/scratch",
                rights: FileRights::WRITE,
            },
        );

        let mut dir_buf = [0u8; 32];
        let mut cursor = 0;
        expect_denied(
            Action::ListDir {
                path: "/",
                out: &mut dir_buf,
                cursor: &mut cursor,
            },
            &[],
            ActionKind::ListDir,
            RequiredCapability::File {
                path: "/",
                rights: FileRights::LOOKUP.union(FileRights::READDIR),
            },
        );
    }

    #[test]
    fn denies_spawn_and_inherited_handle_before_syscall() {
        let inherited = [Handle(4)];
        expect_denied(
            Action::SpawnProcess {
                name: "worker",
                handles: &inherited,
            },
            &[],
            ActionKind::SpawnProcess,
            RequiredCapability::SpawnProcess { name: "worker" },
        );

        let spawn_only = [Grant::SpawnProcess { name: "worker" }];
        expect_denied(
            Action::SpawnProcess {
                name: "worker",
                handles: &inherited,
            },
            &spawn_only,
            ActionKind::SpawnProcess,
            RequiredCapability::InheritHandle(Handle(4)),
        );
    }

    #[test]
    fn denies_process_actions_before_syscall() {
        let handle = ProcessHandle(9);
        expect_denied(
            Action::KillProcess { handle },
            &[],
            ActionKind::KillProcess,
            RequiredCapability::Process {
                handle,
                rights: CapRights::new(meta_rights::USE, process_rights::KILL),
            },
        );
        expect_denied(
            Action::InspectProcess { handle },
            &[],
            ActionKind::InspectProcess,
            RequiredCapability::Process {
                handle,
                rights: CapRights::new(meta_rights::USE, process_rights::INSPECT),
            },
        );
        expect_denied(
            Action::WaitProcess { handle, flags: 0 },
            &[],
            ActionKind::WaitProcess,
            RequiredCapability::Process {
                handle,
                rights: CapRights::new(meta_rights::USE, process_rights::WAIT),
            },
        );
    }

    #[test]
    fn denies_net_infer_and_model_actions_before_syscall() {
        expect_denied(
            Action::NetDial {
                host: "example.test",
                port: 443,
            },
            &[],
            ActionKind::NetDial,
            RequiredCapability::NetDial {
                host: "example.test",
                port: 443,
            },
        );

        expect_denied(
            Action::NetSend {
                handle: TcpSocketHandle(3),
                data: b"x",
            },
            &[],
            ActionKind::NetSend,
            RequiredCapability::TcpSocket {
                handle: TcpSocketHandle(3),
                rights: TcpRights::WRITE,
            },
        );

        let mut net_buf = [0u8; 8];
        expect_denied(
            Action::NetRecv {
                handle: TcpSocketHandle(3),
                out: &mut net_buf,
            },
            &[],
            ActionKind::NetRecv,
            RequiredCapability::TcpSocket {
                handle: TcpSocketHandle(3),
                rights: TcpRights::READ,
            },
        );

        let inputs = [TensorHandle(11)];
        let mut outputs = [TensorHandle(0)];
        expect_denied(
            Action::Infer {
                model: ModelHandle(10),
                inputs: &inputs,
                outputs: &mut outputs,
                flags: 0,
            },
            &[],
            ActionKind::Infer,
            RequiredCapability::ModelUse(ModelHandle(10)),
        );

        let model_grant = [Grant::ModelUse(ModelHandle(10))];
        let mut outputs = [TensorHandle(0)];
        expect_denied(
            Action::Infer {
                model: ModelHandle(10),
                inputs: &inputs,
                outputs: &mut outputs,
                flags: 0,
            },
            &model_grant,
            ActionKind::Infer,
            RequiredCapability::TensorUse(TensorHandle(11)),
        );

        expect_denied(
            Action::LoadModel {
                path: "/neofs/model.neom",
                flags: 0,
            },
            &[],
            ActionKind::LoadModel,
            RequiredCapability::ModelLoad {
                path: "/neofs/model.neom",
            },
        );
    }

    #[test]
    fn granted_file_actions_map_to_existing_syscalls() {
        let grants = [
            Grant::File {
                path: "/hello.txt",
                rights: FileRights::READ,
            },
            Grant::File {
                path: "/scratch",
                rights: FileRights::WRITE,
            },
            Grant::File {
                path: "/",
                rights: FileRights::LOOKUP.union(FileRights::READDIR),
            },
        ];
        let mut syscalls = MockSyscalls::default();
        let mut read_buf = [0u8; 8];
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::ReadFile {
                    path: "/hello.txt",
                    out: &mut read_buf,
                },
            ),
            Ok(ActionOutcome::ReadFile { bytes: 5 })
        );
        assert_eq!(&read_buf[..5], b"hello");

        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::WriteFile {
                    path: "/scratch",
                    data: b"abc",
                },
            ),
            Ok(ActionOutcome::WriteFile { bytes: 3 })
        );

        let mut dir_buf = [0u8; 32];
        let mut cursor = 0;
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::ListDir {
                    path: "/",
                    out: &mut dir_buf,
                    cursor: &mut cursor,
                },
            ),
            Ok(ActionOutcome::ListDir { bytes: 16 })
        );
        assert_eq!(cursor, 16);
        assert_eq!(
            syscalls.calls,
            [
                "open:/hello.txt:0x1",
                "read:90",
                "close:90",
                "open:/scratch:0x2",
                "write:90:3",
                "close:90",
                "open:/:0xc",
                "readdir:90",
                "close:90",
            ]
        );
    }

    #[test]
    fn granted_spawn_process_net_infer_and_model_actions_map_to_syscalls() {
        let process = ProcessHandle(20);
        let channel = Handle(4);
        let tcp = TcpSocketHandle(30);
        let model = ModelHandle(40);
        let tensor = TensorHandle(41);
        let grants = [
            Grant::SpawnProcess { name: "worker" },
            Grant::InheritHandle(channel),
            Grant::Process {
                handle: process,
                rights: CapRights::new(
                    meta_rights::USE,
                    process_rights::KILL | process_rights::WAIT | process_rights::INSPECT,
                ),
            },
            Grant::NetDial {
                host: "example.test",
                port: 443,
                cap: NetDialCap(Handle(30)),
            },
            Grant::TcpSocket {
                handle: tcp,
                rights: TcpRights::READ.union(TcpRights::WRITE),
            },
            Grant::ModelUse(model),
            Grant::TensorUse(tensor),
            Grant::ModelLoad {
                path: "/neofs/model.neom",
            },
        ];
        let mut syscalls = MockSyscalls::default();

        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::SpawnProcess {
                    name: "worker",
                    handles: &[channel],
                },
            ),
            Ok(ActionOutcome::SpawnProcess {
                pid: 7,
                handle: ProcessHandle(8),
            })
        );
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::KillProcess { handle: process },
            ),
            Ok(ActionOutcome::KillProcess)
        );
        assert!(matches!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::InspectProcess { handle: process },
            ),
            Ok(ActionOutcome::InspectProcess { .. })
        ));
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::WaitProcess {
                    handle: process,
                    flags: 1,
                },
            ),
            Ok(ActionOutcome::WaitProcess { exit_code: 42 })
        );
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::NetDial {
                    host: "example.test",
                    port: 443,
                },
            ),
            Ok(ActionOutcome::NetDial {
                handle: TcpSocketHandle(31),
            })
        );
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::NetSend {
                    handle: tcp,
                    data: b"xx",
                },
            ),
            Ok(ActionOutcome::NetSend { bytes: 2 })
        );
        let mut rx = [0u8; 2];
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::NetRecv {
                    handle: tcp,
                    out: &mut rx,
                },
            ),
            Ok(ActionOutcome::NetRecv { bytes: 1 })
        );
        let inputs = [tensor];
        let mut outputs = [TensorHandle(0)];
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::Infer {
                    model,
                    inputs: &inputs,
                    outputs: &mut outputs,
                    flags: 7,
                },
            ),
            Ok(ActionOutcome::Infer { output_count: 1 })
        );
        assert_eq!(outputs[0], TensorHandle(77));
        assert_eq!(
            execute_action_with(
                &mut syscalls,
                &grants,
                Action::LoadModel {
                    path: "/neofs/model.neom",
                    flags: 2,
                },
            ),
            Ok(ActionOutcome::LoadModel {
                handle: ModelHandle(55),
            })
        );

        assert_eq!(
            syscalls.calls,
            [
                "spawn:worker:1",
                "kill:20",
                "inspect:20",
                "wait:20:1",
                "dial:30:example.test:443",
                "tcp_send:30:2",
                "tcp_recv:30",
                "infer:40:1:7",
                "model_load:/neofs/model.neom:2",
            ]
        );
    }

    #[test]
    fn net_dial_cap_scope_is_exact_and_dns_case_insensitive() {
        assert!(super::net_dial_cap_logic::scope_allows(
            "api.example.test",
            443,
            "API.EXAMPLE.TEST",
            443,
        ));
        assert!(!super::net_dial_cap_logic::scope_allows(
            "api.example.test",
            443,
            "other.example.test",
            443,
        ));
        assert!(!super::net_dial_cap_logic::scope_allows(
            "api.example.test",
            443,
            "api.example.test",
            8443,
        ));
    }

    #[test]
    fn gpu_submit_cap_authority_kill_switch_truth_table() {
        use super::gpu_submit_cap_logic::authority_allows;
        // Enforcement OFF (the `gpu-submit-cap` kill-switch default) ⇒ submit is
        // ambient/ungated, bit-for-bit today's behaviour, regardless of caps.
        assert!(authority_allows(false, false));
        assert!(authority_allows(false, true));
        // Enforcement ON ⇒ submit requires a matching GpuSubmitCap held.
        assert!(!authority_allows(true, false));
        assert!(authority_allows(true, true));
    }

    #[test]
    fn gpu_compute_cap_authority_kill_switch_truth_table() {
        // OT-63 GPU P1 Slice 2: the compute-submit authority (the Slice-5 target,
        // amendment 4 — defined but not wired pre-VT-d). Same binary shape as the
        // present-submit cap: enforcement OFF ⇒ ungated; ON ⇒ a matching
        // GpuComputeCap required. Mirrors the present-submit table byte-for-byte,
        // so the two must never drift.
        use super::gpu_compute_cap_logic::authority_allows as compute_allows;
        use super::gpu_submit_cap_logic::authority_allows as submit_allows;
        for enforce in [false, true] {
            for held in [false, true] {
                assert_eq!(
                    compute_allows(enforce, held),
                    submit_allows(enforce, held),
                    "compute/submit authority tables diverged at enforce={enforce} held={held}"
                );
            }
        }
        assert!(compute_allows(false, false));
        assert!(!compute_allows(true, false));
        assert!(compute_allows(true, true));
    }

    #[test]
    fn net_dial_cap_attenuation_never_widens() {
        assert!(super::net_dial_cap_logic::can_attenuate(
            "*",
            0,
            "api.example.test",
            443,
        ));
        assert!(!super::net_dial_cap_logic::can_attenuate(
            "api.example.test",
            443,
            "other.example.test",
            443,
        ));
        assert!(!super::net_dial_cap_logic::can_attenuate(
            "api.example.test",
            443,
            "*",
            0,
        ));
    }

    #[test]
    fn net_dial_cap_kill_switch_is_ambient_equality() {
        for matching_cap_held in [false, true] {
            assert!(super::net_dial_cap_logic::authority_allows(
                false,
                matching_cap_held,
            ));
        }
        assert!(!super::net_dial_cap_logic::authority_allows(true, false));
        assert!(super::net_dial_cap_logic::authority_allows(true, true));
    }
}
