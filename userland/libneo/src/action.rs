// SPDX-License-Identifier: MPL-2.0
//! Capability-bounded userland action dispatcher.
//!
//! Arthur's model output proposes actions. This module disposes those
//! proposals against the caller's explicit grant set before any syscall runs.

use crate::{
    CapRights, FileHandle, FileRights, Handle, ModelHandle, NetDialCap, OPEN_ROOT_TEMPORARY,
    ProcessHandle, ProcessSnapshotV1, TcpSocketHandle, TensorHandle, meta_rights, process_rights,
    sys_close, sys_infer_run, sys_inspect, sys_kill, sys_model_load, sys_net_dial, sys_open,
    sys_read, sys_readdir, sys_spawn, sys_tcp_recv, sys_tcp_send, sys_wait, sys_write,
};
#[cfg(feature = "action-observability")]
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ActionObservabilityV1 {
    pub execute_round_trips: u64,
    pub cap_checks: u64,
}

impl ActionObservabilityV1 {
    pub const fn empty() -> Self {
        Self {
            execute_round_trips: 0,
            cap_checks: 0,
        }
    }
}

#[cfg(feature = "action-observability")]
static ACTION_EXECUTE_ROUND_TRIPS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "action-observability")]
static ACTION_CAP_CHECKS: AtomicU64 = AtomicU64::new(0);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct TcpRights(pub u8);

impl TcpRights {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ActionKind {
    ReadFile,
    WriteFile,
    ListDir,
    SpawnProcess,
    KillProcess,
    InspectProcess,
    WaitProcess,
    NetDial,
    NetSend,
    NetRecv,
    Infer,
    LoadModel,
    CloseFile,
}

pub enum Action<'a> {
    ReadFile {
        path: &'a str,
        out: &'a mut [u8],
    },
    WriteFile {
        path: &'a str,
        data: &'a [u8],
    },
    ListDir {
        path: &'a str,
        out: &'a mut [u8],
        cursor: &'a mut u64,
    },
    SpawnProcess {
        name: &'a str,
        handles: &'a [Handle],
    },
    KillProcess {
        handle: ProcessHandle,
    },
    InspectProcess {
        handle: ProcessHandle,
    },
    WaitProcess {
        handle: ProcessHandle,
        flags: u64,
    },
    NetDial {
        host: &'a str,
        port: u16,
    },
    NetSend {
        handle: TcpSocketHandle,
        data: &'a [u8],
    },
    NetRecv {
        handle: TcpSocketHandle,
        out: &'a mut [u8],
    },
    Infer {
        model: ModelHandle,
        inputs: &'a [TensorHandle],
        outputs: &'a mut [TensorHandle],
        flags: u32,
    },
    LoadModel {
        path: &'a str,
        flags: u32,
    },
}

