// SPDX-License-Identifier: MPL-2.0
//! Pure parsing and path logic for neo-shell.

pub const MAX_LINE_BYTES: usize = 160;
pub const MAX_PATH_BYTES: usize = 192;
pub const MAX_SPAWN_NAME_BYTES: usize = 32;

#[cfg(not(feature = "installer"))]
pub const HELP_TEXT: &[u8] = b"commands: help ls cat pwd cd echo status arthur spawn exit\n";
/// OT-87 R4.4: `install` is listed only where the shell can run it.
#[cfg(feature = "installer")]
pub const HELP_TEXT: &[u8] =
    b"commands: help ls cat pwd cd echo status arthur spawn install exit\n";
pub const SYSSTAT_V1_VERSION: u32 = 1;
#[cfg(test)]
pub const SYSSTAT_USER_HALF_LIMIT: u64 = 0x0000_8000_0000_0000;
#[cfg(test)]
pub const SYSSTAT_EFAULT: i64 = -14;
#[cfg(test)]
pub const SYSSTAT_EINVAL: i64 = -22;
pub const SYSSTAT_ENOSYS: i64 = -38;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ParsedCommand<'a> {
    Empty,
    Exit,
    Arthur,
    Spawn {
        name: &'a str,
    },
    Help,
    Ls {
        path: Option<&'a str>,
    },
    Cat {
        path: &'a str,
    },
    Pwd,
    Cd {
        path: &'a str,
    },
    Echo {
        text: &'a str,
    },
    Status,
    /// OT-87 R4.4: the console installer (THINK §4.5).
    #[cfg(feature = "installer")]
    Install,
    BadArgs {
        command: &'a str,
    },
    Unknown {
        command: &'a str,
    },
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SysStatV1 {
    pub version: u32,
    pub _pad0: u32,
    pub process_count: u64,
    pub frames_used: u64,
    pub memory_used_bytes: u64,
    pub cores_online: u32,
    pub _pad1: u32,
    pub uptime_ticks: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ObserveStatusV1 {
    pub cpu_count: u32,
    pub context_switches: u64,
    pub dispatches: u64,
    pub timer_preemptions: u64,
    pub voluntary_yields: u64,
    pub ready_queue_depth: u64,
    pub ready_queue_high_water: u64,
    pub run_ticks: u64,
    pub idle_ticks: u64,
    pub heap_current_bytes: u64,
    pub heap_high_water_bytes: u64,
    pub heap_alloc_failures: u64,
    pub frames_used: u64,
    pub frames_free: u64,
    pub buddy_largest_free_order: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PathError {
    Empty,
    Invalid,
    TooLong,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DirEntryKind {
    Directory,
    Regular,
    Symlink,
    Model,
    Other,
}

pub fn parse_command(input: &str) -> ParsedCommand<'_> {
    let input = trim_ascii_whitespace(input);
    if input.is_empty() {
        return ParsedCommand::Empty;
    }

    let (command, rest) = split_first_token(input);
    match command {
        "exit" if rest.is_empty() => ParsedCommand::Exit,
        "arthur" if rest.is_empty() => ParsedCommand::Arthur,
        "spawn" => match one_arg(rest) {
            Some(name) if valid_spawn_name(name) => ParsedCommand::Spawn { name },
            _ => ParsedCommand::Unknown { command },
        },
        "help" => no_args(command, rest, ParsedCommand::Help),
        "ls" => match optional_one_arg(rest) {
            Some(path) => ParsedCommand::Ls { path },
            None => ParsedCommand::BadArgs { command },
        },
        "cat" => match one_arg(rest) {
            Some(path) => ParsedCommand::Cat { path },
            None => ParsedCommand::BadArgs { command },
        },
        "pwd" => no_args(command, rest, ParsedCommand::Pwd),
        "cd" => match one_arg(rest) {
            Some(path) => ParsedCommand::Cd { path },
            None => ParsedCommand::BadArgs { command },
        },
        "echo" => ParsedCommand::Echo {
            text: trim_ascii_whitespace(rest),
        },
        "status" => no_args(command, rest, ParsedCommand::Status),
        #[cfg(feature = "installer")]
        "install" => no_args(command, rest, ParsedCommand::Install),
        other => ParsedCommand::Unknown { command: other },
    }
}

pub fn valid_spawn_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SPAWN_NAME_BYTES
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn resolve_path(cwd: &str, input: &str, out: &mut [u8]) -> Result<usize, PathError> {
    let input = trim_ascii_whitespace(input);
    if input.is_empty() {
        return Err(PathError::Empty);
    }
    if out.is_empty() {
        return Err(PathError::TooLong);
    }
    if has_invalid_path_byte(input) || has_invalid_path_byte(cwd) {
        return Err(PathError::Invalid);
    }

    let mut len = 1usize;
    out[0] = b'/';
    if !input.starts_with('/') {
        len = copy_normalized_cwd(cwd, out)?;
    }

    for component in input.split('/') {
        apply_component(out, &mut len, component)?;
    }
    Ok(len)
}

pub fn format_dir_entry(name: &[u8], kind: DirEntryKind, out: &mut [u8]) -> Option<usize> {
    let suffix_len = if kind == DirEntryKind::Directory {
        2
    } else {
        1
    };
    let needed = name.len().checked_add(suffix_len)?;
    if needed > out.len() {
        return None;
    }
    out[..name.len()].copy_from_slice(name);
    let mut len = name.len();
    if kind == DirEntryKind::Directory {
        out[len] = b'/';
        len += 1;
    }
    out[len] = b'\n';
    Some(len + 1)
}

pub fn path_error_message(error: PathError) -> &'static [u8] {
    match error {
        PathError::Empty => b"empty path",
        PathError::Invalid => b"invalid path",
        PathError::TooLong => b"path too long",
    }
}

pub fn render_status(stats: SysStatV1, out: &mut [u8]) -> Option<usize> {
    let mut writer = FixedWriter::new(out);
    writer.bytes(b"processes: ")?;
    writer.u64(stats.process_count)?;
    writer.bytes(b"   memory: ")?;
    writer.u64(stats.memory_used_bytes / 1024)?;
    writer.bytes(b" KiB   cores: ")?;
    writer.u64(stats.cores_online as u64)?;
    writer.bytes(b" online   uptime: ")?;
    writer.u64(stats.uptime_ticks)?;
    writer.bytes(b" ticks\n")?;
    Some(writer.len())
}

pub fn render_observe_status(stats: ObserveStatusV1, out: &mut [u8]) -> Option<usize> {
    let mut writer = FixedWriter::new(out);
    writer.bytes(b"scheduler: cpus=")?;
    writer.u64(stats.cpu_count as u64)?;
    writer.bytes(b" ctx=")?;
    writer.u64(stats.context_switches)?;
    writer.bytes(b" dispatch=")?;
    writer.u64(stats.dispatches)?;
    writer.bytes(b" preempt=")?;
    writer.u64(stats.timer_preemptions)?;
    writer.bytes(b" yield=")?;
    writer.u64(stats.voluntary_yields)?;
    writer.bytes(b" rq=")?;
    writer.u64(stats.ready_queue_depth)?;
    writer.bytes(b"/")?;
    writer.u64(stats.ready_queue_high_water)?;
    writer.bytes(b" ticks=")?;
    writer.u64(stats.run_ticks)?;
    writer.bytes(b"/")?;
    writer.u64(stats.idle_ticks)?;
    writer.bytes(b"\n")?;
    writer.bytes(b"memory pressure: heap=")?;
    writer.u64(stats.heap_current_bytes / 1024)?;
    writer.bytes(b"/")?;
    writer.u64(stats.heap_high_water_bytes / 1024)?;
    writer.bytes(b" KiB frames=")?;
    writer.u64(stats.frames_used)?;
    writer.bytes(b"/")?;
    writer.u64(stats.frames_free)?;
    writer.bytes(b" alloc_fail=")?;
    writer.u64(stats.heap_alloc_failures)?;
    writer.bytes(b" buddy_order=")?;
    writer.u64(stats.buddy_largest_free_order as u64)?;
    writer.bytes(b"\n")?;
    Some(writer.len())
}

pub fn status_error_message(errno: i64) -> Option<&'static [u8]> {
    match errno {
        SYSSTAT_ENOSYS => Some(b"observability disabled"),
        _ => None,
    }
}

