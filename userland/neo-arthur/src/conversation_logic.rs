// SPDX-License-Identifier: MPL-2.0
//! Host-testable Arthur conversation relay logic.

extern crate alloc;

#[cfg(any(
    feature = "agentic-actions",
    all(feature = "arthur-act", not(feature = "agentic-actions")),
    test
))]
use alloc::string::String;
use alloc::vec::Vec;

#[cfg(any(feature = "agentic-actions", test))]
use crate::agentic_action_logic::{
    AgenticAction, AgenticActionKind, AgenticAstError, parse_agentic_ast,
};
#[cfg(any(feature = "agentic-actions-a3", test))]
use crate::agentic_action_logic::{AgenticEffectTier, agentic_effect_tier};
#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
use crate::intent_logic::{
    MAX_TOOL_RETRIES, ToolCall, ToolKind, ToolParseError, extract_tool_call, push_parse_error,
    render_hermes_prompt, render_retry_prompt,
};

// OT-37: must hold a full Qwen chat-templated prompt (control tokens + system
// turn + user turn ≈ 30 tokens) PLUS the generated reply without the rolling
// window dropping the framing. At 32 the framed prompt was truncated to its tail
// (BOS + the system turn fell off the front), so the model never saw a
// well-formed turn. 512 gives ample single-turn headroom (the forward still runs
// only over the live context length, not the window size).
pub const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 512;
/// The longest reply. 256 bytes where the reply crosses a channel to a
/// 256-byte reader (the console relays and shell).
#[cfg(not(feature = "desktop-session"))]
pub const MAX_REPLY_BYTES: usize = 256;
/// The desktop draws the reply on Arthur's own surface: 256 new tokens at up
/// to 16 bytes each.
#[cfg(feature = "desktop-session")]
pub const MAX_REPLY_BYTES: usize = 4096;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ReplyMode {
    DummyEcho,
    Model,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConversationError {
    InvalidUtf8,
    EmptyPrompt,
    #[cfg(any(not(feature = "agentic-actions"), test))]
    Tokenize,
    #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab", test))]
    Generate,
    Decode,
    #[cfg(any(feature = "arthur-act", test))]
    Action,
    ReplyTooLarge,
}

#[cfg(not(feature = "agentic-actions"))]
pub trait ConversationBackend {
    fn tokenize(&mut self, input: &str, out: &mut Vec<u32>) -> Result<(), ConversationError>;
    fn generate(&mut self, context: &[u32], out: &mut Vec<u32>) -> Result<(), ConversationError>;
    fn decode(&mut self, tokens: &[u32], out: &mut Vec<u8>) -> Result<(), ConversationError>;
}

#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
pub trait ToolActionExecutor {
    fn available_tools(&self) -> &[ToolKind];
    fn execute_tool_call(
        &mut self,
        call: ToolCall<'_>,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError>;
}

#[cfg(any(feature = "agentic-actions", test))]
pub trait AgenticActionDecoder {
    fn decode_agentic_action(
        &mut self,
        state: &mut ConversationState,
        user_text: &str,
        available_actions: &[AgenticActionKind],
        out: &mut Vec<u8>,
    ) -> Result<(), ConversationError>;
}

#[cfg(any(feature = "agentic-actions", test))]
pub trait AgenticActionExecutor {
    fn available_agentic_actions(&self) -> &[AgenticActionKind];
    fn execute_agentic_action(
        &mut self,
        action: AgenticAction<'_>,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError>;
    fn execute_confirmed_agentic_action(
        &mut self,
        action: AgenticAction<'_>,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        self.execute_agentic_action(action, reply)
    }
}

#[cfg(any(feature = "agentic-actions", test))]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PendingAgenticMutation {
    path: String,
    content: String,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
const AGENTIC_A3_CONFIRM_TTL_TURNS: u32 = 2;

/// The shipped confirmation posture (yolo-within-guardrails build). This is
/// LAYER-1 policy only — how much per-action confirmation friction the NL
/// layer applies. LAYER 2 — the `execute_action()` kernel cap-check (Article
/// 29.3) — is untouched by posture: an ungranted action is DENIED in every
/// posture, confirmation skipped or not. Posture is USER policy: it is set
/// only by the deterministic `/posture` command intercepted before model
/// decode; there is no posture AST production, so the model structurally
/// cannot emit a posture change (the OT-26 D1 constrained-grammar argument).
#[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AgenticPosture {
    /// Default (OT-18 D1 front-loaded trust): reversible R/T-tier actions
    /// inside held grants execute with no per-action confirmation (undo
    /// stays armed); irreversible tiers keep their gates — L/X still require
    /// the dry-run + typed confirm, U still refuses.
    Yolo,
    /// The pre-yolo A3 posture, byte-for-byte: every mutating/destructive
    /// action stages and requires confirmation exactly as A2/A3 shipped it.
    Cautious,
    /// The agentic layer is inert: no model decode, no action execution.
    /// (Bit-for-bit pre-agentic remains the compile-time collapse proven at
    /// A1 — inside an agentic build the chat tier is not linked, so runtime
    /// Off proves structural inertness instead.)
    Off,
}

#[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PostureCommand {
    Query,
    Set(AgenticPosture),
    Usage,
}

#[cfg(any(feature = "agentic-actions", test))]
impl PendingAgenticMutation {
    pub fn as_action(&self) -> AgenticAction<'_> {
        AgenticAction::WriteFile {
            path: self.path.as_str(),
            content: self.content.as_str(),
        }
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Debug, Clone, Eq, PartialEq)]
enum PendingAgenticA3Kind {
    TrashFile {
        path: String,
    },
    MoveFile {
        path: String,
        destination: String,
    },
    KillProcess {
        handle: u32,
    },
    SpawnProcess {
        name: String,
        inherit_handle: u32,
    },
    ExternalSend {
        host: String,
        port: u16,
        bytes: usize,
    },
    LoadModel {
        path: String,
    },
}

#[cfg(any(feature = "agentic-actions-a3", test))]
impl PendingAgenticA3Kind {
    fn from_action(action: AgenticAction<'_>) -> Option<Self> {
        Some(match action {
            AgenticAction::TrashFile { path } => Self::TrashFile {
                path: String::from(path),
            },
            AgenticAction::MoveFile { path, destination } => Self::MoveFile {
                path: String::from(path),
                destination: String::from(destination),
            },
            AgenticAction::KillProcess { handle } => Self::KillProcess { handle },
            AgenticAction::SpawnProcess {
                name,
                inherit_handle,
            } => Self::SpawnProcess {
                name: String::from(name),
                inherit_handle,
            },
            AgenticAction::ExternalSend { host, port, bytes } => Self::ExternalSend {
                host: String::from(host),
                port,
                bytes,
            },
            AgenticAction::LoadModel { path } => Self::LoadModel {
                path: String::from(path),
            },
            _ => return None,
        })
    }