impl Action<'_> {
    pub const fn kind(&self) -> ActionKind {
        match self {
            Self::ReadFile { .. } => ActionKind::ReadFile,
            Self::WriteFile { .. } => ActionKind::WriteFile,
            Self::ListDir { .. } => ActionKind::ListDir,
            Self::SpawnProcess { .. } => ActionKind::SpawnProcess,
            Self::KillProcess { .. } => ActionKind::KillProcess,
            Self::InspectProcess { .. } => ActionKind::InspectProcess,
            Self::WaitProcess { .. } => ActionKind::WaitProcess,
            Self::NetDial { .. } => ActionKind::NetDial,
            Self::NetSend { .. } => ActionKind::NetSend,
            Self::NetRecv { .. } => ActionKind::NetRecv,
            Self::Infer { .. } => ActionKind::Infer,
            Self::LoadModel { .. } => ActionKind::LoadModel,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Grant<'a> {
    File {
        path: &'a str,
        rights: FileRights,
    },
    SpawnProcess {
        name: &'a str,
    },
    InheritHandle(Handle),
    Process {
        handle: ProcessHandle,
        rights: CapRights,
    },
    NetDial {
        host: &'a str,
        port: u16,
        cap: NetDialCap,
    },
    TcpSocket {
        handle: TcpSocketHandle,
        rights: TcpRights,
    },
    ModelLoad {
        path: &'a str,
    },
    ModelUse(ModelHandle),
    TensorUse(TensorHandle),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RequiredCapability<'a> {
    File {
        path: &'a str,
        rights: FileRights,
    },
    SpawnProcess {
        name: &'a str,
    },
    InheritHandle(Handle),
    Process {
        handle: ProcessHandle,
        rights: CapRights,
    },
    NetDial {
        host: &'a str,
        port: u16,
    },
    TcpSocket {
        handle: TcpSocketHandle,
        rights: TcpRights,
    },
    ModelLoad {
        path: &'a str,
    },
    ModelUse(ModelHandle),
    TensorUse(TensorHandle),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ActionError<'a> {
    Denied {
        action: ActionKind,
        required: RequiredCapability<'a>,
    },
    Syscall {
        action: ActionKind,
        errno: i64,
    },
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ActionOutcome {
    ReadFile { bytes: usize },
    WriteFile { bytes: usize },
    ListDir { bytes: usize },
    SpawnProcess { pid: u32, handle: ProcessHandle },
    KillProcess,
    InspectProcess { snapshot: ProcessSnapshotV1 },
    WaitProcess { exit_code: i32 },
    NetDial { handle: TcpSocketHandle },
    NetSend { bytes: usize },
    NetRecv { bytes: usize },
    Infer { output_count: usize },
    LoadModel { handle: ModelHandle },
}

pub trait ActionSyscalls {
    fn open(&mut self, path: &str, rights: FileRights) -> Result<FileHandle, i64>;
    fn read(&mut self, handle: FileHandle, out: &mut [u8]) -> i64;
    fn write(&mut self, handle: FileHandle, data: &[u8]) -> i64;
    fn readdir(&mut self, handle: FileHandle, out: &mut [u8], cursor: &mut u64) -> i64;
    fn close(&mut self, handle: FileHandle) -> i64;
    fn spawn(&mut self, name: &str, handles: &[Handle]) -> Result<(u32, ProcessHandle), i64>;
    fn kill(&mut self, handle: ProcessHandle) -> i64;
    fn inspect(&mut self, handle: ProcessHandle) -> Result<ProcessSnapshotV1, i64>;
    fn wait(&mut self, handle: ProcessHandle, flags: u64) -> Result<i32, i64>;
    fn net_dial(&mut self, cap: NetDialCap, host: &str, port: u16) -> Result<TcpSocketHandle, i64>;
    fn tcp_send(&mut self, handle: TcpSocketHandle, data: &[u8]) -> i64;
    fn tcp_recv(&mut self, handle: TcpSocketHandle, out: &mut [u8]) -> i64;
    fn infer(
        &mut self,
        model: ModelHandle,
        inputs: &[TensorHandle],
        outputs: &mut [TensorHandle],
        flags: u32,
    ) -> Result<usize, i64>;
    fn model_load(&mut self, path: &str, flags: u32) -> Result<ModelHandle, i64>;
}

pub struct LibneoActionSyscalls;

impl ActionSyscalls for LibneoActionSyscalls {
    fn open(&mut self, path: &str, rights: FileRights) -> Result<FileHandle, i64> {
        sys_open(path, OPEN_ROOT_TEMPORARY, rights)
    }

    fn read(&mut self, handle: FileHandle, out: &mut [u8]) -> i64 {
        sys_read(handle, out)
    }

    fn write(&mut self, handle: FileHandle, data: &[u8]) -> i64 {
        sys_write(handle.0, data.as_ptr(), data.len())
    }

    fn readdir(&mut self, handle: FileHandle, out: &mut [u8], cursor: &mut u64) -> i64 {
        sys_readdir(handle, out, cursor)
    }

    fn close(&mut self, handle: FileHandle) -> i64 {
        sys_close(handle)
    }

    fn spawn(&mut self, name: &str, handles: &[Handle]) -> Result<(u32, ProcessHandle), i64> {
        sys_spawn(name, handles)
    }

    fn kill(&mut self, handle: ProcessHandle) -> i64 {
        sys_kill(handle)
    }

    fn inspect(&mut self, handle: ProcessHandle) -> Result<ProcessSnapshotV1, i64> {
        sys_inspect(handle)
    }

    fn wait(&mut self, handle: ProcessHandle, flags: u64) -> Result<i32, i64> {
        sys_wait(handle, flags)
    }

    fn net_dial(&mut self, cap: NetDialCap, host: &str, port: u16) -> Result<TcpSocketHandle, i64> {
        sys_net_dial(cap, host, port)
    }

    fn tcp_send(&mut self, handle: TcpSocketHandle, data: &[u8]) -> i64 {
        sys_tcp_send(handle, data)
    }

    fn tcp_recv(&mut self, handle: TcpSocketHandle, out: &mut [u8]) -> i64 {
        sys_tcp_recv(handle, out)
    }

    fn infer(
        &mut self,
        model: ModelHandle,
        inputs: &[TensorHandle],
        outputs: &mut [TensorHandle],
        flags: u32,
    ) -> Result<usize, i64> {
        sys_infer_run(model, inputs, outputs, flags)
    }

    fn model_load(&mut self, path: &str, flags: u32) -> Result<ModelHandle, i64> {
        sys_model_load(path, flags)
    }
}

pub fn execute_action<'a>(
    grants: &[Grant<'a>],
    action: Action<'a>,
) -> Result<ActionOutcome, ActionError<'a>> {
    let mut syscalls = LibneoActionSyscalls;
    execute_action_with(&mut syscalls, grants, action)
}

pub fn execute_action_with<'a, S: ActionSyscalls>(
    syscalls: &mut S,
    grants: &[Grant<'a>],
    action: Action<'a>,
) -> Result<ActionOutcome, ActionError<'a>> {
    record_execute_round_trip();
    match action {
        Action::ReadFile { path, out } => {
            let rights = FileRights::READ;
            require(grants, ActionKind::ReadFile, file_required(path, rights))?;
            let file = syscalls
                .open(path, rights)
                .map_err(|errno| syscall_error(ActionKind::ReadFile, errno))?;
            let n = syscall_usize(ActionKind::ReadFile, syscalls.read(file, out));
            close_after(syscalls, file, n)?;
            n.map(|bytes| ActionOutcome::ReadFile { bytes })
        }
        Action::WriteFile { path, data } => {
            let rights = FileRights::WRITE;
            require(grants, ActionKind::WriteFile, file_required(path, rights))?;
            let file = syscalls
                .open(path, rights)
                .map_err(|errno| syscall_error(ActionKind::WriteFile, errno))?;
            let n = syscall_usize(ActionKind::WriteFile, syscalls.write(file, data));
            close_after(syscalls, file, n)?;
            n.map(|bytes| ActionOutcome::WriteFile { bytes })
        }
        Action::ListDir { path, out, cursor } => {
            let rights = FileRights::LOOKUP.union(FileRights::READDIR);
            require(grants, ActionKind::ListDir, file_required(path, rights))?;
            let file = syscalls
                .open(path, rights)
                .map_err(|errno| syscall_error(ActionKind::ListDir, errno))?;
            let n = syscall_usize(ActionKind::ListDir, syscalls.readdir(file, out, cursor));
            close_after(syscalls, file, n)?;
            n.map(|bytes| ActionOutcome::ListDir { bytes })
        }
        Action::SpawnProcess { name, handles } => {
            require(
                grants,
                ActionKind::SpawnProcess,
                RequiredCapability::SpawnProcess { name },
            )?;
            for handle in handles {
                require(
                    grants,
                    ActionKind::SpawnProcess,
                    RequiredCapability::InheritHandle(*handle),
                )?;
            }
            syscalls
                .spawn(name, handles)
                .map(|(pid, handle)| ActionOutcome::SpawnProcess { pid, handle })
                .map_err(|errno| syscall_error(ActionKind::SpawnProcess, errno))
        }
        Action::KillProcess { handle } => {
            let rights = process_required_right(process_rights::KILL);
            require(
                grants,
                ActionKind::KillProcess,
                process_required(handle, rights),
            )?;
            syscall_unit(ActionKind::KillProcess, syscalls.kill(handle))?;
            Ok(ActionOutcome::KillProcess)
        }
        Action::InspectProcess { handle } => {
            let rights = process_required_right(process_rights::INSPECT);
            require(
                grants,
                ActionKind::InspectProcess,
                process_required(handle, rights),
            )?;
            syscalls
                .inspect(handle)
                .map(|snapshot| ActionOutcome::InspectProcess { snapshot })
                .map_err(|errno| syscall_error(ActionKind::InspectProcess, errno))
        }
        Action::WaitProcess { handle, flags } => {
            let rights = process_required_right(process_rights::WAIT);
            require(
                grants,
                ActionKind::WaitProcess,
                process_required(handle, rights),
            )?;
            syscalls
                .wait(handle, flags)
                .map(|exit_code| ActionOutcome::WaitProcess { exit_code })
                .map_err(|errno| syscall_error(ActionKind::WaitProcess, errno))
        }
        Action::NetDial { host, port } => {
            require(
                grants,
                ActionKind::NetDial,
                RequiredCapability::NetDial { host, port },
            )?;
            let cap = grants.iter().find_map(|grant| match *grant {
                Grant::NetDial {
                    host: granted_host,
                    port: granted_port,
                    cap,
                } if granted_host == host && granted_port == port => Some(cap),
                _ => None,
            });
            let cap = match cap {
                Some(cap) => cap,
                None => {
                    return Err(ActionError::Denied {
                        action: ActionKind::NetDial,
                        required: RequiredCapability::NetDial { host, port },
                    });
                }
            };
            syscalls
                .net_dial(cap, host, port)
                .map(|handle| ActionOutcome::NetDial { handle })
                .map_err(|errno| syscall_error(ActionKind::NetDial, errno))
        }
        Action::NetSend { handle, data } => {
            require(
                grants,
                ActionKind::NetSend,
                RequiredCapability::TcpSocket {
                    handle,
                    rights: TcpRights::WRITE,
                },
            )?;
            syscall_usize(ActionKind::NetSend, syscalls.tcp_send(handle, data))
                .map(|bytes| ActionOutcome::NetSend { bytes })
        }
        Action::NetRecv { handle, out } => {
            require(
                grants,
                ActionKind::NetRecv,
                RequiredCapability::TcpSocket {
                    handle,
                    rights: TcpRights::READ,
                },
            )?;
            syscall_usize(ActionKind::NetRecv, syscalls.tcp_recv(handle, out))
                .map(|bytes| ActionOutcome::NetRecv { bytes })
        }
        Action::Infer {
            model,
            inputs,
            outputs,
            flags,
        } => {
            require(
                grants,
                ActionKind::Infer,
                RequiredCapability::ModelUse(model),
            )?;
            for input in inputs {
                require(
                    grants,
                    ActionKind::Infer,
                    RequiredCapability::TensorUse(*input),
                )?;
            }
            syscalls
                .infer(model, inputs, outputs, flags)
                .map(|output_count| ActionOutcome::Infer { output_count })
                .map_err(|errno| syscall_error(ActionKind::Infer, errno))
        }
        Action::LoadModel { path, flags } => {
            require(
                grants,
                ActionKind::LoadModel,
                RequiredCapability::ModelLoad { path },
            )?;
            syscalls
                .model_load(path, flags)
                .map(|handle| ActionOutcome::LoadModel { handle })
                .map_err(|errno| syscall_error(ActionKind::LoadModel, errno))
        }
    }
}