#[cfg(test)]
pub fn validate_sysstat_buffer_shape(addr: u64, len: usize) -> Result<(), i64> {
    if addr == 0 {
        return Err(SYSSTAT_EINVAL);
    }
    if len != core::mem::size_of::<SysStatV1>() {
        return Err(SYSSTAT_EINVAL);
    }
    if addr >= SYSSTAT_USER_HALF_LIMIT {
        return Err(SYSSTAT_EFAULT);
    }
    let end = addr.checked_add(len as u64).ok_or(SYSSTAT_EFAULT)?;
    if end > SYSSTAT_USER_HALF_LIMIT {
        return Err(SYSSTAT_EFAULT);
    }
    Ok(())
}

fn no_args<'a>(command: &'a str, rest: &'a str, parsed: ParsedCommand<'a>) -> ParsedCommand<'a> {
    if rest.is_empty() {
        parsed
    } else {
        ParsedCommand::BadArgs { command }
    }
}

fn one_arg(rest: &str) -> Option<&str> {
    let rest = trim_ascii_whitespace(rest);
    let (arg, tail) = split_first_token(rest);
    if !arg.is_empty() && tail.is_empty() {
        Some(arg)
    } else {
        None
    }
}

fn optional_one_arg(rest: &str) -> Option<Option<&str>> {
    let rest = trim_ascii_whitespace(rest);
    if rest.is_empty() {
        return Some(None);
    }
    one_arg(rest).map(Some)
}

