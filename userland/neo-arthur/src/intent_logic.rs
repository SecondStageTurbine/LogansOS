// SPDX-License-Identifier: MPL-2.0
//! Hermes tool-call rendering and no_std intent extraction for Arthur.

extern crate alloc;

use alloc::string::String;

pub const MAX_TOOL_RETRIES: usize = 3;

const OPEN_TOOL_CALL: &str = "<tool_call>";
const OPEN_TOOL_CODE: &str = "<tool_code>";
const CLOSE_TOOL_CALL: &str = "</tool_call>";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ToolKind {
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
}

impl ToolKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "read_file" => Some(Self::ReadFile),
            "write_file" => Some(Self::WriteFile),
            "list_dir" => Some(Self::ListDir),
            "spawn_process" => Some(Self::SpawnProcess),
            "kill_process" => Some(Self::KillProcess),
            "inspect_process" => Some(Self::InspectProcess),
            "wait_process" => Some(Self::WaitProcess),
            "net_dial" => Some(Self::NetDial),
            "net_send" => Some(Self::NetSend),
            "net_recv" => Some(Self::NetRecv),
            "infer" => Some(Self::Infer),
            "load_model" => Some(Self::LoadModel),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ToolCall<'a> {
    ReadFile { path: &'a str },
    WriteFile { path: &'a str, content: &'a str },
    ListDir { path: &'a str },
    SpawnProcess { name: &'a str },
    KillProcess { handle: u32 },
    InspectProcess { handle: u32 },
    WaitProcess { handle: u32 },
    NetDial { host: &'a str, port: u16 },
    NetSend { handle: u32, data: &'a str },
    NetRecv { handle: u32, max_bytes: usize },
    Infer { model_handle: u32, input: &'a str },
    LoadModel { path: &'a str },
}

impl ToolCall<'_> {
    #[cfg(test)]
    pub const fn kind(&self) -> ToolKind {
        match self {
            Self::ReadFile { .. } => ToolKind::ReadFile,
            Self::WriteFile { .. } => ToolKind::WriteFile,
            Self::ListDir { .. } => ToolKind::ListDir,
            Self::SpawnProcess { .. } => ToolKind::SpawnProcess,
            Self::KillProcess { .. } => ToolKind::KillProcess,
            Self::InspectProcess { .. } => ToolKind::InspectProcess,
            Self::WaitProcess { .. } => ToolKind::WaitProcess,
            Self::NetDial { .. } => ToolKind::NetDial,
            Self::NetSend { .. } => ToolKind::NetSend,
            Self::NetRecv { .. } => ToolKind::NetRecv,
            Self::Infer { .. } => ToolKind::Infer,
            Self::LoadModel { .. } => ToolKind::LoadModel,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ToolParseError {
    MissingClose,
    InvalidJson,
    MissingAction,
    UnknownAction,
    MissingArguments,
    MissingField(&'static str),
    InvalidString(&'static str),
    InvalidInteger(&'static str),
}

pub fn render_hermes_prompt(tools: &[ToolKind], user_text: &str, out: &mut String) {
    out.clear();
    out.push_str("<|im_start|>system\n");
    out.push_str("You are Arthur on LogansOS. Answer plainly. ");
    out.push_str("When one tool is needed, emit exactly one ");
    out.push_str(OPEN_TOOL_CALL);
    out.push_str("{\"name\":\"tool_name\",\"arguments\":{...}}");
    out.push_str(CLOSE_TOOL_CALL);
    out.push_str(" block.\nAvailable tools:\n[");
    for (idx, tool) in tools.iter().copied().enumerate() {
        if idx != 0 {
            out.push(',');
        }
        render_tool_def(tool, out);
    }
    out.push_str("]\n<|im_end|>\n<|im_start|>user\n");
    out.push_str(user_text);
    out.push_str("\n<|im_end|>\n<|im_start|>assistant\n");
}

pub fn render_retry_prompt(
    tools: &[ToolKind],
    user_text: &str,
    error: ToolParseError,
    out: &mut String,
) {
    let mut retry_user = String::new();
    retry_user.push_str(user_text);
    retry_user.push_str("\n\nYour previous tool call was invalid: ");
    push_parse_error(error, &mut retry_user);
    retry_user.push_str(". Emit one valid ");
    retry_user.push_str(OPEN_TOOL_CALL);
    retry_user.push_str(" JSON object or answer in text.");
    render_hermes_prompt(tools, &retry_user, out);
}

pub fn extract_tool_call(output: &str) -> Result<Option<ToolCall<'_>>, ToolParseError> {
    let Some((open_start, open_len)) = find_open_delimiter(output) else {
        return Ok(None);
    };
    let after_open = &output[open_start + open_len..];
    let close_start = after_open
        .find(CLOSE_TOOL_CALL)
        .ok_or(ToolParseError::MissingClose)?;
    let payload = after_open[..close_start].trim();
    let object = first_object(payload).ok_or(ToolParseError::InvalidJson)?;
    parse_tool_object(object).map(Some)
}

pub fn push_parse_error(error: ToolParseError, out: &mut String) {
    match error {
        ToolParseError::MissingClose => out.push_str("missing </tool_call>"),
        ToolParseError::InvalidJson => out.push_str("invalid JSON"),
        ToolParseError::MissingAction => out.push_str("missing action name"),
        ToolParseError::UnknownAction => out.push_str("unknown action"),
        ToolParseError::MissingArguments => out.push_str("missing arguments object"),
        ToolParseError::MissingField(field) => {
            out.push_str("missing field ");
            out.push_str(field);
        }
        ToolParseError::InvalidString(field) => {
            out.push_str("invalid string field ");
            out.push_str(field);
        }
        ToolParseError::InvalidInteger(field) => {
            out.push_str("invalid integer field ");
            out.push_str(field);
        }
    }
}

fn render_tool_def(tool: ToolKind, out: &mut String) {
    match tool {
        ToolKind::ReadFile => out.push_str("{\"name\":\"read_file\",\"description\":\"Read a file\",\"parameters\":{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}}"),
        ToolKind::WriteFile => out.push_str("{\"name\":\"write_file\",\"description\":\"Write a file\",\"parameters\":{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"},\"content\":{\"type\":\"string\"}},\"required\":[\"path\",\"content\"]}}"),
        ToolKind::ListDir => out.push_str("{\"name\":\"list_dir\",\"description\":\"List a directory\",\"parameters\":{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}}"),
        ToolKind::SpawnProcess => out.push_str("{\"name\":\"spawn_process\",\"description\":\"Spawn a process\",\"parameters\":{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"}},\"required\":[\"name\"]}}"),
        ToolKind::KillProcess => render_handle_tool("kill_process", "Kill a process", out),
        ToolKind::InspectProcess => render_handle_tool("inspect_process", "Inspect a process", out),
        ToolKind::WaitProcess => render_handle_tool("wait_process", "Wait for a process", out),
        ToolKind::NetDial => out.push_str("{\"name\":\"net_dial\",\"description\":\"Open a TCP connection\",\"parameters\":{\"type\":\"object\",\"properties\":{\"host\":{\"type\":\"string\"},\"port\":{\"type\":\"integer\"}},\"required\":[\"host\",\"port\"]}}"),
        ToolKind::NetSend => out.push_str("{\"name\":\"net_send\",\"description\":\"Send TCP data\",\"parameters\":{\"type\":\"object\",\"properties\":{\"handle\":{\"type\":\"integer\"},\"data\":{\"type\":\"string\"}},\"required\":[\"handle\",\"data\"]}}"),
        ToolKind::NetRecv => out.push_str("{\"name\":\"net_recv\",\"description\":\"Receive TCP data\",\"parameters\":{\"type\":\"object\",\"properties\":{\"handle\":{\"type\":\"integer\"},\"max_bytes\":{\"type\":\"integer\"}},\"required\":[\"handle\",\"max_bytes\"]}}"),
        ToolKind::Infer => out.push_str("{\"name\":\"infer\",\"description\":\"Run model inference\",\"parameters\":{\"type\":\"object\",\"properties\":{\"model_handle\":{\"type\":\"integer\"},\"input\":{\"type\":\"string\"}},\"required\":[\"model_handle\",\"input\"]}}"),
        ToolKind::LoadModel => out.push_str("{\"name\":\"load_model\",\"description\":\"Load a model\",\"parameters\":{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}}"),
    }
}

fn render_handle_tool(name: &str, description: &str, out: &mut String) {
    out.push_str("{\"name\":\"");
    out.push_str(name);
    out.push_str("\",\"description\":\"");
    out.push_str(description);
    out.push_str("\",\"parameters\":{\"type\":\"object\",\"properties\":{\"handle\":{\"type\":\"integer\"}},\"required\":[\"handle\"]}}");
}

fn find_open_delimiter(output: &str) -> Option<(usize, usize)> {
    match (output.find(OPEN_TOOL_CALL), output.find(OPEN_TOOL_CODE)) {
        (Some(call), Some(code)) if call <= code => Some((call, OPEN_TOOL_CALL.len())),
        (Some(_call), Some(code)) => Some((code, OPEN_TOOL_CODE.len())),
        (Some(call), None) => Some((call, OPEN_TOOL_CALL.len())),
        (None, Some(code)) => Some((code, OPEN_TOOL_CODE.len())),
        (None, None) => None,
    }
}

fn parse_tool_object(object: &str) -> Result<ToolCall<'_>, ToolParseError> {
    let name = match field_value(object, "name").or_else(|| field_value(object, "action")) {
        Some(value) => parse_string(value).map_err(|_| ToolParseError::InvalidString("name"))?,
        None => return Err(ToolParseError::MissingAction),
    };
    let kind = ToolKind::from_name(name).ok_or(ToolParseError::UnknownAction)?;
    let args = match field_value(object, "arguments").or_else(|| field_value(object, "args")) {
        Some(value) if value.trim_start().starts_with('{') => value.trim(),
        Some(_) => return Err(ToolParseError::MissingArguments),
        None => return Err(ToolParseError::MissingArguments),
    };

    match kind {
        ToolKind::ReadFile => Ok(ToolCall::ReadFile {
            path: required_string(args, "path")?,
        }),
        ToolKind::WriteFile => Ok(ToolCall::WriteFile {
            path: required_string(args, "path")?,
            content: required_string(args, "content")?,
        }),
        ToolKind::ListDir => Ok(ToolCall::ListDir {
            path: required_string(args, "path")?,
        }),
        ToolKind::SpawnProcess => Ok(ToolCall::SpawnProcess {
            name: required_string(args, "name")?,
        }),
        ToolKind::KillProcess => Ok(ToolCall::KillProcess {
            handle: required_u32(args, "handle")?,
        }),
        ToolKind::InspectProcess => Ok(ToolCall::InspectProcess {
            handle: required_u32(args, "handle")?,
        }),
        ToolKind::WaitProcess => Ok(ToolCall::WaitProcess {
            handle: required_u32(args, "handle")?,
        }),
        ToolKind::NetDial => Ok(ToolCall::NetDial {
            host: required_string(args, "host")?,
            port: required_u16(args, "port")?,
        }),
        ToolKind::NetSend => Ok(ToolCall::NetSend {
            handle: required_u32(args, "handle")?,
            data: required_string(args, "data")?,
        }),
        ToolKind::NetRecv => Ok(ToolCall::NetRecv {
            handle: required_u32(args, "handle")?,
            max_bytes: required_usize(args, "max_bytes")?,
        }),
        ToolKind::Infer => Ok(ToolCall::Infer {
            model_handle: required_u32(args, "model_handle")?,
            input: required_string(args, "input")?,
        }),
        ToolKind::LoadModel => Ok(ToolCall::LoadModel {
            path: required_string(args, "path")?,
        }),
    }
}

fn required_string<'a>(object: &'a str, key: &'static str) -> Result<&'a str, ToolParseError> {
    let value = field_value(object, key).ok_or(ToolParseError::MissingField(key))?;
    parse_string(value).map_err(|_| ToolParseError::InvalidString(key))
}

fn required_u16(object: &str, key: &'static str) -> Result<u16, ToolParseError> {
    let value = required_u32(object, key)?;
    u16::try_from(value).map_err(|_| ToolParseError::InvalidInteger(key))
}

fn required_usize(object: &str, key: &'static str) -> Result<usize, ToolParseError> {
    let value = required_u32(object, key)?;
    usize::try_from(value).map_err(|_| ToolParseError::InvalidInteger(key))
}

fn required_u32(object: &str, key: &'static str) -> Result<u32, ToolParseError> {
    let value = field_value(object, key).ok_or(ToolParseError::MissingField(key))?;
    parse_u32(value).map_err(|_| ToolParseError::InvalidInteger(key))
}

fn first_object(payload: &str) -> Option<&str> {
    let trimmed = payload.trim();
    let start = if trimmed.starts_with('[') {
        trimmed.find('{')?
    } else if trimmed.starts_with('{') {
        0
    } else {
        return None;
    };
    let end = object_end(trimmed, start)?;
    Some(&trimmed[start..end])
}

fn field_value<'a>(object: &'a str, key: &str) -> Option<&'a str> {
    let object = object.trim();
    if !object.starts_with('{') {
        return None;
    }
    let mut idx = 1usize;
    while idx < object.len() {
        idx = skip_ws(object, idx);
        if object.as_bytes().get(idx) == Some(&b'}') {
            return None;
        }
        if object.as_bytes().get(idx) != Some(&b'"') {
            return None;
        }
        let key_end = string_end(object, idx)?;
        let parsed_key = &object[idx + 1..key_end];
        idx = skip_ws(object, key_end + 1);
        if object.as_bytes().get(idx) != Some(&b':') {
            return None;
        }
        idx = skip_ws(object, idx + 1);
        let value_end = value_end(object, idx)?;
        if parsed_key == key {
            return Some(object[idx..value_end].trim());
        }
        idx = skip_ws(object, value_end);
        match object.as_bytes().get(idx) {
            Some(b',') => idx += 1,
            Some(b'}') => return None,
            _ => return None,
        }
    }
    None
}

fn parse_string(value: &str) -> Result<&str, ()> {
    let value = value.trim();
    if !value.starts_with('"') {
        return Err(());
    }
    let end = string_end(value, 0).ok_or(())?;
    if skip_ws(value, end + 1) != value.len() {
        return Err(());
    }
    Ok(&value[1..end])
}

fn parse_u32(value: &str) -> Result<u32, ()> {
    let value = value.trim();
    let mut out = 0u32;
    let mut saw_digit = false;
    for byte in value.bytes() {
        if !byte.is_ascii_digit() {
            return Err(());
        }
        saw_digit = true;
        out = out
            .checked_mul(10)
            .and_then(|next| next.checked_add(u32::from(byte - b'0')))
            .ok_or(())?;
    }
    if saw_digit { Ok(out) } else { Err(()) }
}

fn value_end(text: &str, start: usize) -> Option<usize> {
    match text.as_bytes().get(start).copied()? {
        b'"' => string_end(text, start).map(|end| end + 1),
        b'{' => object_end(text, start),
        b'[' => array_end(text, start),
        _ => Some(scalar_end(text, start)),
    }
}

fn object_end(text: &str, start: usize) -> Option<usize> {
    balanced_end(text, start, b'{', b'}')
}

fn array_end(text: &str, start: usize) -> Option<usize> {
    balanced_end(text, start, b'[', b']')
}

fn balanced_end(text: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    if text.as_bytes().get(start) != Some(&open) {
        return None;
    }
    let mut depth = 0usize;
    let mut idx = start;
    while idx < text.len() {
        match text.as_bytes()[idx] {
            b'"' => idx = string_end(text, idx)?,
            byte if byte == open => depth += 1,
            byte if byte == close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(idx + 1);
                }
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

fn string_end(text: &str, start: usize) -> Option<usize> {
    if text.as_bytes().get(start) != Some(&b'"') {
        return None;
    }
    let mut idx = start + 1;
    while idx < text.len() {
        match text.as_bytes()[idx] {
            b'\\' => idx += 2,
            b'"' => return Some(idx),
            _ => idx += 1,
        }
    }
    None
}

fn scalar_end(text: &str, start: usize) -> usize {
    let mut idx = start;
    while idx < text.len() {
        match text.as_bytes()[idx] {
            b',' | b'}' | b']' => break,
            _ => idx += 1,
        }
    }
    skip_trailing_ws(text, start, idx)
}

fn skip_ws(text: &str, mut idx: usize) -> usize {
    while idx < text.len() && text.as_bytes()[idx].is_ascii_whitespace() {
        idx += 1;
    }
    idx
}

fn skip_trailing_ws(text: &str, start: usize, mut end: usize) -> usize {
    while end > start && text.as_bytes()[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}
