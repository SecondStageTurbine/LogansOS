// SPDX-License-Identifier: MPL-2.0
//! A1/A2/A3 agentic-action AST rendering and parsing.

extern crate alloc;

use alloc::string::String;
#[cfg(any(feature = "arthur-real-vocab", test))]
use alloc::vec::Vec;

const READ_PREFIX: &str = "<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"";
const WRITE_PREFIX: &str = "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"";
const WRITE_CONTENT_SEP: &str = "\",\"content\":\"";
const LIST_PREFIX: &str = "<tool_call>{\"name\":\"list_dir\",\"arguments\":{\"path\":\"";
const CLARIFY_PREFIX: &str = "<tool_call>{\"name\":\"clarify\",\"arguments\":{\"question\":\"";
const COMPACT_READ_PREFIX: &str = "ReadFile(\"";
const COMPACT_LIST_PREFIX: &str = "ListDir(\"";
const COMPACT_CD_PREFIX: &str = "cd(\"";
const COMPACT_CLARIFY_PREFIX: &str = "clarify(\"";
const COMPACT_FIELD_SUFFIX: &str = "\")";
const COMPACT_STATUS_AST: &str = "status";
const RESTRICTED_PREFIX: &str = "<tool_call>{\"name\":\"restricted\",\"arguments\":{\"reason\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const DRY_RUN_PREFIX: &str = "<tool_call>{\"name\":\"dry_run\",\"arguments\":{\"plan\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const CONFIRM_TYPED_PREFIX: &str =
    "<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const REFUSE_PREFIX: &str = "<tool_call>{\"name\":\"refuse\",\"arguments\":{\"reason\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const DONE_PREFIX: &str = "<tool_call>{\"name\":\"done\",\"arguments\":{\"summary\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const TRASH_PREFIX: &str = "<tool_call>{\"name\":\"trash_file\",\"arguments\":{\"path\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const MOVE_PREFIX: &str = "<tool_call>{\"name\":\"move_file\",\"arguments\":{\"path\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const MOVE_DESTINATION_SEP: &str = "\",\"destination\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const KILL_PREFIX: &str = "<tool_call>{\"name\":\"kill_process\",\"arguments\":{\"handle\":";
#[cfg(any(feature = "agentic-actions-a3", test))]
const SPAWN_PREFIX: &str = "<tool_call>{\"name\":\"spawn_process\",\"arguments\":{\"name\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const SPAWN_INHERIT_SEP: &str = "\",\"inherit_handle\":";
#[cfg(any(feature = "agentic-actions-a3", test))]
const EXTERNAL_SEND_PREFIX: &str =
    "<tool_call>{\"name\":\"external_send\",\"arguments\":{\"host\":\"";
#[cfg(any(feature = "agentic-actions-a3", test))]
const EXTERNAL_SEND_PORT_SEP: &str = "\",\"port\":";
#[cfg(any(feature = "agentic-actions-a3", test))]
const EXTERNAL_SEND_BYTES_SEP: &str = ",\"bytes\":";
#[cfg(any(feature = "agentic-actions-a3", test))]
const LOAD_MODEL_PREFIX: &str = "<tool_call>{\"name\":\"load_model\",\"arguments\":{\"path\":\"";
const FIELD_SUFFIX: &str = "\"}}</tool_call>";
#[cfg(any(feature = "agentic-actions-a3", test))]
const NUMBER_SUFFIX: &str = "}}</tool_call>";
const CONFIRM_AST: &str = "<tool_call>{\"name\":\"confirm\"}</tool_call>";
const UNDO_AST: &str = "<tool_call>{\"name\":\"undo\"}</tool_call>";
#[cfg(any(feature = "agentic-actions-a3", test))]
const CANCEL_AST: &str = "<tool_call>{\"name\":\"cancel\"}</tool_call>";
const NO_OP_AST: &str = "<tool_call>{\"name\":\"no_op\"}</tool_call>";
const MAX_AGENTIC_FIELD_BYTES: usize = 128;
#[cfg(any(feature = "arthur-real-vocab", test))]
const A1_CLARIFY_QUESTION: &str = "A1 can only read or list paths";
const A2_RESTRICTED_REASON: &str = "A2 restricts delete move kill spawn and external actions";
const DEFAULT_WRITE_CONTENT: &str = "NeoPlutarch scratch A2ok";
#[cfg(any(feature = "arthur-real-vocab", test))]
const DEFAULT_READ_PATHS: &[&str] = &["/hello.txt", "/missing.txt"];
#[cfg(any(feature = "arthur-real-vocab", test))]
const DEFAULT_LIST_PATHS: &[&str] = &["/"];
#[cfg(any(feature = "arthur-real-vocab", test))]
const DEFAULT_WRITE_PATHS: &[&str] = &["/scratch", "/forbidden"];
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_READ_HELLO_TOKEN: u32 = 300;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_READ_MISSING_TOKEN: u32 = 301;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_LIST_ROOT_TOKEN: u32 = 302;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_CLARIFY_INSPECT_TOKEN: u32 = 303;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_NO_OP_TOKEN: u32 = 304;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_CLARIFY_A1_TOKEN: u32 = 305;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_READ_ROOT_TOKEN: u32 = 306;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_WRITE_SCRATCH_TOKEN: u32 = 307;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_WRITE_FORBIDDEN_TOKEN: u32 = 308;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_CONFIRM_TOKEN: u32 = 309;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_UNDO_TOKEN: u32 = 310;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_RESTRICTED_A2_TOKEN: u32 = 311;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_READ_SCRATCH_TOKEN: u32 = 312;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_TRASH_SCRATCH_TOKEN: u32 = 313;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_MOVE_SCRATCH_TOKEN: u32 = 314;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_KILL_4242_TOKEN: u32 = 315;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_KILL_4243_TOKEN: u32 = 316;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_EXTERNAL_SEND_TOKEN: u32 = 317;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_SPAWN_INHERIT_TOKEN: u32 = 318;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_LOAD_MODEL_TOKEN: u32 = 319;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_TRASH_TOKEN: u32 = 320;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_MOVE_TOKEN: u32 = 321;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_KILL_4242_TOKEN: u32 = 322;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_KILL_4243_TOKEN: u32 = 323;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_SEND_TOKEN: u32 = 324;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_SPAWN_TOKEN: u32 = 325;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_MODEL_TOKEN: u32 = 326;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_CONFIRM_WRONG_TOKEN: u32 = 327;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_REFUSE_PROTECTED_TOKEN: u32 = 328;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_CD_ROOT_TOKEN: u32 = 329;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_STATUS_TOKEN: u32 = 330;
#[cfg(any(feature = "arthur-real-vocab", test))]
pub const AGENTIC_FIXTURE_WRITE_DANGER_KEY_TOKEN: u32 = 331;
#[cfg(any(
    all(feature = "arthur-real-vocab", feature = "agentic-actions-a3"),
    test
))]
pub const AGENTIC_FIXTURE_DONE_TOKEN: u32 = 332;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AgenticActionKind {
    ReadFile,
    WriteFile,
    ListDir,
    ChangeDir,
    Status,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    TrashFile,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    MoveFile,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    KillProcess,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    SpawnProcess,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    ExternalSend,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    LoadModel,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AgenticEffectTier {
    ReadOnly,
    TrashRecoverable,
    LocalDestructive,
    ExternalSend,
    Unpreviewable,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AgenticAction<'a> {
    ReadFile {
        path: &'a str,
    },
    WriteFile {
        path: &'a str,
        content: &'a str,
    },
    ListDir {
        path: &'a str,
    },
    ChangeDir {
        path: &'a str,
    },
    Status,
    Clarify {
        question: &'a str,
    },
    Restricted {
        reason: &'a str,
    },
    Confirm,
    Undo,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    DryRun {
        plan: &'a str,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    ConfirmTyped {
        phrase: &'a str,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    Cancel,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    Refuse {
        reason: &'a str,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    Done {
        summary: &'a str,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    TrashFile {
        path: &'a str,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    MoveFile {
        path: &'a str,
        destination: &'a str,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    KillProcess {
        handle: u32,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    SpawnProcess {
        name: &'a str,
        inherit_handle: u32,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    ExternalSend {
        host: &'a str,
        port: u16,
        bytes: usize,
    },
    #[cfg(any(feature = "agentic-actions-a3", test))]
    LoadModel {
        path: &'a str,
    },
    NoOp,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AgenticAstError {
    UnknownProduction,
    InvalidPath,
    InvalidContent,
    InvalidQuestion,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    InvalidName,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    InvalidHost,
    TooLarge,
}

pub fn parse_agentic_ast(ast: &str) -> Result<AgenticAction<'_>, AgenticAstError> {
    if let Some(path) = parse_compact_field(ast, COMPACT_READ_PREFIX) {
        require_valid_path(path)?;
        return Ok(AgenticAction::ReadFile { path });
    }
    if let Some(path) = parse_field(ast, READ_PREFIX) {
        require_valid_path(path)?;
        return Ok(AgenticAction::ReadFile { path });
    }
    if let Some((path, content)) = parse_write_fields(ast) {
        require_valid_path(path)?;
        require_valid_content(content)?;
        return Ok(AgenticAction::WriteFile { path, content });
    }
    if let Some(path) = parse_field(ast, LIST_PREFIX) {
        require_valid_path(path)?;
        return Ok(AgenticAction::ListDir { path });
    }
    if let Some(path) = parse_compact_field(ast, COMPACT_LIST_PREFIX) {
        require_valid_path(path)?;
        return Ok(AgenticAction::ListDir { path });
    }
    if let Some(path) = parse_compact_field(ast, COMPACT_CD_PREFIX) {
        require_valid_path(path)?;
        return Ok(AgenticAction::ChangeDir { path });
    }
    if ast == COMPACT_STATUS_AST {
        return Ok(AgenticAction::Status);
    }
    if let Some(question) = parse_compact_field(ast, COMPACT_CLARIFY_PREFIX) {
        require_valid_question(question)?;
        return Ok(AgenticAction::Clarify { question });
    }
    if let Some(question) = parse_field(ast, CLARIFY_PREFIX) {
        require_valid_question(question)?;
        return Ok(AgenticAction::Clarify { question });
    }
    if let Some(reason) = parse_field(ast, RESTRICTED_PREFIX) {
        require_valid_reason(reason)?;
        return Ok(AgenticAction::Restricted { reason });
    }
    if ast == CONFIRM_AST {
        return Ok(AgenticAction::Confirm);
    }
    if ast == UNDO_AST {
        return Ok(AgenticAction::Undo);
    }
    #[cfg(any(feature = "agentic-actions-a3", test))]
    {
        if let Some(plan) = parse_field(ast, DRY_RUN_PREFIX) {
            require_valid_phrase(plan)?;
            return Ok(AgenticAction::DryRun { plan });
        }
        if let Some(phrase) = parse_field(ast, CONFIRM_TYPED_PREFIX) {
            require_valid_phrase(phrase)?;
            return Ok(AgenticAction::ConfirmTyped { phrase });
        }
        if ast == CANCEL_AST {
            return Ok(AgenticAction::Cancel);
        }
        if let Some(reason) = parse_field(ast, REFUSE_PREFIX) {
            require_valid_reason(reason)?;
            return Ok(AgenticAction::Refuse { reason });
        }
        if let Some(summary) = parse_field(ast, DONE_PREFIX) {
            require_valid_reason(summary)?;
            return Ok(AgenticAction::Done { summary });
        }
        if let Some(path) = parse_field(ast, TRASH_PREFIX) {
            require_valid_path(path)?;
            return Ok(AgenticAction::TrashFile { path });
        }
        if let Some((path, destination)) = parse_move_fields(ast) {
            require_valid_path(path)?;
            require_valid_path(destination)?;
            return Ok(AgenticAction::MoveFile { path, destination });
        }
        if let Some(handle) = parse_number_field(ast, KILL_PREFIX) {
            return Ok(AgenticAction::KillProcess { handle });
        }
        if let Some((name, inherit_handle)) = parse_spawn_fields(ast) {
            require_valid_name(name)?;
            return Ok(AgenticAction::SpawnProcess {
                name,
                inherit_handle,
            });
        }
        if let Some((host, port, bytes)) = parse_external_send_fields(ast) {
            require_valid_host(host)?;
            return Ok(AgenticAction::ExternalSend { host, port, bytes });
        }
        if let Some(path) = parse_field(ast, LOAD_MODEL_PREFIX) {
            require_valid_path(path)?;
            return Ok(AgenticAction::LoadModel { path });
        }
    }
    if ast == NO_OP_AST {
        return Ok(AgenticAction::NoOp);
    }
    Err(AgenticAstError::UnknownProduction)
}

pub fn render_agentic_ast(
    action: AgenticAction<'_>,
    out: &mut String,
) -> Result<(), AgenticAstError> {
    out.clear();
    match action {
        AgenticAction::ReadFile { path } => {
            require_valid_path(path)?;
            render_compact_field(COMPACT_READ_PREFIX, path, out)
        }
        AgenticAction::WriteFile { path, content } => {
            require_valid_path(path)?;
            require_valid_content(content)?;
            render_write_field(path, content, out)
        }
        AgenticAction::ListDir { path } => {
            require_valid_path(path)?;
            render_compact_field(COMPACT_LIST_PREFIX, path, out)
        }
        AgenticAction::ChangeDir { path } => {
            require_valid_path(path)?;
            render_compact_field(COMPACT_CD_PREFIX, path, out)
        }
        AgenticAction::Status => {
            out.push_str(COMPACT_STATUS_AST);
            Ok(())
        }
        AgenticAction::Clarify { question } => {
            require_valid_question(question)?;
            render_compact_field(COMPACT_CLARIFY_PREFIX, question, out)
        }
        AgenticAction::Restricted { reason } => {
            require_valid_reason(reason)?;
            render_field(RESTRICTED_PREFIX, reason, out)
        }
        AgenticAction::Confirm => {
            out.push_str(CONFIRM_AST);
            Ok(())
        }
        AgenticAction::Undo => {
            out.push_str(UNDO_AST);
            Ok(())
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::DryRun { plan } => {
            require_valid_phrase(plan)?;
            render_field(DRY_RUN_PREFIX, plan, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::ConfirmTyped { phrase } => {
            require_valid_phrase(phrase)?;
            render_field(CONFIRM_TYPED_PREFIX, phrase, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::Cancel => {
            out.push_str(CANCEL_AST);
            Ok(())
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::Refuse { reason } => {
            require_valid_reason(reason)?;
            render_field(REFUSE_PREFIX, reason, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::Done { summary } => {
            require_valid_reason(summary)?;
            render_field(DONE_PREFIX, summary, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::TrashFile { path } => {
            require_valid_path(path)?;
            render_field(TRASH_PREFIX, path, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::MoveFile { path, destination } => {
            require_valid_path(path)?;
            require_valid_path(destination)?;
            render_move_field(path, destination, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::KillProcess { handle } => render_number_field(KILL_PREFIX, handle, out),
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::SpawnProcess {
            name,
            inherit_handle,
        } => {
            require_valid_name(name)?;
            render_spawn_field(name, inherit_handle, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::ExternalSend { host, port, bytes } => {
            require_valid_host(host)?;
            render_external_send_field(host, port, bytes, out)
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAction::LoadModel { path } => {
            require_valid_path(path)?;
            render_field(LOAD_MODEL_PREFIX, path, out)
        }
        AgenticAction::NoOp => {
            out.push_str(NO_OP_AST);
            Ok(())
        }
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
pub fn agentic_effect_tier(action: AgenticAction<'_>) -> AgenticEffectTier {
    match action {
        AgenticAction::ReadFile { .. }
        | AgenticAction::ListDir { .. }
        | AgenticAction::ChangeDir { .. }
        | AgenticAction::Status
        | AgenticAction::Clarify { .. }
        | AgenticAction::Restricted { .. }
        | AgenticAction::Confirm
        | AgenticAction::Undo
        | AgenticAction::DryRun { .. }
        | AgenticAction::ConfirmTyped { .. }
        | AgenticAction::Cancel
        | AgenticAction::Done { .. }
        | AgenticAction::NoOp => AgenticEffectTier::ReadOnly,
        AgenticAction::WriteFile { .. }
        | AgenticAction::TrashFile { .. }
        | AgenticAction::MoveFile { .. } => AgenticEffectTier::TrashRecoverable,
        AgenticAction::KillProcess { .. }
        | AgenticAction::SpawnProcess { .. }
        | AgenticAction::LoadModel { .. } => AgenticEffectTier::LocalDestructive,
        AgenticAction::ExternalSend { .. } => AgenticEffectTier::ExternalSend,
        AgenticAction::Refuse { .. } => AgenticEffectTier::Unpreviewable,
    }
}

pub fn fallback_agentic_action<'a>(
    user_text: &'a str,
    available_actions: &[AgenticActionKind],
) -> AgenticAction<'a> {
    let read_allowed = available_actions.contains(&AgenticActionKind::ReadFile);
    let write_allowed = available_actions.contains(&AgenticActionKind::WriteFile);
    let list_allowed = available_actions.contains(&AgenticActionKind::ListDir);
    let cd_allowed = available_actions.contains(&AgenticActionKind::ChangeDir);
    let status_allowed = available_actions.contains(&AgenticActionKind::Status);
    let path = explicit_path(user_text);
    #[cfg(any(feature = "agentic-actions-a3", test))]
    let a3_allowed = a3_actions_available(available_actions);
    #[cfg(any(feature = "agentic-actions-a3", test))]
    if a3_allowed {
        if let Some(phrase) = typed_confirm_phrase(user_text) {
            return AgenticAction::ConfirmTyped { phrase };
        }
        if wants_cancel_action(user_text) {
            return AgenticAction::Cancel;
        }
        if wants_done_action(user_text) {
            return AgenticAction::Done {
                summary: "multi-step complete",
            };
        }
        if wants_a3_refuse_action(user_text) {
            return AgenticAction::Refuse {
                reason: "A3 refuses broad protected or supervising-session actions",
            };
        }
        if wants_move_action(user_text) {
            return AgenticAction::MoveFile {
                path: path.unwrap_or("/scratch"),
                destination: "/trash/scratch",
            };
        }
        if wants_delete_or_trash_action(user_text) {
            return AgenticAction::TrashFile {
                path: path.unwrap_or("/scratch"),
            };
        }
        if wants_kill_action(user_text) {
            return AgenticAction::KillProcess {
                handle: explicit_u32(user_text).unwrap_or(4242),
            };
        }
        if wants_spawn_action(user_text) {
            return AgenticAction::SpawnProcess {
                name: explicit_name(user_text).unwrap_or("worker"),
                inherit_handle: if contains_word(user_text, "inherit") {
                    explicit_u32(user_text).unwrap_or(99)
                } else {
                    0
                },
            };
        }
        if wants_external_send_action(user_text) {
            return AgenticAction::ExternalSend {
                host: "api.example.com",
                port: 443,
                bytes: 16,
            };
        }
        if wants_model_load_action(user_text) {
            return AgenticAction::LoadModel {
                path: path.unwrap_or("/models/a3.neom"),
            };
        }
    }
    if wants_confirm_action(user_text) {
        return AgenticAction::Confirm;
    }
    if wants_undo_action(user_text) {
        return AgenticAction::Undo;
    }
    if wants_restricted_a3_action(user_text) {
        return AgenticAction::Clarify {
            question: A2_RESTRICTED_REASON,
        };
    }

    if wants_write_action(user_text) {
        if write_allowed {
            return AgenticAction::WriteFile {
                path: path.unwrap_or("/scratch"),
                content: DEFAULT_WRITE_CONTENT,
            };
        }
        return AgenticAction::Clarify {
            question: "Write file is not available",
        };
    }
    if wants_status_action(user_text) {
        if status_allowed {
            return AgenticAction::Status;
        }
        return AgenticAction::Clarify {
            question: "Status is not available",
        };
    }
    if wants_cd_action(user_text) {
        if cd_allowed {
            return AgenticAction::ChangeDir {
                path: path.unwrap_or("/"),
            };
        }
        return AgenticAction::Clarify {
            question: "Directory change is not available",
        };
    }
    if wants_list_action(user_text) {
        if list_allowed {
            return AgenticAction::ListDir {
                path: path.unwrap_or("/"),
            };
        }
        return AgenticAction::Clarify {
            question: "List directory is not available",
        };
    }
    if wants_read_action(user_text) || path.is_some() {
        if read_allowed {
            return AgenticAction::ReadFile {
                path: path.unwrap_or("/hello.txt"),
            };
        }
        return AgenticAction::Clarify {
            question: "Read file is not available",
        };
    }

    AgenticAction::NoOp
}

#[cfg(any(feature = "arthur-real-vocab", test))]
pub fn push_agentic_candidate_asts(
    user_text: &str,
    available_actions: &[AgenticActionKind],
    asts: &mut Vec<String>,
) -> Result<(), AgenticAstError> {
    #[cfg(any(feature = "agentic-actions-a3", test))]
    if a3_actions_available(available_actions)
        && let Some(phrase) = typed_confirm_phrase(user_text)
    {
        push_candidate_ast(AgenticAction::ConfirmTyped { phrase }, asts)?;
        return push_candidate_ast(AgenticAction::NoOp, asts);
    }
    if wants_confirm_action(user_text) {
        push_candidate_ast(AgenticAction::Confirm, asts)?;
        return push_candidate_ast(AgenticAction::NoOp, asts);
    }
    if wants_undo_action(user_text) {
        push_candidate_ast(AgenticAction::Undo, asts)?;
        return push_candidate_ast(AgenticAction::NoOp, asts);
    }
    #[cfg(any(feature = "agentic-actions-a3", test))]
    if a3_actions_available(available_actions) {
        if wants_cancel_action(user_text) {
            push_candidate_ast(AgenticAction::Cancel, asts)?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
        if wants_done_action(user_text) {
            push_candidate_ast(
                AgenticAction::Done {
                    summary: "multi-step complete",
                },
                asts,
            )?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
        if wants_a3_refuse_action(user_text) {
            return push_candidate_ast(
                AgenticAction::Refuse {
                    reason: "A3 refuses broad protected or supervising-session actions",
                },
                asts,
            );
        }
        let explicit = explicit_path(user_text).filter(|path| require_valid_path(path).is_ok());
        if wants_move_action(user_text) {
            push_candidate_ast(
                AgenticAction::MoveFile {
                    path: explicit.unwrap_or("/scratch"),
                    destination: "/trash/scratch",
                },
                asts,
            )?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
        if wants_delete_or_trash_action(user_text) {
            push_candidate_ast(
                AgenticAction::TrashFile {
                    path: explicit.unwrap_or("/scratch"),
                },
                asts,
            )?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
        if wants_kill_action(user_text) {
            push_candidate_ast(
                AgenticAction::KillProcess {
                    handle: explicit_u32(user_text).unwrap_or(4242),
                },
                asts,
            )?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
        if wants_spawn_action(user_text) {
            push_candidate_ast(
                AgenticAction::SpawnProcess {
                    name: explicit_name(user_text).unwrap_or("worker"),
                    inherit_handle: if contains_word(user_text, "inherit") {
                        explicit_u32(user_text).unwrap_or(99)
                    } else {
                        0
                    },
                },
                asts,
            )?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
        if wants_external_send_action(user_text) {
            push_candidate_ast(
                AgenticAction::ExternalSend {
                    host: "api.example.com",
                    port: 443,
                    bytes: 16,
                },
                asts,
            )?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
        if wants_model_load_action(user_text) {
            push_candidate_ast(
                AgenticAction::LoadModel {
                    path: explicit.unwrap_or("/models/a3.neom"),
                },
                asts,
            )?;
            return push_candidate_ast(AgenticAction::NoOp, asts);
        }
    }
    if wants_restricted_a3_action(user_text) {
        return push_candidate_ast(
            AgenticAction::Clarify {
                question: A2_RESTRICTED_REASON,
            },
            asts,
        );
    }

    let explicit = explicit_path(user_text).filter(|path| require_valid_path(path).is_ok());
    let read_allowed = available_actions.contains(&AgenticActionKind::ReadFile);
    let write_allowed = available_actions.contains(&AgenticActionKind::WriteFile);
    let list_allowed = available_actions.contains(&AgenticActionKind::ListDir);
    let cd_allowed = available_actions.contains(&AgenticActionKind::ChangeDir);
    let status_allowed = available_actions.contains(&AgenticActionKind::Status);

    if read_allowed {
        let mut paths = Vec::new();
        for path in DEFAULT_READ_PATHS {
            push_unique_path(&mut paths, path);
        }
        if let Some(path) = explicit {
            push_unique_path(&mut paths, path);
        }
        for path in paths {
            push_candidate_ast(AgenticAction::ReadFile { path }, asts)?;
        }
    }

    if list_allowed {
        let mut paths = Vec::new();
        for path in DEFAULT_LIST_PATHS {
            push_unique_path(&mut paths, path);
        }
        if let Some(path) = explicit {
            push_unique_path(&mut paths, path);
        }
        for path in paths {
            push_candidate_ast(AgenticAction::ListDir { path }, asts)?;
        }
    }

    if cd_allowed {
        let mut paths = Vec::new();
        for path in DEFAULT_LIST_PATHS {
            push_unique_path(&mut paths, path);
        }
        if let Some(path) = explicit {
            push_unique_path(&mut paths, path);
        }
        for path in paths {
            push_candidate_ast(AgenticAction::ChangeDir { path }, asts)?;
        }
    }

    if status_allowed {
        push_candidate_ast(AgenticAction::Status, asts)?;
    }

    if write_allowed {
        let mut paths = Vec::new();
        for path in DEFAULT_WRITE_PATHS {
            push_unique_path(&mut paths, path);
        }
        if let Some(path) = explicit {
            push_unique_path(&mut paths, path);
        }
        for path in paths {
            push_candidate_ast(
                AgenticAction::WriteFile {
                    path,
                    content: DEFAULT_WRITE_CONTENT,
                },
                asts,
            )?;
        }
    }

    push_candidate_ast(
        AgenticAction::Clarify {
            question: "Which path should I inspect, sir?",
        },
        asts,
    )?;
    push_candidate_ast(AgenticAction::NoOp, asts)
}

#[cfg(any(feature = "arthur-real-vocab", test))]
pub fn agentic_fixture_oracle_token_for_ast(ast: &str) -> Option<u32> {
    if ast == rendered_fixture_ast(AgenticAction::ReadFile { path: "/hello.txt" })? {
        return Some(AGENTIC_FIXTURE_READ_HELLO_TOKEN);
    }
    if ast
        == rendered_fixture_ast(AgenticAction::ReadFile {
            path: "/missing.txt",
        })?
    {
        return Some(AGENTIC_FIXTURE_READ_MISSING_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::ListDir { path: "/" })? {
        return Some(AGENTIC_FIXTURE_LIST_ROOT_TOKEN);
    }
    if ast
        == rendered_fixture_ast(AgenticAction::Clarify {
            question: "Which path should I inspect, sir?",
        })?
    {
        return Some(AGENTIC_FIXTURE_CLARIFY_INSPECT_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::NoOp)? {
        return Some(AGENTIC_FIXTURE_NO_OP_TOKEN);
    }
    if ast
        == rendered_fixture_ast(AgenticAction::Clarify {
            question: A1_CLARIFY_QUESTION,
        })?
    {
        return Some(AGENTIC_FIXTURE_CLARIFY_A1_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::ReadFile { path: "/" })? {
        return Some(AGENTIC_FIXTURE_READ_ROOT_TOKEN);
    }
    if ast
        == rendered_fixture_ast(AgenticAction::WriteFile {
            path: "/scratch",
            content: DEFAULT_WRITE_CONTENT,
        })?
    {
        return Some(AGENTIC_FIXTURE_WRITE_SCRATCH_TOKEN);
    }
    if ast
        == rendered_fixture_ast(AgenticAction::WriteFile {
            path: "/forbidden",
            content: DEFAULT_WRITE_CONTENT,
        })?
    {
        return Some(AGENTIC_FIXTURE_WRITE_FORBIDDEN_TOKEN);
    }
    if ast
        == rendered_fixture_ast(AgenticAction::WriteFile {
            path: "/neofs/danger.key",
            content: DEFAULT_WRITE_CONTENT,
        })?
    {
        return Some(AGENTIC_FIXTURE_WRITE_DANGER_KEY_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::Confirm)? {
        return Some(AGENTIC_FIXTURE_CONFIRM_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::Undo)? {
        return Some(AGENTIC_FIXTURE_UNDO_TOKEN);
    }
    if ast
        == rendered_fixture_ast(AgenticAction::Clarify {
            question: A2_RESTRICTED_REASON,
        })?
    {
        return Some(AGENTIC_FIXTURE_RESTRICTED_A2_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::ReadFile { path: "/scratch" })? {
        return Some(AGENTIC_FIXTURE_READ_SCRATCH_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::ChangeDir { path: "/" })? {
        return Some(AGENTIC_FIXTURE_CD_ROOT_TOKEN);
    }
    if ast == rendered_fixture_ast(AgenticAction::Status)? {
        return Some(AGENTIC_FIXTURE_STATUS_TOKEN);
    }
    #[cfg(any(feature = "agentic-actions-a3", test))]
    {
        if ast == rendered_fixture_ast(AgenticAction::TrashFile { path: "/scratch" })? {
            return Some(AGENTIC_FIXTURE_TRASH_SCRATCH_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::MoveFile {
                path: "/scratch",
                destination: "/trash/scratch",
            })?
        {
            return Some(AGENTIC_FIXTURE_MOVE_SCRATCH_TOKEN);
        }
        if ast == rendered_fixture_ast(AgenticAction::KillProcess { handle: 4242 })? {
            return Some(AGENTIC_FIXTURE_KILL_4242_TOKEN);
        }
        if ast == rendered_fixture_ast(AgenticAction::KillProcess { handle: 4243 })? {
            return Some(AGENTIC_FIXTURE_KILL_4243_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ExternalSend {
                host: "api.example.com",
                port: 443,
                bytes: 16,
            })?
        {
            return Some(AGENTIC_FIXTURE_EXTERNAL_SEND_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::SpawnProcess {
                name: "worker",
                inherit_handle: 99,
            })?
        {
            return Some(AGENTIC_FIXTURE_SPAWN_INHERIT_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::LoadModel {
                path: "/models/a3.neom",
            })?
        {
            return Some(AGENTIC_FIXTURE_LOAD_MODEL_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm trash /scratch",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_TRASH_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm move /scratch /trash/scratch",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_MOVE_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm kill 4242",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_KILL_4242_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm kill 4243",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_KILL_4243_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm send api.example.com:443",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_SEND_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm spawn worker 99",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_SPAWN_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm model /models/a3.neom",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_MODEL_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::ConfirmTyped {
                phrase: "confirm wrong phrase",
            })?
        {
            return Some(AGENTIC_FIXTURE_CONFIRM_WRONG_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::Refuse {
                reason: "A3 refuses broad protected or supervising-session actions",
            })?
        {
            return Some(AGENTIC_FIXTURE_REFUSE_PROTECTED_TOKEN);
        }
        if ast
            == rendered_fixture_ast(AgenticAction::Done {
                summary: "multi-step complete",
            })?
        {
            return Some(AGENTIC_FIXTURE_DONE_TOKEN);
        }
    }
    None
}

#[cfg(any(feature = "arthur-real-vocab", test))]
fn rendered_fixture_ast(action: AgenticAction<'_>) -> Option<String> {
    let mut ast = String::new();
    render_agentic_ast(action, &mut ast).ok()?;
    Some(ast)
}

#[cfg(any(feature = "arthur-real-vocab", test))]
fn push_candidate_ast(
    action: AgenticAction<'_>,
    asts: &mut Vec<String>,
) -> Result<(), AgenticAstError> {
    let mut ast = String::new();
    render_agentic_ast(action, &mut ast)?;
    if !asts.iter().any(|candidate| candidate == &ast) {
        asts.push(ast);
    }
    Ok(())
}

#[cfg(any(feature = "arthur-real-vocab", test))]
fn push_unique_path<'a>(paths: &mut Vec<&'a str>, path: &'a str) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn parse_field<'a>(ast: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = ast.strip_prefix(prefix)?;
    let field = rest.strip_suffix(FIELD_SUFFIX)?;
    Some(field)
}

fn parse_compact_field<'a>(ast: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = ast.strip_prefix(prefix)?;
    let field = rest.strip_suffix(COMPACT_FIELD_SUFFIX)?;
    Some(field)
}

fn parse_write_fields(ast: &str) -> Option<(&str, &str)> {
    let rest = ast.strip_prefix(WRITE_PREFIX)?;
    let rest = rest.strip_suffix(FIELD_SUFFIX)?;
    rest.split_once(WRITE_CONTENT_SEP)
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn parse_move_fields(ast: &str) -> Option<(&str, &str)> {
    let rest = ast.strip_prefix(MOVE_PREFIX)?;
    let rest = rest.strip_suffix(FIELD_SUFFIX)?;
    rest.split_once(MOVE_DESTINATION_SEP)
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn parse_number_field(ast: &str, prefix: &str) -> Option<u32> {
    let rest = ast.strip_prefix(prefix)?;
    let field = rest.strip_suffix(NUMBER_SUFFIX)?;
    parse_u32_decimal(field)
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn parse_spawn_fields(ast: &str) -> Option<(&str, u32)> {
    let rest = ast.strip_prefix(SPAWN_PREFIX)?;
    let rest = rest.strip_suffix(NUMBER_SUFFIX)?;
    let (name, inherit) = rest.split_once(SPAWN_INHERIT_SEP)?;
    parse_u32_decimal(inherit).map(|handle| (name, handle))
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn parse_external_send_fields(ast: &str) -> Option<(&str, u16, usize)> {
    let rest = ast.strip_prefix(EXTERNAL_SEND_PREFIX)?;
    let rest = rest.strip_suffix(NUMBER_SUFFIX)?;
    let (host, rest) = rest.split_once(EXTERNAL_SEND_PORT_SEP)?;
    let (port, bytes) = rest.split_once(EXTERNAL_SEND_BYTES_SEP)?;
    let port = parse_u32_decimal(port)?;
    let bytes = parse_u32_decimal(bytes)?;
    if port > u32::from(u16::MAX) {
        return None;
    }
    Some((host, port as u16, bytes as usize))
}

fn render_field(prefix: &str, field: &str, out: &mut String) -> Result<(), AgenticAstError> {
    if field.len() > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::TooLarge);
    }
    out.push_str(prefix);
    out.push_str(field);
    out.push_str(FIELD_SUFFIX);
    Ok(())
}

fn render_compact_field(
    prefix: &str,
    field: &str,
    out: &mut String,
) -> Result<(), AgenticAstError> {
    if field.len() > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::TooLarge);
    }
    out.push_str(prefix);
    out.push_str(field);
    out.push_str(COMPACT_FIELD_SUFFIX);
    Ok(())
}

fn render_write_field(path: &str, content: &str, out: &mut String) -> Result<(), AgenticAstError> {
    if path.len().saturating_add(content.len()) > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::TooLarge);
    }
    out.push_str(WRITE_PREFIX);
    out.push_str(path);
    out.push_str(WRITE_CONTENT_SEP);
    out.push_str(content);
    out.push_str(FIELD_SUFFIX);
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn render_move_field(
    path: &str,
    destination: &str,
    out: &mut String,
) -> Result<(), AgenticAstError> {
    if path.len().saturating_add(destination.len()) > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::TooLarge);
    }
    out.push_str(MOVE_PREFIX);
    out.push_str(path);
    out.push_str(MOVE_DESTINATION_SEP);
    out.push_str(destination);
    out.push_str(FIELD_SUFFIX);
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn render_number_field(prefix: &str, value: u32, out: &mut String) -> Result<(), AgenticAstError> {
    out.push_str(prefix);
    push_decimal_u32(out, value);
    out.push_str(NUMBER_SUFFIX);
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn render_spawn_field(
    name: &str,
    inherit_handle: u32,
    out: &mut String,
) -> Result<(), AgenticAstError> {
    out.push_str(SPAWN_PREFIX);
    out.push_str(name);
    out.push_str(SPAWN_INHERIT_SEP);
    push_decimal_u32(out, inherit_handle);
    out.push_str(NUMBER_SUFFIX);
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn render_external_send_field(
    host: &str,
    port: u16,
    bytes: usize,
    out: &mut String,
) -> Result<(), AgenticAstError> {
    if host.len() > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::TooLarge);
    }
    out.push_str(EXTERNAL_SEND_PREFIX);
    out.push_str(host);
    out.push_str(EXTERNAL_SEND_PORT_SEP);
    push_decimal_u32(out, u32::from(port));
    out.push_str(EXTERNAL_SEND_BYTES_SEP);
    push_decimal_usize(out, bytes);
    out.push_str(NUMBER_SUFFIX);
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn parse_u32_decimal(text: &str) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    let mut value = 0u32;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
    }
    Some(value)
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_decimal_u32(out: &mut String, value: u32) {
    push_decimal_usize(out, value as usize);
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_decimal_usize(out: &mut String, value: usize) {
    let mut buf = [0u8; 20];
    let mut n = value;
    let mut len = 0usize;
    if n == 0 {
        out.push('0');
        return;
    }
    while n > 0 {
        buf[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
    }
    while len > 0 {
        len -= 1;
        out.push(char::from(buf[len]));
    }
}

fn require_valid_path(path: &str) -> Result<(), AgenticAstError> {
    if path.is_empty() || path.len() > MAX_AGENTIC_FIELD_BYTES || !path.starts_with('/') {
        return Err(AgenticAstError::InvalidPath);
    }
    if path.split('/').any(|segment| segment == "..") {
        return Err(AgenticAstError::InvalidPath);
    }
    for byte in path.bytes() {
        let ok = byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'.');
        if !ok {
            return Err(AgenticAstError::InvalidPath);
        }
    }
    Ok(())
}

fn require_valid_content(content: &str) -> Result<(), AgenticAstError> {
    if content.is_empty() || content.len() > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::InvalidContent);
    }
    if content
        .bytes()
        .any(|byte| byte < b' ' || byte == b'"' || byte == b'\\' || byte == 0x7f)
    {
        return Err(AgenticAstError::InvalidContent);
    }
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn require_valid_phrase(phrase: &str) -> Result<(), AgenticAstError> {
    if phrase.is_empty() || phrase.len() > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::InvalidContent);
    }
    if phrase
        .bytes()
        .any(|byte| byte < b' ' || byte == b'"' || byte == b'\\' || byte == 0x7f)
    {
        return Err(AgenticAstError::InvalidContent);
    }
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn require_valid_name(name: &str) -> Result<(), AgenticAstError> {
    if name.is_empty() || name.len() > 64 {
        return Err(AgenticAstError::InvalidName);
    }
    for byte in name.bytes() {
        let ok = byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.');
        if !ok {
            return Err(AgenticAstError::InvalidName);
        }
    }
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn require_valid_host(host: &str) -> Result<(), AgenticAstError> {
    if host.is_empty() || host.len() > 64 {
        return Err(AgenticAstError::InvalidHost);
    }
    for byte in host.bytes() {
        let ok = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.');
        if !ok {
            return Err(AgenticAstError::InvalidHost);
        }
    }
    Ok(())
}

fn wants_write_action(text: &str) -> bool {
    contains_word(text, "write") || contains_word(text, "save") || contains_word(text, "update")
}

fn wants_restricted_a3_action(text: &str) -> bool {
    contains_word(text, "delete")
        || contains_word(text, "remove")
        || contains_word(text, "move")
        || contains_word(text, "kill")
        || contains_word(text, "spawn")
        || contains_word(text, "send")
        || contains_word(text, "network")
        || contains_word(text, "model")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn a3_actions_available(available_actions: &[AgenticActionKind]) -> bool {
    available_actions.contains(&AgenticActionKind::TrashFile)
        || available_actions.contains(&AgenticActionKind::KillProcess)
        || available_actions.contains(&AgenticActionKind::ExternalSend)
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_cancel_action(text: &str) -> bool {
    contains_word(text, "cancel") || contains_word(text, "refuse")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_delete_or_trash_action(text: &str) -> bool {
    contains_word(text, "delete") || contains_word(text, "remove") || contains_word(text, "trash")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_move_action(text: &str) -> bool {
    contains_word(text, "move") || contains_word(text, "rename")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_kill_action(text: &str) -> bool {
    contains_word(text, "kill") || contains_word(text, "terminate")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_spawn_action(text: &str) -> bool {
    contains_word(text, "spawn") || contains_word(text, "launch")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_external_send_action(text: &str) -> bool {
    contains_word(text, "send") || contains_word(text, "network") || contains_word(text, "external")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_model_load_action(text: &str) -> bool {
    (contains_word(text, "load") && contains_word(text, "model")) || contains_word(text, "model")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_a3_refuse_action(text: &str) -> bool {
    contains_word(text, "wildcard")
        || text.contains("/*")
        || text.contains("/.arthur")
        || text.contains(".arthur/")
        || contains_word(text, "supervisor")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_done_action(text: &str) -> bool {
    contains_word(text, "done") || contains_word(text, "complete")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn typed_confirm_phrase(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let phrase = trimmed
        .strip_prefix("confirm typed ")
        .or_else(|| trimmed.strip_prefix("confirm_typed "))?;
    let phrase = strip_oracle_marker(phrase.trim());
    if phrase.is_empty() {
        None
    } else {
        Some(phrase)
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn strip_oracle_marker(text: &str) -> &str {
    let Some((prefix, last)) = text.rsplit_once(' ') else {
        return text;
    };
    if last.len() == 1
        && matches!(
            last.as_bytes()[0],
            b'#' | b'$' | b'@' | b'%' | b'?' | b'&' | b'^' | b'*' | b'0'..=b'9'
        )
    {
        prefix.trim_end()
    } else {
        text
    }
}

fn wants_confirm_action(text: &str) -> bool {
    contains_word(text, "confirm") || contains_word(text, "proceed") || contains_word(text, "yes")
}

fn wants_undo_action(text: &str) -> bool {
    contains_word(text, "undo") || contains_word(text, "revert")
}

fn wants_list_action(text: &str) -> bool {
    contains_word(text, "list") || contains_word(text, "ls") || contains_word(text, "directory")
}

fn wants_cd_action(text: &str) -> bool {
    contains_word(text, "cd") || contains_word(text, "chdir")
}

fn wants_status_action(text: &str) -> bool {
    contains_word(text, "status") || contains_word(text, "state")
}

fn wants_read_action(text: &str) -> bool {
    contains_word(text, "read") || contains_word(text, "cat") || contains_word(text, "show")
}

fn contains_word(text: &str, needle: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|word| word.eq_ignore_ascii_case(needle))
}

fn explicit_path(text: &str) -> Option<&str> {
    text.split_ascii_whitespace()
        .map(|word| {
            word.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '"' | '\'' | ',' | '.' | '?' | '!' | ';' | ':' | '(' | ')'
                )
            })
        })
        .find(|word| word.starts_with('/'))
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn explicit_u32(text: &str) -> Option<u32> {
    text.split_ascii_whitespace()
        .map(trim_word)
        .find_map(parse_u32_decimal)
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn explicit_name(text: &str) -> Option<&str> {
    text.split_ascii_whitespace()
        .map(trim_word)
        .find(|word| require_valid_name(word).is_ok() && !is_reserved_intent_word(word))
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn trim_word(word: &str) -> &str {
    word.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | ','
                | '.'
                | '?'
                | '!'
                | ';'
                | ':'
                | '('
                | ')'
                | '#'
                | '$'
                | '@'
                | '%'
                | '&'
                | '^'
                | '*'
        )
    })
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn is_reserved_intent_word(word: &str) -> bool {
    word.eq_ignore_ascii_case("spawn")
        || word.eq_ignore_ascii_case("launch")
        || word.eq_ignore_ascii_case("kill")
        || word.eq_ignore_ascii_case("inherit")
        || word.eq_ignore_ascii_case("send")
        || word.eq_ignore_ascii_case("model")
        || word.eq_ignore_ascii_case("load")
}

fn require_valid_question(question: &str) -> Result<(), AgenticAstError> {
    if question.is_empty() || question.len() > MAX_AGENTIC_FIELD_BYTES {
        return Err(AgenticAstError::InvalidQuestion);
    }
    if question
        .bytes()
        .any(|byte| byte < b' ' || byte == b'"' || byte == b'\\' || byte == 0x7f)
    {
        return Err(AgenticAstError::InvalidQuestion);
    }
    Ok(())
}

fn require_valid_reason(reason: &str) -> Result<(), AgenticAstError> {
    require_valid_question(reason)
}