    fn as_action(&self) -> AgenticAction<'_> {
        match self {
            Self::TrashFile { path } => AgenticAction::TrashFile {
                path: path.as_str(),
            },
            Self::MoveFile { path, destination } => AgenticAction::MoveFile {
                path: path.as_str(),
                destination: destination.as_str(),
            },
            Self::KillProcess { handle } => AgenticAction::KillProcess { handle: *handle },
            Self::SpawnProcess {
                name,
                inherit_handle,
            } => AgenticAction::SpawnProcess {
                name: name.as_str(),
                inherit_handle: *inherit_handle,
            },
            Self::ExternalSend { host, port, bytes } => AgenticAction::ExternalSend {
                host: host.as_str(),
                port: *port,
                bytes: *bytes,
            },
            Self::LoadModel { path } => AgenticAction::LoadModel {
                path: path.as_str(),
            },
        }
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Debug, Clone, Eq, PartialEq)]
struct PendingAgenticA3Action {
    kind: PendingAgenticA3Kind,
    phrase: String,
    digest: String,
    expires_at_turn: u32,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
impl PendingAgenticA3Action {
    fn as_action(&self) -> AgenticAction<'_> {
        self.kind.as_action()
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
enum AgenticA3Confirm {
    Ready(PendingAgenticA3Action),
    Wrong,
    Stale,
    None,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
const AGENTIC_MULTI_STEP_MAX_STEPS: u8 = 4;

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum AgenticMultiStepUndo {
    None,
    Session,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct AgenticMultiStepEntry {
    step: u8,
    action: &'static str,
    undo: AgenticMultiStepUndo,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct AgenticMultiStepWaiting {
    step_index: u8,
    action: &'static str,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Debug, Clone, Eq, PartialEq)]
struct AgenticMultiStepSession {
    goal: String,
    next_step: u8,
    budget: u8,
    completed: Vec<AgenticMultiStepEntry>,
    waiting: Option<AgenticMultiStepWaiting>,
}

#[cfg(any(feature = "agentic-actions-a3", test))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum AgenticMultiStepOutcome {
    Executed {
        action: &'static str,
        undo: AgenticMultiStepUndo,
    },
    AwaitingConfirm {
        action: &'static str,
        reason: &'static str,
    },
    Failed {
        action: &'static str,
        reason: &'static str,
    },
    Done,
    Clarify,
}

#[derive(Debug)]
pub struct ConversationState {
    context: Vec<u32>,
    max_context: usize,
    #[cfg(any(feature = "agentic-actions", test))]
    pending_agentic_mutation: Option<PendingAgenticMutation>,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    agentic_turn: u32,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    pending_agentic_a3: Option<PendingAgenticA3Action>,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    stale_agentic_phrase: Option<String>,
    #[cfg(any(feature = "agentic-actions-a3", test))]
    agentic_multi_step: Option<AgenticMultiStepSession>,
    #[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
    posture: AgenticPosture,
}

impl ConversationState {
    pub fn new(max_context: usize) -> Self {
        Self {
            context: Vec::new(),
            max_context,
            #[cfg(any(feature = "agentic-actions", test))]
            pending_agentic_mutation: None,
            #[cfg(any(feature = "agentic-actions-a3", test))]
            agentic_turn: 0,
            #[cfg(any(feature = "agentic-actions-a3", test))]
            pending_agentic_a3: None,
            #[cfg(any(feature = "agentic-actions-a3", test))]
            stale_agentic_phrase: None,
            #[cfg(any(feature = "agentic-actions-a3", test))]
            agentic_multi_step: None,
            // The shipped default (PL decision 2026-07-03): yolo-within-scope.
            #[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
            posture: AgenticPosture::Yolo,
        }
    }

    #[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
    pub fn posture(&self) -> AgenticPosture {
        self.posture
    }

    /// Switch posture. A switch invalidates any staged preview or pending
    /// mutation — a dry-run captured under one posture must not be
    /// confirmable under another (the stale-confirm discipline extended to
    /// posture changes).
    #[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
    pub fn set_posture(&mut self, posture: AgenticPosture) {
        #[cfg(any(feature = "agentic-actions-a3", test))]
        self.stale_pending_agentic_a3();
        self.pending_agentic_mutation = None;
        #[cfg(any(feature = "agentic-actions-a3", test))]
        {
            self.agentic_multi_step = None;
        }
        self.posture = posture;
    }

    pub fn context(&self) -> &[u32] {
        &self.context
    }

    pub fn push_tokens(&mut self, tokens: &[u32]) {
        self.context.extend_from_slice(tokens);
        if self.context.len() > self.max_context {
            let overflow = self.context.len() - self.max_context;
            self.context.drain(..overflow);
        }
    }

    #[cfg(any(feature = "agentic-actions", test))]
    fn set_pending_agentic_mutation(
        &mut self,
        path: &str,
        content: &str,
    ) -> Result<(), ConversationError> {
        #[cfg(any(feature = "agentic-actions-a3", test))]
        self.stale_pending_agentic_a3();
        self.pending_agentic_mutation = Some(PendingAgenticMutation {
            path: String::from(path),
            content: String::from(content),
        });
        Ok(())
    }

    #[cfg(any(feature = "agentic-actions", test))]
    fn take_pending_agentic_mutation(&mut self) -> Option<PendingAgenticMutation> {
        self.pending_agentic_mutation.take()
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn start_agentic_multi_step(&mut self, goal: &str) {
        self.stale_pending_agentic_a3();
        self.pending_agentic_mutation = None;
        self.agentic_multi_step = Some(AgenticMultiStepSession {
            goal: String::from(goal),
            next_step: 0,
            budget: AGENTIC_MULTI_STEP_MAX_STEPS,
            completed: Vec::new(),
            waiting: None,
        });
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn take_agentic_multi_step(&mut self) -> Option<AgenticMultiStepSession> {
        self.agentic_multi_step.take()
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn put_agentic_multi_step(&mut self, session: AgenticMultiStepSession) {
        self.agentic_multi_step = Some(session);
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn finish_agentic_multi_step_waiting(
        &mut self,
        undo: AgenticMultiStepUndo,
    ) -> Option<AgenticMultiStepSession> {
        let mut session = self.agentic_multi_step.take()?;
        let waiting = session.waiting.take()?;
        session.completed.push(AgenticMultiStepEntry {
            step: waiting.step_index.saturating_add(1),
            action: waiting.action,
            undo,
        });
        session.next_step = waiting.step_index.saturating_add(1);
        Some(session)
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn begin_agentic_turn(&mut self) {
        self.agentic_turn = self.agentic_turn.saturating_add(1);
        self.expire_pending_agentic_a3();
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn expire_pending_agentic_a3(&mut self) {
        let expired = self
            .pending_agentic_a3
            .as_ref()
            .is_some_and(|pending| self.agentic_turn > pending.expires_at_turn);
        if expired {
            self.stale_pending_agentic_a3();
        }
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn stale_pending_agentic_a3(&mut self) {
        if let Some(pending) = self.pending_agentic_a3.take() {
            self.stale_agentic_phrase = Some(pending.phrase);
        }
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn has_pending_agentic_a3(&self) -> bool {
        self.pending_agentic_a3.is_some()
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn set_pending_agentic_a3(
        &mut self,
        action: AgenticAction<'_>,
    ) -> Result<&PendingAgenticA3Action, ConversationError> {
        let kind = PendingAgenticA3Kind::from_action(action).ok_or(ConversationError::Action)?;
        self.stale_pending_agentic_a3();
        self.pending_agentic_mutation = None;
        let mut phrase = String::new();
        push_agentic_a3_phrase(&kind, &mut phrase);
        let mut digest = String::new();
        push_agentic_a3_digest(&kind, &mut digest);
        self.pending_agentic_a3 = Some(PendingAgenticA3Action {
            kind,
            phrase,
            digest,
            expires_at_turn: self
                .agentic_turn
                .saturating_add(AGENTIC_A3_CONFIRM_TTL_TURNS),
        });
        Ok(self
            .pending_agentic_a3
            .as_ref()
            .expect("pending A3 action was just stored"))
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn confirm_pending_agentic_a3(&mut self, phrase: &str) -> AgenticA3Confirm {
        self.expire_pending_agentic_a3();
        if let Some(pending) = &self.pending_agentic_a3 {
            if pending.phrase == phrase {
                return AgenticA3Confirm::Ready(
                    self.pending_agentic_a3
                        .take()
                        .expect("pending A3 action existed for matching phrase"),
                );
            }
            if self
                .stale_agentic_phrase
                .as_ref()
                .is_some_and(|stale| stale == phrase)
            {
                return AgenticA3Confirm::Stale;
            }
            return AgenticA3Confirm::Wrong;
        }
        if self
            .stale_agentic_phrase
            .as_ref()
            .is_some_and(|stale| stale == phrase)
        {
            AgenticA3Confirm::Stale
        } else {
            AgenticA3Confirm::None
        }
    }

    #[cfg(any(feature = "agentic-actions-a3", test))]
    fn cancel_pending_agentic_a3(&mut self) -> bool {
        let existed = self.pending_agentic_a3.is_some();
        self.stale_pending_agentic_a3();
        self.agentic_multi_step = None;
        existed
    }
}

#[cfg(all(not(feature = "arthur-act"), not(feature = "agentic-actions")))]
pub fn respond_to_turn<B: ConversationBackend>(
    mode: ReplyMode,
    state: &mut ConversationState,
    backend: &mut B,
    input: &[u8],
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    reply.clear();
    let text = core::str::from_utf8(input).map_err(|_| ConversationError::InvalidUtf8)?;
    let text = trim_line(text);
    if text.is_empty() {
        return Err(ConversationError::EmptyPrompt);
    }

    match mode {
        ReplyMode::DummyEcho => {
            push_reply(reply, b"arthur: echo: ")?;
            push_reply(reply, text.as_bytes())?;
            Ok(())
        }
        ReplyMode::Model => generate_model_reply(state, backend, text, reply),
    }
}

#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
pub fn respond_to_turn_with_actions<B: ConversationBackend, A: ToolActionExecutor>(
    mode: ReplyMode,
    state: &mut ConversationState,
    backend: &mut B,
    actions: &mut A,
    input: &[u8],
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    reply.clear();
    let text = core::str::from_utf8(input).map_err(|_| ConversationError::InvalidUtf8)?;
    let text = trim_line(text);
    if text.is_empty() {
        return Err(ConversationError::EmptyPrompt);
    }
    if mode == ReplyMode::DummyEcho {
        push_reply(reply, b"arthur: echo: ")?;
        push_reply(reply, text.as_bytes())?;
        return Ok(());
    }

    let mut prompt = String::new();
    let mut model_reply = Vec::new();
    let mut retry_error: Option<ToolParseError> = None;
    for attempt in 0..=MAX_TOOL_RETRIES {
        if let Some(error) = retry_error {
            render_retry_prompt(actions.available_tools(), text, error, &mut prompt);
        } else {
            render_hermes_prompt(actions.available_tools(), text, &mut prompt);
        }

        model_reply.clear();
        generate_model_reply(state, backend, &prompt, &mut model_reply)?;
        let model_text =
            core::str::from_utf8(&model_reply).map_err(|_| ConversationError::Decode)?;

        match extract_tool_call(model_text) {
            Ok(Some(call)) => {
                reply.clear();
                actions.execute_tool_call(call, reply)?;
                if reply.len() > MAX_REPLY_BYTES {
                    return Err(ConversationError::ReplyTooLarge);
                }
                return Ok(());
            }
            Ok(None) => {
                reply.clear();
                push_reply(reply, &model_reply)?;
                return Ok(());
            }
            Err(error) if attempt < MAX_TOOL_RETRIES => {
                retry_error = Some(error);
            }
            Err(error) => {
                reply.clear();
                push_reply(reply, b"arthur: error: invalid tool call: ")?;
                let mut error_text = String::new();
                push_parse_error(error, &mut error_text);
                push_reply(reply, error_text.as_bytes())?;
                return Ok(());
            }
        }
    }
    Err(ConversationError::Generate)
}

#[cfg(any(feature = "agentic-actions", test))]
pub fn respond_to_turn_with_agentic_actions<B: AgenticActionDecoder, A: AgenticActionExecutor>(
    mode: ReplyMode,
    state: &mut ConversationState,
    backend: &mut B,
    actions: &mut A,
    input: &[u8],
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    reply.clear();
    let text = core::str::from_utf8(input).map_err(|_| ConversationError::InvalidUtf8)?;
    let text = trim_line(text);
    if text.is_empty() {
        return Err(ConversationError::EmptyPrompt);
    }
    // Posture control is a deterministic USER command handled before any
    // model decode — the model's constrained grammar has no posture
    // production, so posture can never be model-selected.
    #[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
    if let Some(command) = parse_posture_command(text) {
        return respond_posture_command(command, state, reply);
    }
    #[cfg(any(feature = "agentic-actions-a3", test))]
    state.begin_agentic_turn();
    if mode == ReplyMode::DummyEcho {
        push_reply(reply, b"arthur: echo: ")?;
        push_reply(reply, text.as_bytes())?;
        return Ok(());
    }
    // Posture Off: the agentic layer is inert — no decode, no execution.
    #[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
    if state.posture() == AgenticPosture::Off {
        return push_reply(
            reply,
            b"arthur: posture: off - agentic actions disabled (/posture yolo to enable)",
        );
    }
    #[cfg(any(feature = "agentic-actions-a3", test))]
    if wants_agentic_multi_step_goal(text) {
        state.start_agentic_multi_step(text);
        return continue_agentic_multi_step(state, backend, actions, reply);
    }
    #[cfg(feature = "arthur-reversibility-stakes")]
    if wants_agentic_direct_undo(text) {
        actions.execute_agentic_action(AgenticAction::Undo, reply)?;
        if reply.len() > MAX_REPLY_BYTES {
            return Err(ConversationError::ReplyTooLarge);
        }
        return Ok(());
    }

    let mut ast = Vec::new();
    backend.decode_agentic_action(state, text, actions.available_agentic_actions(), &mut ast)?;
    let ast_text = core::str::from_utf8(&ast).map_err(|_| ConversationError::Decode)?;
    match parse_agentic_ast(ast_text) {
        Ok(AgenticAction::Clarify { question }) => {
            push_reply(reply, b"arthur: clarify: ")?;
            push_reply(reply, question.as_bytes())
        }
        Ok(AgenticAction::NoOp) => push_reply(reply, b"arthur: no-op"),
        #[cfg(any(feature = "agentic-actions-a3", test))]
        Ok(AgenticAction::DryRun { plan }) => {
            push_reply(reply, b"arthur: dry-run: ")?;
            push_reply(reply, plan.as_bytes())
        }
        // T-tier (trash-recoverable): under the yolo posture these execute
        // immediately within held grants — the executor still routes every
        // one through the unchanged execute_action() cap-check, so a missing
        // cap is DENIED with confirmation skipped (Article 29.3 intact), and
        // the A2 undo snapshot stays armed. Cautious keeps the A3 staging.
        #[cfg(any(feature = "agentic-actions-a3", test))]
        Ok(action @ (AgenticAction::TrashFile { .. } | AgenticAction::MoveFile { .. })) => {
            if state.posture() == AgenticPosture::Yolo {
                actions.execute_agentic_action(action, reply)?;
                push_reply(reply, b" posture=yolo")?;
                return Ok(());
            }
            let pending = state.set_pending_agentic_a3(action)?;
            push_agentic_a3_preview(reply, pending)
        }
        // L-tier (irreversible-local: kill/spawn/load-model) and X-tier
        // (irreversible-external: send) keep the dry-run + typed-confirm gate
        // in EVERY posture — yolo drops friction only for reversible actions;
        // silent destruction/send stays impossible (A3-D4 hard rule). Under
        // yolo the preview carries the posture marker so the QEMU needles
        // prove still-gated-in-yolo distinctly from the cautious segment.
        #[cfg(any(feature = "agentic-actions-a3", test))]
        Ok(
            action @ (AgenticAction::KillProcess { .. }
            | AgenticAction::SpawnProcess { .. }
            | AgenticAction::ExternalSend { .. }
            | AgenticAction::LoadModel { .. }),
        ) => {
            let yolo = state.posture() == AgenticPosture::Yolo;
            let pending = state.set_pending_agentic_a3(action)?;
            push_agentic_a3_preview(reply, pending)?;
            if yolo {
                push_reply(reply, b" posture=yolo")?;
            }
            Ok(())
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        Ok(AgenticAction::ConfirmTyped { phrase }) => {
            match state.confirm_pending_agentic_a3(phrase) {
                AgenticA3Confirm::Ready(pending) => {
                    let pending_action = pending.as_action();
                    let pending_label = pending_action_label(pending_action);
                    actions.execute_confirmed_agentic_action(pending.as_action(), reply)?;
                    if reply.len() > MAX_REPLY_BYTES {
                        return Err(ConversationError::ReplyTooLarge);
                    }
                    if let Some(session) = state.take_agentic_multi_step() {
                        if let Some(reason) = agentic_step_failure_reason(reply) {
                            return finish_agentic_multi_step_failure(
                                reply,
                                session,
                                pending_label,
                                reason,
                            );
                        }
                        state.put_agentic_multi_step(session);
                        let session = state
                            .finish_agentic_multi_step_waiting(agentic_reply_undoability(reply))
                            .expect("multi-step waiting action was stored");
                        state.put_agentic_multi_step(session);
                        return continue_agentic_multi_step(state, backend, actions, reply);
                    }
                    Ok(())
                }
                AgenticA3Confirm::Wrong => {
                    push_reply(reply, b"arthur: typed-confirm: wrong phrase no-op")
                }
                AgenticA3Confirm::Stale => {
                    push_reply(reply, b"arthur: typed-confirm: stale-preview no-op")
                }
                AgenticA3Confirm::None => {
                    push_reply(reply, b"arthur: typed-confirm: no pending action")
                }
            }
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        Ok(AgenticAction::Cancel) => {
            if state.cancel_pending_agentic_a3() {
                push_reply(reply, b"arthur: typed-confirm: cancelled pending action")
            } else {
                push_reply(reply, b"arthur: typed-confirm: no pending action")
            }
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        Ok(AgenticAction::Refuse { reason }) => {
            push_reply(reply, b"arthur: refused: ")?;
            push_reply(reply, reason.as_bytes())
        }
        #[cfg(any(feature = "agentic-actions-a3", test))]
        Ok(AgenticAction::Done { summary }) => {
            push_reply(reply, b"arthur: done: ")?;
            push_reply(reply, summary.as_bytes())
        }
        Ok(
            action @ (AgenticAction::ReadFile { .. }
            | AgenticAction::ListDir { .. }
            | AgenticAction::ChangeDir { .. }
            | AgenticAction::Status),
        ) => {
            actions.execute_agentic_action(action, reply)?;
            if reply.len() > MAX_REPLY_BYTES {
                return Err(ConversationError::ReplyTooLarge);
            }
            Ok(())
        }
        Ok(AgenticAction::WriteFile { path, content }) => {
            // R-tier (snapshot-before-write reversible): yolo executes
            // immediately — the executor's cap-check still disposes, and it
            // still escalates (never silently proceeds) when the undo
            // snapshot cannot be taken. Cautious keeps the A2 confirm.
            #[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
            if state.posture() == AgenticPosture::Yolo {
                actions
                    .execute_agentic_action(AgenticAction::WriteFile { path, content }, reply)?;
                if agentic_write_confirm_required(reply) {
                    state.set_pending_agentic_mutation(path, content)?;
                }
                push_reply(reply, b" posture=yolo")?;
                return Ok(());
            }
            state.set_pending_agentic_mutation(path, content)?;
            push_reply(reply, b"arthur: confirm required: write_file ")?;
            push_reply(reply, path.as_bytes())?;
            push_reply(reply, b" bytes=")?;
            push_usize(reply, content.len())?;
            push_reply(reply, b" reversible=snapshot-before-write")
        }
        Ok(AgenticAction::Confirm) => {
            #[cfg(any(feature = "agentic-actions-a3", test))]
            if state.has_pending_agentic_a3() {
                return push_reply(reply, b"arthur: confirm: typed confirmation required no-op");
            }
            match state.take_pending_agentic_mutation() {
                Some(pending) => {
                    #[cfg(any(feature = "agentic-actions-a3", test))]
                    let pending_action = pending.as_action();
                    #[cfg(any(feature = "agentic-actions-a3", test))]
                    let pending_label = pending_action_label(pending_action);
                    actions.execute_confirmed_agentic_action(pending.as_action(), reply)?;
                    if reply.len() > MAX_REPLY_BYTES {
                        return Err(ConversationError::ReplyTooLarge);
                    }
                    #[cfg(any(feature = "agentic-actions-a3", test))]
                    if let Some(session) = state.take_agentic_multi_step() {
                        if let Some(reason) = agentic_step_failure_reason(reply) {
                            return finish_agentic_multi_step_failure(
                                reply,
                                session,
                                pending_label,
                                reason,
                            );
                        }
                        state.put_agentic_multi_step(session);
                        let session = state
                            .finish_agentic_multi_step_waiting(agentic_reply_undoability(reply))
                            .expect("multi-step waiting action was stored");
                        state.put_agentic_multi_step(session);
                        return continue_agentic_multi_step(state, backend, actions, reply);
                    }
                    Ok(())
                }
                None => push_reply(reply, b"arthur: confirm: no pending action"),
            }
        }
        Ok(AgenticAction::Undo) => {
            actions.execute_agentic_action(AgenticAction::Undo, reply)?;
            if reply.len() > MAX_REPLY_BYTES {
                return Err(ConversationError::ReplyTooLarge);
            }
            Ok(())
        }
        Ok(AgenticAction::Restricted { reason }) => {
            push_reply(reply, b"arthur: restricted: ")?;
            push_reply(reply, reason.as_bytes())
        }
        Err(error) => push_agentic_parse_error(reply, error),
    }
}

#[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
fn agentic_write_confirm_required(reply: &[u8]) -> bool {
    reply.starts_with(b"arthur: confirm required: write_file ")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn wants_agentic_multi_step_goal(text: &str) -> bool {
    contains_ascii_word(text, "multi") || contains_ascii_word(text, "multistep")
}

#[cfg(feature = "arthur-reversibility-stakes")]
fn wants_agentic_direct_undo(text: &str) -> bool {
    contains_ascii_word(text, "undo") || contains_ascii_word(text, "revert")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn continue_agentic_multi_step<B: AgenticActionDecoder, A: AgenticActionExecutor>(
    state: &mut ConversationState,
    backend: &mut B,
    actions: &mut A,
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    let Some(mut session) = state.take_agentic_multi_step() else {
        return push_reply(reply, b"arthur: multi-step stopped reason=no-session");
    };
    let mut prompt = String::new();
    let mut ast = Vec::new();
    loop {
        if session.next_step >= session.budget {
            return finish_agentic_multi_step_budget(reply, session);
        }
        prompt.clear();
        render_agentic_multi_step_prompt(&session, &mut prompt);
        ast.clear();
        backend.decode_agentic_action(
            state,
            prompt.as_str(),
            actions.available_agentic_actions(),
            &mut ast,
        )?;
        let ast_text = core::str::from_utf8(&ast).map_err(|_| ConversationError::Decode)?;
        let action = match parse_agentic_ast(ast_text) {
            Ok(action) => action,
            Err(_) => {
                return finish_agentic_multi_step_failure(
                    reply,
                    session,
                    "decode",
                    "invalid-action",
                );
            }
        };
        match execute_agentic_multi_step_action(state, actions, action, reply)? {
            AgenticMultiStepOutcome::Executed { action, undo } => {
                let step = session.next_step.saturating_add(1);
                session
                    .completed
                    .push(AgenticMultiStepEntry { step, action, undo });
                session.next_step = step;
            }
            AgenticMultiStepOutcome::AwaitingConfirm { action, reason } => {
                session.waiting = Some(AgenticMultiStepWaiting {
                    step_index: session.next_step,
                    action,
                });
                let result = finish_agentic_multi_step_pause(reply, &session, reason);
                state.put_agentic_multi_step(session);
                return result;
            }
            AgenticMultiStepOutcome::Failed { action, reason } => {
                return finish_agentic_multi_step_failure(reply, session, action, reason);
            }
            AgenticMultiStepOutcome::Done => {
                return finish_agentic_multi_step_done(reply, session);
            }
            AgenticMultiStepOutcome::Clarify => {
                state.put_agentic_multi_step(session);
                return push_reply(
                    reply,
                    b"arthur: multi-step clarify: I need a clearer next step before continuing",
                );
            }
        }
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn execute_agentic_multi_step_action<A: AgenticActionExecutor>(
    state: &mut ConversationState,
    actions: &mut A,
    action: AgenticAction<'_>,
    reply: &mut Vec<u8>,
) -> Result<AgenticMultiStepOutcome, ConversationError> {
    reply.clear();
    match action {
        AgenticAction::Done { .. } => Ok(AgenticMultiStepOutcome::Done),
        AgenticAction::Clarify { .. } => Ok(AgenticMultiStepOutcome::Clarify),
        AgenticAction::Restricted { .. } => Ok(AgenticMultiStepOutcome::Failed {
            action: "restricted",
            reason: "restricted",
        }),
        AgenticAction::Refuse { .. } => Ok(AgenticMultiStepOutcome::Failed {
            action: "refuse",
            reason: "refused",
        }),
        AgenticAction::WriteFile { path, content } => {
            actions.execute_agentic_action(AgenticAction::WriteFile { path, content }, reply)?;
            if agentic_write_confirm_required(reply) {
                state.set_pending_agentic_mutation(path, content)?;
                return Ok(AgenticMultiStepOutcome::AwaitingConfirm {
                    action: "write_file",
                    reason: agentic_confirm_reason(reply),
                });
            }
            if let Some(reason) = agentic_step_failure_reason(reply) {
                return Ok(AgenticMultiStepOutcome::Failed {
                    action: "write_file",
                    reason,
                });
            }
            Ok(AgenticMultiStepOutcome::Executed {
                action: "write_file",
                undo: agentic_reply_undoability(reply),
            })
        }
        action @ (AgenticAction::TrashFile { .. } | AgenticAction::MoveFile { .. }) => {
            if state.posture() == AgenticPosture::Yolo {
                let label = pending_action_label(action);
                actions.execute_agentic_action(action, reply)?;
                if let Some(reason) = agentic_step_failure_reason(reply) {
                    return Ok(AgenticMultiStepOutcome::Failed {
                        action: label,
                        reason,
                    });
                }
                return Ok(AgenticMultiStepOutcome::Executed {
                    action: label,
                    undo: agentic_reply_undoability(reply),
                });
            }
            let label = pending_action_label(action);
            state.set_pending_agentic_a3(action)?;
            Ok(AgenticMultiStepOutcome::AwaitingConfirm {
                action: label,
                reason: "typed-confirm",
            })
        }
        action @ (AgenticAction::KillProcess { .. }
        | AgenticAction::SpawnProcess { .. }
        | AgenticAction::ExternalSend { .. }
        | AgenticAction::LoadModel { .. }) => {
            let label = pending_action_label(action);
            state.set_pending_agentic_a3(action)?;
            Ok(AgenticMultiStepOutcome::AwaitingConfirm {
                action: label,
                reason: "typed-confirm",
            })
        }
        action @ (AgenticAction::ReadFile { .. }
        | AgenticAction::ListDir { .. }
        | AgenticAction::ChangeDir { .. }
        | AgenticAction::Status
        | AgenticAction::Undo) => {
            let label = pending_action_label(action);
            actions.execute_agentic_action(action, reply)?;
            if let Some(reason) = agentic_step_failure_reason(reply) {
                return Ok(AgenticMultiStepOutcome::Failed {
                    action: label,
                    reason,
                });
            }
            Ok(AgenticMultiStepOutcome::Executed {
                action: label,
                undo: agentic_reply_undoability(reply),
            })
        }
        AgenticAction::Confirm
        | AgenticAction::DryRun { .. }
        | AgenticAction::ConfirmTyped { .. }
        | AgenticAction::Cancel
        | AgenticAction::NoOp => Ok(AgenticMultiStepOutcome::Failed {
            action: pending_action_label(action),
            reason: "control-action",
        }),
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn render_agentic_multi_step_prompt(session: &AgenticMultiStepSession, out: &mut String) {
    out.push_str("multi-step goal=");
    out.push_str(&session.goal);
    out.push_str(" step=");
    push_decimal_u8_string(out, session.next_step.saturating_add(1));
    out.push(' ');
    if contains_ascii_word(&session.goal, "runaway") {
        out.push_str("observe read /scratch h");
        return;
    }
    match session.next_step {
        0 => out.push_str("observe read /scratch h"),
        1 => out.push_str("mutate write /scratch !"),
        2 => out.push_str("verify read /scratch h"),
        _ => out.push_str("done d"),
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn finish_agentic_multi_step_pause(
    reply: &mut Vec<u8>,
    session: &AgenticMultiStepSession,
    reason: &'static str,
) -> Result<(), ConversationError> {
    reply.clear();
    let Some(waiting) = session.waiting else {
        return push_reply(
            reply,
            b"arthur: multi-step stopped reason=no-pending-confirm",
        );
    };
    push_reply(reply, b"arthur: multi-step paused step=")?;
    push_usize(reply, waiting.step_index.saturating_add(1) as usize)?;
    push_reply(reply, b" reason=")?;
    push_reply(reply, reason.as_bytes())?;
    push_reply(reply, b" ledger=")?;
    push_agentic_multi_step_ledger(reply, session, true)?;
    push_reply(reply, b" chain_undo=session per_step_confirm=ok")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn finish_agentic_multi_step_done(
    reply: &mut Vec<u8>,
    session: AgenticMultiStepSession,
) -> Result<(), ConversationError> {
    reply.clear();
    push_reply(reply, b"arthur: multi-step done steps=")?;
    push_usize(reply, session.completed.len())?;
    push_reply(reply, b" budget=")?;
    push_usize(reply, session.budget as usize)?;
    push_reply(reply, b" ledger=")?;
    push_agentic_multi_step_ledger(reply, &session, false)?;
    push_reply(
        reply,
        b" chain_undo=session cap_check=per_step multi_step=ok",
    )
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn finish_agentic_multi_step_budget(
    reply: &mut Vec<u8>,
    session: AgenticMultiStepSession,
) -> Result<(), ConversationError> {
    reply.clear();
    push_reply(reply, b"arthur: multi-step stopped reason=budget steps=")?;
    push_usize(reply, session.completed.len())?;
    push_reply(reply, b" budget=")?;
    push_usize(reply, session.budget as usize)?;
    push_reply(reply, b" ledger=")?;
    push_agentic_multi_step_ledger(reply, &session, false)
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn finish_agentic_multi_step_failure(
    reply: &mut Vec<u8>,
    session: AgenticMultiStepSession,
    action: &'static str,
    reason: &'static str,
) -> Result<(), ConversationError> {
    let failed_step = session.next_step.saturating_add(1);
    reply.clear();
    push_reply(reply, b"arthur: multi-step stopped step=")?;
    push_usize(reply, failed_step as usize)?;
    push_reply(reply, b" action=")?;
    push_reply(reply, action.as_bytes())?;
    push_reply(reply, b" reason=")?;
    push_reply(reply, reason.as_bytes())?;
    push_reply(reply, b" ledger=")?;
    push_agentic_multi_step_ledger(reply, &session, false)?;
    push_reply(reply, b" chain_undo=session")
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_agentic_multi_step_ledger(
    reply: &mut Vec<u8>,
    session: &AgenticMultiStepSession,
    include_pending: bool,
) -> Result<(), ConversationError> {
    if session.completed.is_empty() && !(include_pending && session.waiting.is_some()) {
        return push_reply(reply, b"none");
    }
    let mut first = true;
    for entry in &session.completed {
        if !first {
            push_reply(reply, b",")?;
        }
        first = false;
        push_usize(reply, entry.step as usize)?;
        push_reply(reply, b":")?;
        push_reply(reply, entry.action.as_bytes())?;
        push_reply(reply, b" undo=")?;
        push_reply(reply, agentic_multi_step_undo_label(entry.undo))?;
    }
    if include_pending && let Some(waiting) = session.waiting {
        if !first {
            push_reply(reply, b",")?;
        }
        push_usize(reply, waiting.step_index.saturating_add(1) as usize)?;
        push_reply(reply, b":")?;
        push_reply(reply, waiting.action.as_bytes())?;
        push_reply(reply, b" pending-confirm")?;
    }
    Ok(())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn agentic_multi_step_undo_label(undo: AgenticMultiStepUndo) -> &'static [u8] {
    match undo {
        AgenticMultiStepUndo::None => b"no",
        AgenticMultiStepUndo::Session => b"session",
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn pending_action_label(action: AgenticAction<'_>) -> &'static str {
    match action {
        AgenticAction::ReadFile { .. } => "read_file",
        AgenticAction::WriteFile { .. } => "write_file",
        AgenticAction::ListDir { .. } => "list_dir",
        AgenticAction::ChangeDir { .. } => "cd",
        AgenticAction::Status => "status",
        AgenticAction::TrashFile { .. } => "trash_file",
        AgenticAction::MoveFile { .. } => "move_file",
        AgenticAction::KillProcess { .. } => "kill_process",
        AgenticAction::SpawnProcess { .. } => "spawn_process",
        AgenticAction::ExternalSend { .. } => "external_send",
        AgenticAction::LoadModel { .. } => "load_model",
        AgenticAction::Undo => "undo",
        AgenticAction::Confirm => "confirm",
        AgenticAction::ConfirmTyped { .. } => "confirm_typed",
        AgenticAction::DryRun { .. } => "dry_run",
        AgenticAction::Cancel => "cancel",
        AgenticAction::Clarify { .. } => "clarify",
        AgenticAction::Restricted { .. } => "restricted",
        AgenticAction::Refuse { .. } => "refuse",
        AgenticAction::Done { .. } => "done",
        AgenticAction::NoOp => "no_op",
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn agentic_reply_undoability(reply: &[u8]) -> AgenticMultiStepUndo {
    if bytes_contains(reply, b"undo=") || bytes_contains(reply, b"restored") {
        AgenticMultiStepUndo::Session
    } else {
        AgenticMultiStepUndo::None
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn agentic_step_failure_reason(reply: &[u8]) -> Option<&'static str> {
    if reply.starts_with(b"arthur: Denied:") {
        Some("denied")
    } else if reply.starts_with(b"arthur: restricted:") {
        Some("restricted")
    } else if reply.starts_with(b"arthur: syscall failed:") {
        Some("syscall")
    } else {
        None
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn agentic_confirm_reason(reply: &[u8]) -> &'static str {
    if bytes_contains(reply, b"stakes-policy") {
        "stakes-policy"
    } else if bytes_contains(reply, b"key-file-not-backed-up") {
        "key-file-not-backed-up"
    } else {
        "confirm-required"
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn bytes_contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[cfg(any(
    feature = "agentic-actions-a3",
    feature = "arthur-reversibility-stakes",
    test
))]
fn contains_ascii_word(text: &str, needle: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|word| word.eq_ignore_ascii_case(needle))
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_decimal_u8_string(out: &mut String, value: u8) {
    if value >= 10 {
        out.push(char::from(b'0' + (value / 10)));
    }
    out.push(char::from(b'0' + (value % 10)));
}

/// Parse the deterministic `/posture` control command. Returns `None` for
/// anything that is not a `/posture` line (falls through to model decode);
/// a `/posture` line with an unknown argument yields `Usage` so it can never
/// leak into the model as a prompt.
#[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
fn parse_posture_command(text: &str) -> Option<PostureCommand> {
    let rest = text.trim().strip_prefix("/posture")?;
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    Some(match rest.trim() {
        "" => PostureCommand::Query,
        "yolo" => PostureCommand::Set(AgenticPosture::Yolo),
        "cautious" => PostureCommand::Set(AgenticPosture::Cautious),
        "off" => PostureCommand::Set(AgenticPosture::Off),
        _ => PostureCommand::Usage,
    })
}

#[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
fn respond_posture_command(
    command: PostureCommand,
    state: &mut ConversationState,
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    match command {
        PostureCommand::Query => {
            push_reply(reply, b"arthur: posture: ")?;
            push_reply(reply, posture_label(state.posture()))
        }
        PostureCommand::Set(posture) => {
            state.set_posture(posture);
            push_reply(reply, b"arthur: posture: set ")?;
            push_reply(reply, posture_label(posture))
        }
        PostureCommand::Usage => push_reply(
            reply,
            b"arthur: posture: usage /posture [yolo|cautious|off]",
        ),
    }
}

#[cfg(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3", test))]
fn posture_label(posture: AgenticPosture) -> &'static [u8] {
    match posture {
        AgenticPosture::Yolo => b"yolo",
        AgenticPosture::Cautious => b"cautious",
        AgenticPosture::Off => b"off",
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_agentic_a3_preview(
    reply: &mut Vec<u8>,
    pending: &PendingAgenticA3Action,
) -> Result<(), ConversationError> {
    push_reply(reply, b"arthur: dry-run required: tier=")?;
    push_reply(
        reply,
        agentic_effect_label(agentic_effect_tier(pending.as_action())),
    )?;
    push_reply(reply, b" digest=")?;
    push_reply(reply, pending.digest.as_bytes())?;
    push_reply(reply, b" action=")?;
    push_agentic_a3_kind(reply, &pending.kind)?;
    push_reply(reply, b" phrase=")?;
    push_reply(reply, pending.phrase.as_bytes())
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn agentic_effect_label(tier: AgenticEffectTier) -> &'static [u8] {
    match tier {
        AgenticEffectTier::ReadOnly => b"R",
        AgenticEffectTier::TrashRecoverable => b"T",
        AgenticEffectTier::LocalDestructive => b"L",
        AgenticEffectTier::ExternalSend => b"X",
        AgenticEffectTier::Unpreviewable => b"U",
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_agentic_a3_kind(
    reply: &mut Vec<u8>,
    kind: &PendingAgenticA3Kind,
) -> Result<(), ConversationError> {
    match kind {
        PendingAgenticA3Kind::TrashFile { path } => {
            push_reply(reply, b"trash_file ")?;
            push_reply(reply, path.as_bytes())?;
            push_reply(reply, b" recovery=A2")
        }
        PendingAgenticA3Kind::MoveFile { path, destination } => {
            push_reply(reply, b"move_file ")?;
            push_reply(reply, path.as_bytes())?;
            push_reply(reply, b" -> ")?;
            push_reply(reply, destination.as_bytes())?;
            push_reply(reply, b" recovery=A2")
        }
        PendingAgenticA3Kind::KillProcess { handle } => {
            push_reply(reply, b"kill_process ")?;
            push_usize(reply, *handle as usize)
        }
        PendingAgenticA3Kind::SpawnProcess {
            name,
            inherit_handle,
        } => {
            push_reply(reply, b"spawn_process ")?;
            push_reply(reply, name.as_bytes())?;
            if *inherit_handle != 0 {
                push_reply(reply, b" inherit=")?;
                push_usize(reply, *inherit_handle as usize)?;
            }
            Ok(())
        }
        PendingAgenticA3Kind::ExternalSend { host, port, bytes } => {
            push_reply(reply, b"external_send ")?;
            push_reply(reply, host.as_bytes())?;
            push_reply(reply, b":")?;
            push_usize(reply, *port as usize)?;
            push_reply(reply, b" bytes=")?;
            push_usize(reply, *bytes)?;
            push_reply(reply, b" policy=per-send")
        }
        PendingAgenticA3Kind::LoadModel { path } => {
            push_reply(reply, b"load_model ")?;
            push_reply(reply, path.as_bytes())
        }
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_agentic_a3_phrase(kind: &PendingAgenticA3Kind, out: &mut String) {
    match kind {
        PendingAgenticA3Kind::TrashFile { path } => {
            out.push_str("confirm trash ");
            out.push_str(path);
        }
        PendingAgenticA3Kind::MoveFile { path, destination } => {
            out.push_str("confirm move ");
            out.push_str(path);
            out.push(' ');
            out.push_str(destination);
        }
        PendingAgenticA3Kind::KillProcess { handle } => {
            out.push_str("confirm kill ");
            push_string_u32(out, *handle);
        }
        PendingAgenticA3Kind::SpawnProcess {
            name,
            inherit_handle,
        } => {
            out.push_str("confirm spawn ");
            out.push_str(name);
            out.push(' ');
            push_string_u32(out, *inherit_handle);
        }
        PendingAgenticA3Kind::ExternalSend { host, port, .. } => {
            out.push_str("confirm send ");
            out.push_str(host);
            out.push(':');
            push_string_u32(out, u32::from(*port));
        }
        PendingAgenticA3Kind::LoadModel { path } => {
            out.push_str("confirm model ");
            out.push_str(path);
        }
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_agentic_a3_digest(kind: &PendingAgenticA3Kind, out: &mut String) {
    let mut hash = 0x811c9dc5u32;
    hash_agentic_a3_kind(kind, &mut hash);
    out.push_str("a3-");
    push_hex_u32_string(out, hash);
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn hash_agentic_a3_kind(kind: &PendingAgenticA3Kind, hash: &mut u32) {
    match kind {
        PendingAgenticA3Kind::TrashFile { path } => {
            hash_bytes(hash, b"trash");
            hash_bytes(hash, path.as_bytes());
        }
        PendingAgenticA3Kind::MoveFile { path, destination } => {
            hash_bytes(hash, b"move");
            hash_bytes(hash, path.as_bytes());
            hash_bytes(hash, destination.as_bytes());
        }
        PendingAgenticA3Kind::KillProcess { handle } => {
            hash_bytes(hash, b"kill");
            hash_u32(hash, *handle);
        }
        PendingAgenticA3Kind::SpawnProcess {
            name,
            inherit_handle,
        } => {
            hash_bytes(hash, b"spawn");
            hash_bytes(hash, name.as_bytes());
            hash_u32(hash, *inherit_handle);
        }
        PendingAgenticA3Kind::ExternalSend { host, port, bytes } => {
            hash_bytes(hash, b"send");
            hash_bytes(hash, host.as_bytes());
            hash_u32(hash, u32::from(*port));
            hash_u32(hash, *bytes as u32);
        }
        PendingAgenticA3Kind::LoadModel { path } => {
            hash_bytes(hash, b"model");
            hash_bytes(hash, path.as_bytes());
        }
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn hash_bytes(hash: &mut u32, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u32::from(*byte);
        *hash = hash.wrapping_mul(0x01000193);
    }
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn hash_u32(hash: &mut u32, value: u32) {
    hash_bytes(hash, &value.to_le_bytes());
}

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_string_u32(out: &mut String, value: u32) {
    let mut buf = [0u8; 10];
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

#[cfg(any(feature = "agentic-actions-a3", test))]
fn push_hex_u32_string(out: &mut String, value: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for shift in (0..8).rev() {
        let nibble = ((value >> (shift * 4)) & 0xf) as usize;
        out.push(char::from(HEX[nibble]));
    }
}

#[cfg(not(feature = "agentic-actions"))]
fn generate_model_reply<B: ConversationBackend>(
    state: &mut ConversationState,
    backend: &mut B,
    prompt: &str,
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    let mut prompt_tokens = Vec::new();
    backend.tokenize(prompt, &mut prompt_tokens)?;
    #[cfg(feature = "arthur-real-brain-boot")]
    stage_b_trace(b"conversation tokenize returned");
    if prompt_tokens.is_empty() {
        return Err(ConversationError::EmptyPrompt);
    }
    state.push_tokens(&prompt_tokens);
    #[cfg(feature = "arthur-real-brain-boot")]
    stage_b_trace(b"conversation context pushed");

    let mut generated = Vec::new();
    #[cfg(feature = "arthur-real-brain-boot")]
    stage_b_trace(b"conversation generate call");
    backend.generate(state.context(), &mut generated)?;
    if generated.is_empty() {
        return Err(ConversationError::Generate);
    }
    state.push_tokens(&generated);
    backend.decode(&generated, reply)?;
    if reply.len() > MAX_REPLY_BYTES {
        return Err(ConversationError::ReplyTooLarge);
    }
    Ok(())
}

#[cfg(any(feature = "agentic-actions", test))]
fn push_agentic_parse_error(
    reply: &mut Vec<u8>,
    error: AgenticAstError,
) -> Result<(), ConversationError> {
    push_reply(reply, b"arthur: error: invalid agentic action: ")?;
    let text = match error {
        AgenticAstError::UnknownProduction => b"unknown production".as_slice(),
        AgenticAstError::InvalidPath => b"invalid path".as_slice(),
        AgenticAstError::InvalidContent => b"invalid content".as_slice(),
        AgenticAstError::InvalidQuestion => b"invalid question".as_slice(),
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAstError::InvalidName => b"invalid name".as_slice(),
        #[cfg(any(feature = "agentic-actions-a3", test))]
        AgenticAstError::InvalidHost => b"invalid host".as_slice(),
        AgenticAstError::TooLarge => b"too large".as_slice(),
    };
    push_reply(reply, text)
}

#[cfg(any(feature = "agentic-actions", test))]
fn push_usize(reply: &mut Vec<u8>, mut value: usize) -> Result<(), ConversationError> {
    let mut digits = [0u8; 20];
    let mut idx = digits.len();
    if value == 0 {
        return push_reply(reply, b"0");
    }
    while value > 0 {
        idx -= 1;
        digits[idx] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    push_reply(reply, &digits[idx..])
}

fn push_reply(reply: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ConversationError> {
    if reply.len().saturating_add(bytes.len()) > MAX_REPLY_BYTES {
        return Err(ConversationError::ReplyTooLarge);
    }
    reply.extend_from_slice(bytes);
    Ok(())
}

fn trim_line(text: &str) -> &str {
    text.trim_matches(|ch| ch == '\r' || ch == '\n')
}

#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_trace(message: &[u8]) {
    let _ = libneo::sys_write(1, message.as_ptr(), message.len());
    let _ = libneo::sys_write(1, b"\n".as_ptr(), 1);
}