fn file_required(path: &str, rights: FileRights) -> RequiredCapability<'_> {
    RequiredCapability::File { path, rights }
}

fn process_required(handle: ProcessHandle, rights: CapRights) -> RequiredCapability<'static> {
    RequiredCapability::Process { handle, rights }
}

fn process_required_right(right: u8) -> CapRights {
    CapRights::new(meta_rights::USE, right)
}

fn require<'a>(
    grants: &[Grant<'a>],
    action: ActionKind,
    required: RequiredCapability<'a>,
) -> Result<(), ActionError<'a>> {
    record_cap_check();
    if grants.iter().any(|grant| grant.allows(&required)) {
        Ok(())
    } else {
        Err(ActionError::Denied { action, required })
    }
}

pub fn action_observability_snapshot() -> ActionObservabilityV1 {
    #[cfg(feature = "action-observability")]
    {
        return ActionObservabilityV1 {
            execute_round_trips: ACTION_EXECUTE_ROUND_TRIPS.load(Ordering::Relaxed),
            cap_checks: ACTION_CAP_CHECKS.load(Ordering::Relaxed),
        };
    }

    #[cfg(not(feature = "action-observability"))]
    {
        ActionObservabilityV1::empty()
    }
}

#[inline(always)]
fn record_execute_round_trip() {
    #[cfg(feature = "action-observability")]
    {
        ACTION_EXECUTE_ROUND_TRIPS.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(always)]
fn record_cap_check() {
    #[cfg(feature = "action-observability")]
    {
        ACTION_CAP_CHECKS.fetch_add(1, Ordering::Relaxed);
    }
}

impl Grant<'_> {
    fn allows(&self, required: &RequiredCapability<'_>) -> bool {
        match (*self, *required) {
            (
                Self::File { path, rights },
                RequiredCapability::File {
                    path: needed_path,
                    rights: needed_rights,
                },
            ) => path == needed_path && file_rights_include(rights, needed_rights),
            (
                Self::SpawnProcess { name },
                RequiredCapability::SpawnProcess { name: needed_name },
            ) => name == needed_name,
            (Self::InheritHandle(handle), RequiredCapability::InheritHandle(needed)) => {
                handle == needed
            }
            (
                Self::Process { handle, rights },
                RequiredCapability::Process {
                    handle: needed,
                    rights: needed_rights,
                },
            ) => handle == needed && cap_rights_include(rights, needed_rights),
            (
                Self::NetDial { host, port, .. },
                RequiredCapability::NetDial {
                    host: needed_host,
                    port: needed_port,
                },
            ) => host == needed_host && port == needed_port,
            (
                Self::TcpSocket { handle, rights },
                RequiredCapability::TcpSocket {
                    handle: needed,
                    rights: needed_rights,
                },
            ) => handle == needed && tcp_rights_include(rights, needed_rights),
            (Self::ModelLoad { path }, RequiredCapability::ModelLoad { path: needed_path }) => {
                path == needed_path
            }
            (Self::ModelUse(handle), RequiredCapability::ModelUse(needed)) => handle == needed,
            (Self::TensorUse(handle), RequiredCapability::TensorUse(needed)) => handle == needed,
            _ => false,
        }
    }
}