fn split_first_token(input: &str) -> (&str, &str) {
    let bytes = input.as_bytes();
    let mut end = 0usize;
    while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
        end += 1;
    }
    let token = &input[..end];
    let rest = trim_ascii_whitespace(&input[end..]);
    (token, rest)
}

fn trim_ascii_whitespace(input: &str) -> &str {
    let bytes = input.as_bytes();
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &input[start..end]
}

fn has_invalid_path_byte(path: &str) -> bool {
    path.bytes().any(|b| b == 0 || b.is_ascii_control())
}

fn copy_normalized_cwd(cwd: &str, out: &mut [u8]) -> Result<usize, PathError> {
    if !cwd.starts_with('/') {
        return Err(PathError::Invalid);
    }
    if cwd.len() > out.len() {
        return Err(PathError::TooLong);
    }
    out[..cwd.len()].copy_from_slice(cwd.as_bytes());
    let mut len = cwd.len();
    while len > 1 && out[len - 1] == b'/' {
        len -= 1;
    }
    Ok(len)
}

fn apply_component(out: &mut [u8], len: &mut usize, component: &str) -> Result<(), PathError> {
    match component {
        "" | "." => Ok(()),
        ".." => {
            pop_component(out, len);
            Ok(())
        }
        _ => push_component(out, len, component.as_bytes()),
    }
}

fn pop_component(out: &mut [u8], len: &mut usize) {
    if *len <= 1 {
        *len = 1;
        out[0] = b'/';
        return;
    }
    while *len > 1 && out[*len - 1] != b'/' {
        *len -= 1;
    }
    if *len > 1 {
        *len -= 1;
    }
}

fn push_component(out: &mut [u8], len: &mut usize, component: &[u8]) -> Result<(), PathError> {
    let slash = usize::from(*len > 1);
    let needed = len
        .checked_add(slash)
        .and_then(|n| n.checked_add(component.len()))
        .ok_or(PathError::TooLong)?;
    if needed > out.len() {
        return Err(PathError::TooLong);
    }
    if slash == 1 {
        out[*len] = b'/';
        *len += 1;
    }
    out[*len..*len + component.len()].copy_from_slice(component);
    *len += component.len();
    Ok(())
}

struct FixedWriter<'a> {
    out: &'a mut [u8],
    len: usize,
}

impl<'a> FixedWriter<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Self { out, len: 0 }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn bytes(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.len.checked_add(bytes.len())?;
        if end > self.out.len() {
            return None;
        }
        self.out[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Some(())
    }

    fn u64(&mut self, mut value: u64) -> Option<()> {
        let mut digits = [0u8; 20];
        let mut n = 0usize;
        if value == 0 {
            digits[0] = b'0';
            n = 1;
        } else {
            while value > 0 {
                digits[n] = b'0' + (value % 10) as u8;
                value /= 10;
                n += 1;
            }
        }
        while n > 0 {
            n -= 1;
            self.bytes(&digits[n..n + 1])?;
        }
        Some(())
    }
}