fn file_rights_include(granted: FileRights, required: FileRights) -> bool {
    (granted.bits() & required.bits()) == required.bits()
}

fn cap_rights_include(granted: CapRights, required: CapRights) -> bool {
    (granted.meta & required.meta) == required.meta
        && (granted.semantic & required.semantic) == required.semantic
}

fn tcp_rights_include(granted: TcpRights, required: TcpRights) -> bool {
    (granted.bits() & required.bits()) == required.bits()
}

fn close_after<'a, S: ActionSyscalls>(
    syscalls: &mut S,
    file: FileHandle,
    result: Result<usize, ActionError<'a>>,
) -> Result<(), ActionError<'a>> {
    let close = syscalls.close(file);
    if close < 0 && result.is_ok() {
        Err(syscall_error(ActionKind::CloseFile, close))
    } else {
        Ok(())
    }
}

fn syscall_unit<'a>(action: ActionKind, value: i64) -> Result<(), ActionError<'a>> {
    if value < 0 {
        Err(syscall_error(action, value))
    } else {
        Ok(())
    }
}

fn syscall_usize<'a>(action: ActionKind, value: i64) -> Result<usize, ActionError<'a>> {
    if value < 0 {
        Err(syscall_error(action, value))
    } else {
        Ok(value as usize)
    }
}

fn syscall_error<'a>(action: ActionKind, errno: i64) -> ActionError<'a> {
    ActionError::Syscall { action, errno }
}
