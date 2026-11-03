// SPDX-License-Identifier: MPL-2.0
//! `neo-arthur` — Arthur Slice 3 conversation relay.

#![no_std]
#![no_main]

#[cfg(all(feature = "arthur-compositor", feature = "console-interactive"))]
compile_error!("Arthur's G1 client surface supersedes the direct framebuffer console");
#[cfg(all(feature = "arthur-busy-emblem", feature = "arthur-legacy-5x7-font"))]
compile_error!("the busy emblem's rect comes from neo-text's flow; the legacy grid has none");
#[cfg(all(feature = "arthur-scrollback", feature = "arthur-legacy-5x7-font"))]
compile_error!("the scrollback is neo-text's flow's; the legacy grid has none");

extern crate alloc;

#[cfg(feature = "agentic-actions")]
mod agentic_action_logic;
mod conversation_logic;
#[cfg(feature = "desktop-actions")]
mod desktop_action_logic;
#[cfg(feature = "desktop-actions")]
mod desktop_actions;
#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
mod intent_logic;
#[cfg(any(feature = "arthur-memory-encryption", feature = "arthur-reversibility"))]
#[cfg_attr(feature = "arthur-reversibility", allow(dead_code))]
mod memory_crypto;
#[cfg(feature = "arthur-memory-encryption")]
mod memory_encryption;
#[cfg(feature = "arthur-memory")]
mod memory_logic;
#[cfg(feature = "arthur-boot-module")]
#[cfg_attr(not(feature = "arthur-model-from-neofs"), allow(dead_code))]
mod model_state_logic;
#[cfg(feature = "arthur-reversibility")]
mod reversibility_logic;
#[cfg(feature = "arthur-compositor")]
mod showcase_logic;
#[cfg(feature = "arthur-compositor")]
mod showcase_surface;
#[cfg(feature = "arthur-compositor")]
mod showcase_text;
#[path = "../../../kernel/src/ai/tokenizer_logic.rs"]
mod tokenizer_logic;
#[cfg(feature = "arthur-real-vocab")]
#[path = "../../../kernel/src/ai/vocab_bin_logic.rs"]
mod vocab_bin_logic;
#[cfg(feature = "arthur-real-vocab")]
mod vocab_sidecar;

#[cfg(feature = "agentic-actions")]
use agentic_action_logic::{
    AgenticAction, AgenticActionKind, fallback_agentic_action, render_agentic_ast,
};
#[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
use agentic_action_logic::{agentic_fixture_oracle_token_for_ast, push_agentic_candidate_asts};
#[cfg(feature = "agentic-actions")]
use alloc::string::String;
use alloc::vec::Vec;
#[cfg(not(feature = "agentic-actions"))]
use conversation_logic::ConversationBackend;
#[cfg(not(feature = "arthur-act"))]
use conversation_logic::respond_to_turn;
#[cfg(feature = "agentic-actions")]
use conversation_logic::{
    AgenticActionDecoder, AgenticActionExecutor, respond_to_turn_with_agentic_actions,
};
use conversation_logic::{
    ConversationError, ConversationState, DEFAULT_CONTEXT_WINDOW_TOKENS, ReplyMode,
};
#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
use conversation_logic::{ToolActionExecutor, respond_to_turn_with_actions};
#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
use intent_logic::{ToolCall, ToolKind};

#[cfg(feature = "arthur-act")]
use libneo::FileRights;
#[cfg(any(
    not(feature = "console-interactive"),
    feature = "arthur-act",
    feature = "arthur-grants",
    feature = "arthur-memory-delegation"
))]
use libneo::Handle;
#[cfg(any(
    all(feature = "arthur-act", not(feature = "agentic-actions")),
    feature = "agentic-actions-a3"
))]
use libneo::ProcessHandle;
#[cfg(any(
    all(feature = "arthur-act", not(feature = "agentic-actions")),
    feature = "arthur-grants",
    feature = "agentic-actions-a3"
))]
use libneo::TcpSocketHandle;
#[cfg(any(
    not(feature = "agentic-actions"),
    feature = "arthur-grants",
    feature = "agentic-actions-a3"
))]
use libneo::TensorHandle;
#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
use libneo::action::execute_action;
#[cfg(feature = "arthur-act")]
use libneo::action::{
    Action, ActionError, ActionKind, ActionOutcome, Grant, RequiredCapability, execute_action_with,
};
#[cfg(any(feature = "arthur-grants", feature = "agentic-actions-a3"))]
use libneo::action::{ActionSyscalls, TcpRights};
#[cfg(any(feature = "arthur-grants", feature = "agentic-actions-a3"))]
use libneo::process_rights;
use libneo::sys_exit;
#[cfg(feature = "arthur-compositor")]
use libneo::sys_sysstat;
#[cfg(any(
    feature = "arthur-grants",
    feature = "agentic-actions-a3",
    feature = "arthur-memory-delegation"
))]
use libneo::{CapRights, meta_rights};
#[cfg(feature = "arthur-grants")]
use libneo::{EINVAL, sys_recv_grant};
#[cfg(feature = "arthur-memory")]
use libneo::{
    ENOENT, FileHandle, FileRights, OPEN_CREATE_REGULAR, sys_close, sys_fsync, sys_open, sys_read,
    sys_readdir, sys_write,
};
#[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
use libneo::{GenerateConstrainedSequenceDesc, sys_generate_masked_step};
#[cfg(any(
    not(feature = "agentic-actions"),
    feature = "arthur-real-vocab",
    feature = "arthur-grants",
    feature = "agentic-actions-a3"
))]
use libneo::{ModelHandle, OPEN_ROOT_TEMPORARY};
#[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
use libneo::{ModelQueryDesc, sys_model_drop, sys_model_load, sys_model_query};
#[cfg(all(feature = "arthur-reversibility", not(feature = "arthur-memory")))]
use libneo::{OPEN_CREATE_REGULAR, sys_close, sys_fsync, sys_open, sys_read, sys_write};
#[cfg(not(feature = "agentic-actions"))]
use libneo::{
    TENSOR_DEVICE_CPU, TENSOR_DTYPE_F32, TENSOR_DTYPE_I32, TensorCreateDesc, TensorQueryDesc,
    sys_infer_run, sys_tensor_create, sys_tensor_drop, sys_tensor_query,
};
#[cfg(feature = "arthur-memory-delegation")]
use libneo::{sys_derive, sys_ftruncate, sys_grant, sys_revoke};
#[cfg(not(feature = "console-interactive"))]
use libneo::{sys_recv, sys_send};
#[cfg(feature = "arthur-reversibility")]
use memory_crypto::{CategoryKey, decrypt_file, encrypt_plaintext};
#[cfg(feature = "arthur-memory-encryption")]
use memory_encryption::MemoryEncryption;
#[cfg(feature = "arthur-memory-delegation")]
use memory_logic::MemoryDelegationAccess;
#[cfg(feature = "arthur-memory")]
use memory_logic::{
    MAX_MEMORY_FILE_BYTES, MemoryCommand, MemoryError, MemoryRecord, MemoryType, OwnedMemoryRecord,
    category_path, encode_record, parse_memory_command, parse_records_lossy,
};
#[cfg(not(feature = "agentic-actions"))]
use tokenizer_logic::{BpeMergeEntry, BpeTokenizer, BpeVocabEntry};
#[cfg(feature = "arthur-real-vocab")]
use vocab_sidecar::{QwenTemplate, RealVocab};

#[cfg(any(not(feature = "console-interactive"), feature = "arthur-grants"))]
const CONVERSATION: Handle = Handle(0);
#[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
const INHERITED_MODEL: ModelHandle = ModelHandle(2);
#[cfg(feature = "arthur-memory")]
const MEMORY_DIR: FileHandle = FileHandle(3);
#[cfg(feature = "arthur-memory-forgetting")]
const FORGET_OVERWRITE_BYTE: u8 = 0xa5;
#[cfg(feature = "arthur-grants")]
const GRANT_HANDLE_START: u32 = 3;
#[cfg(feature = "arthur-grants")]
const ARTHUR_GRANTS_STORE_PATH: &str = "/neofs/.arthur/grants.json";
#[cfg(feature = "arthur-grants")]
const GRANTS_BUF_LEN: usize = 2048;
#[cfg(feature = "arthur-grants")]
const MAX_BACKED_GRANTS: usize = 8;
#[cfg(feature = "arthur-real-vocab")]
#[cfg(not(feature = "arthur-real-brain-boot-qwen35"))]
#[cfg(any(
    not(feature = "arthur-real-brain-boot"),
    feature = "arthur-real-brain-boot-15b"
))]
const VOCAB_SIDECAR_PATH: &str = "/neofs/model-qwen2.5-1.5b.vocab";
#[cfg(feature = "arthur-real-vocab")]
#[cfg(all(
    feature = "arthur-real-brain-boot",
    not(feature = "arthur-boot-module"),
    not(feature = "arthur-real-brain-boot-15b"),
    not(feature = "arthur-real-brain-boot-3b"),
    not(feature = "arthur-real-brain-boot-qwen35")
))]
const VOCAB_SIDECAR_PATH: &str = "/neofs/model-qwen2.5-0.5b.vocab";
// B1b: the vocab is an ordinary boot module, read through the kernel's
// read-only `/boot` (no NeoFS disk needed).
#[cfg(all(
    feature = "arthur-boot-module",
    not(feature = "arthur-vocab-binary"),
    not(feature = "arthur-real-brain-boot-15b"),
    not(feature = "arthur-real-brain-boot-3b"),
    not(feature = "arthur-real-brain-boot-qwen35")
))]
const VOCAB_SIDECAR_PATH: &str = "/boot/vocab/model-qwen2.5-0.5b.vocab";
// The same vocab prebuilt as NEOVOCB1 at image build: validated and used in
// place, no parse. A missing or bad file is the vocab's state; there is no
// fallback to the text sidecar.
#[cfg(all(
    feature = "arthur-vocab-binary",
    not(feature = "arthur-real-brain-boot-15b"),
    not(feature = "arthur-real-brain-boot-3b"),
    not(feature = "arthur-real-brain-boot-qwen35")
))]
const VOCAB_SIDECAR_PATH: &str = "/boot/vocab/model-qwen2.5-0.5b.vocb";
#[cfg(feature = "arthur-real-vocab")]
#[cfg(all(
    feature = "arthur-real-brain-boot-3b",
    not(feature = "arthur-real-brain-boot-qwen35")
))]
const VOCAB_SIDECAR_PATH: &str = "/neofs/model-qwen2.5-3b.vocab";
#[cfg(feature = "arthur-real-brain-boot-qwen35")]
const VOCAB_SIDECAR_PATH: &str = "/neofs/model-qwen3.5-0.8b.vocab";
#[cfg(all(feature = "arthur-real-vocab", not(feature = "arthur-real-brain-boot")))]
const VOCAB_READ_BUF_LEN: usize = 1 << 20; // 1 MiB read buffer for fixture sidecars
#[cfg(all(feature = "arthur-real-vocab", feature = "arthur-real-brain-boot"))]
const VOCAB_READ_BUF_LEN: usize = 16 << 20; // Real Qwen2.5 sidecar is about 8 MiB.
#[cfg(feature = "arthur-real-vocab")]
const VOCAB_READ_CHUNK_LEN: usize = 4096;
#[cfg(all(
    not(feature = "arthur-real-brain-boot"),
    any(not(feature = "agentic-actions"), feature = "arthur-real-vocab")
))]
const DEFAULT_MODEL_INDEX: usize = 0;
#[cfg(all(
    feature = "arthur-real-brain-boot",
    not(feature = "arthur-real-brain-boot-15b"),
    not(feature = "arthur-real-brain-boot-3b"),
    not(feature = "arthur-real-brain-boot-qwen35"),
    any(not(feature = "agentic-actions"), feature = "arthur-real-vocab")
))]
const DEFAULT_MODEL_INDEX: usize = 1;
#[cfg(all(
    feature = "arthur-real-brain-boot-15b",
    not(feature = "arthur-real-brain-boot-qwen35"),
    any(not(feature = "agentic-actions"), feature = "arthur-real-vocab")
))]
const DEFAULT_MODEL_INDEX: usize = 0;
#[cfg(all(
    feature = "arthur-real-brain-boot-3b",
    not(feature = "arthur-real-brain-boot-qwen35"),
    any(not(feature = "agentic-actions"), feature = "arthur-real-vocab")
))]
const DEFAULT_MODEL_INDEX: usize = 2;
#[cfg(all(
    feature = "arthur-real-brain-boot-qwen35",
    any(not(feature = "agentic-actions"), feature = "arthur-real-vocab")
))]
const DEFAULT_MODEL_INDEX: usize = 2 + cfg!(feature = "arthur-real-brain-boot-3b") as usize;
#[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
const QWEN_15B_ALIASES: &[&str] = &["default", "1.5b", "qwen2.5-1.5b", "qwen2.5-1.5b-instruct"];
#[cfg(all(
    feature = "arthur-real-brain-boot-3b",
    feature = "arthur-real-infer",
    not(feature = "agentic-actions")
))]
const QWEN_3B_ALIASES: &[&str] = &["3b", "qwen2.5-3b", "qwen2.5-3b-instruct"];
#[cfg(all(
    feature = "arthur-real-brain-boot-qwen35",
    feature = "arthur-real-infer",
    not(feature = "agentic-actions")
))]
const QWEN35_ALIASES: &[&str] = &["qwen3.5", "0.8b", "qwen3.5-0.8b"];
#[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
const QWEN_05B_ALIASES: &[&str] = &[
    "fallback",
    "0.5b",
    "qwen2.5-0.5b",
    "qwen2.5-0.5b-instruct",
    "qwen2.5-0.5b-q4_k_m",
];
#[cfg(all(
    any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"),
    not(feature = "arthur-boot-module")
))]
const MODEL_05B_PATH: &str = "/neofs/model-qwen2.5-0.5b.neom";
/// B1b: the 0.5B comes from its boot module. The kernel checks the pinned
/// digest before building it and reclaims the module afterwards.
#[cfg(all(feature = "arthur-boot-module", not(feature = "arthur-boot-model-kat")))]
const MODEL_05B_PATH: &str = "/boot/models/model-qwen2.5-0.5b.neom";
/// OT-87 R2: the second source, the installed NeoFS volume. The kernel holds
/// both files to the same pins as the boot modules.
#[cfg(feature = "arthur-model-from-neofs")]
const NEOFS_MODEL_05B_PATH: &str = "/neofs/model-qwen2.5-0.5b.neom";
#[cfg(feature = "arthur-model-from-neofs")]
const NEOFS_VOCAB_PATH: &str = "/neofs/model-qwen2.5-0.5b.vocb";
/// A9-D test only: the KAT inventory LM, so a QEMU answer is a real
/// multi-token generation through the product core.
#[cfg(feature = "arthur-boot-model-kat")]
const MODEL_05B_PATH: &str = "/boot/models/kv-persist-kat.neom";
#[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
const MODEL_REGISTRY: &[ModelEntry] = &[
    ModelEntry {
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        name: "1.5b",
        path: "/neofs/model-qwen2.5-1.5b.neom",
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        aliases: QWEN_15B_ALIASES,
    },
    ModelEntry {
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        name: "0.5b",
        path: MODEL_05B_PATH,
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        aliases: QWEN_05B_ALIASES,
    },
    // OT-38: the 3B entry only exists in a `-3b` build, so the 0.5B/1.5B registry
    // is bit-for-bit unchanged. `DEFAULT_MODEL_INDEX = 2` selects it for the 3B
    // boot smoke (it is a sibling of the 1.5B entry, not a replacement).
    #[cfg(feature = "arthur-real-brain-boot-3b")]
    ModelEntry {
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        name: "3b",
        path: "/neofs/model-qwen2.5-3b.neom",
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        aliases: QWEN_3B_ALIASES,
    },
    #[cfg(feature = "arthur-real-brain-boot-qwen35")]
    ModelEntry {
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        name: "qwen3.5-0.8b",
        path: "/neofs/model-qwen3.5-0.8b.neom",
        #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
        aliases: QWEN35_ALIASES,
    },
];
const MAX_INPUT_BYTES: usize = 192;
#[cfg(feature = "console-interactive")]
const CONSOLE_PROMPT: &[u8] = b"arthur> ";
#[cfg(feature = "console-interactive")]
const CONSOLE_NEWLINE: &[u8] = b"\n";
#[cfg(feature = "console-interactive")]
const CONSOLE_BYE: &[u8] = b"arthur: bye\n";
#[cfg(feature = "console-interactive")]
const CONSOLE_INPUT_TOO_LONG: &[u8] = b"arthur: input too long\n";
#[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
const V1_CONSOLE_GREETING: &[u8] = b"arthur: Good afternoon. I'm Arthur -- I look after this machine for you.\narthur: Tell me what you'd like, in your own words, and I'll see to it.\narthur: If ever you'd rather do it yourself, \"help\" shows you the way.\n";
#[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
const V1_CONSOLE_LIVENESS_BEGIN: &[u8] = b"arthur: thinking... forward pass running\n";
#[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
const V1_CONSOLE_LIVENESS_DONE: &[u8] = b"arthur: thinking... forward pass complete tokens=";
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_CONTEXT: usize = 3;
/// OT-37: max new tokens for the real-brain reply (capped so the decoded text
/// fits `MAX_REPLY_BYTES`; the model stops earlier at EOS for short answers).
/// Only the real-vocab (inventory) generation path uses it. A reply that
/// reaches it without a stop token ends with `REPLY_CUT_MARKER`.
#[cfg(all(
    feature = "arthur-real-vocab",
    not(feature = "agentic-actions"),
    not(feature = "desktop-session")
))]
const STAGE_B_MAX_NEW_TOKENS: usize = 32;
/// The desktop draws the reply on Arthur's own surface (no 256-byte channel
/// reader), so a normal answer completes: the kernel's `sys_generate` cap.
#[cfg(all(
    feature = "arthur-real-vocab",
    not(feature = "agentic-actions"),
    feature = "desktop-session"
))]
const STAGE_B_MAX_NEW_TOKENS: usize = 256;
/// The Qwen3.5 boot smoke's llama.cpp oracle is exactly 32 greedy tokens.
#[cfg(feature = "arthur-real-brain-boot-qwen35")]
const QWEN35_HELLO_TOKENS: usize = 32;
/// The reply's `max_new`: what `sys_generate` is asked for, and the cap
/// `decode` checks for a cut answer. The Qwen3.5 smoke asks for exactly its
/// oracle's length.
#[cfg(all(
    feature = "arthur-real-brain-boot-qwen35",
    not(feature = "agentic-actions")
))]
const REPLY_MAX_NEW_TOKENS: usize = QWEN35_HELLO_TOKENS;
#[cfg(all(
    feature = "arthur-real-vocab",
    not(feature = "agentic-actions"),
    not(feature = "arthur-real-brain-boot-qwen35")
))]
const REPLY_MAX_NEW_TOKENS: usize = STAGE_B_MAX_NEW_TOKENS;
#[cfg(feature = "arthur-real-brain-boot-qwen35")]
mod qwen35_llama_cpp_golden {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/qwen35_llama_cpp_hello_golden.rs"
    ));
}
#[cfg(feature = "arthur-real-brain-boot-qwen35")]
const QWEN35_HELLO_ORACLE: [u32; QWEN35_HELLO_TOKENS] =
    qwen35_llama_cpp_golden::LLAMA_CPP_QWEN35_HELLO_GREEDY_32;
#[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
const AGENTIC_AST_MAX_NEW_TOKENS: usize = 192;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_PAD: u32 = 0;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_HELLO: u32 = 1;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_ARTHUR: u32 = 2;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_REPLY: u32 = 3;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_READ_TOOL: u32 = 4;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_WRITE_TOOL: u32 = 5;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_READ_PROMPT: u32 = 6;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_WRITE_PROMPT: u32 = 7;
#[cfg(not(feature = "agentic-actions"))]
const TOKEN_READ_MISSING_TOOL: u32 = 8;

#[cfg(feature = "arthur-grants")]
const PLACEHOLDER_GRANT: Grant<'static> = Grant::File {
    path: "",
    rights: FileRights(0),
};
#[cfg(feature = "arthur-grants")]
const PLACEHOLDER_BACKED_GRANT: BackedGrant<'static> = BackedGrant::Empty;

#[cfg(not(feature = "agentic-actions"))]
const BPE_VOCAB: &[BpeVocabEntry<'static>] = &[
    BpeVocabEntry {
        token: b"hello",
        id: TOKEN_HELLO,
    },
    BpeVocabEntry {
        token: b"\xc4\xa0arthur",
        id: TOKEN_ARTHUR,
    },
    BpeVocabEntry {
        token: b"arthur:\xc4\xa0model:\xc4\xa0hello",
        id: TOKEN_REPLY,
    },
    BpeVocabEntry {
        token: b"<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/hello.txt\"}}</tool_call>",
        id: TOKEN_READ_TOOL,
    },
    BpeVocabEntry {
        token: b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"denied\"}}</tool_call>",
        id: TOKEN_WRITE_TOOL,
    },
    BpeVocabEntry {
        token: b"<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/missing.txt\"}}</tool_call>",
        id: TOKEN_READ_MISSING_TOOL,
    },
    BpeVocabEntry {
        token: b"read",
        id: TOKEN_READ_PROMPT,
    },
    BpeVocabEntry {
        token: b"write",
        id: TOKEN_WRITE_PROMPT,
    },
];

#[cfg(not(feature = "agentic-actions"))]
const BPE_MERGES: &[BpeMergeEntry<'static>] = &[
    BpeMergeEntry {
        left: b"h",
        right: b"e",
        rank: 1,
    },
    BpeMergeEntry {
        left: b"l",
        right: b"l",
        rank: 2,
    },
    BpeMergeEntry {
        left: b"he",
        right: b"ll",
        rank: 3,
    },
    BpeMergeEntry {
        left: b"hell",
        right: b"o",
        rank: 4,
    },
    BpeMergeEntry {
        left: b"\xc4\xa0",
        right: b"a",
        rank: 1,
    },
    BpeMergeEntry {
        left: b"\xc4\xa0a",
        right: b"r",
        rank: 2,
    },
    BpeMergeEntry {
        left: b"\xc4\xa0ar",
        right: b"t",
        rank: 3,
    },
    BpeMergeEntry {
        left: b"\xc4\xa0art",
        right: b"h",
        rank: 4,
    },
    BpeMergeEntry {
        left: b"\xc4\xa0arth",
        right: b"u",
        rank: 5,
    },
    BpeMergeEntry {
        left: b"\xc4\xa0arthu",
        right: b"r",
        rank: 6,
    },
];

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    sys_exit(run())
}

#[cfg_attr(feature = "arthur-compositor", allow(unreachable_code))]
fn run() -> i32 {
    #[cfg(feature = "arthur-memory-encryption")]
    ot43_log_process(b"first_instruction", 0);
    let mut input = [0u8; MAX_INPUT_BYTES];
    let mut reply = Vec::new();
    let mut state = ConversationState::new(DEFAULT_CONTEXT_WINDOW_TOKENS);
    let mut backend = SyscallBackend::new();
    #[cfg(feature = "arthur-compositor")]
    let mut showcase = match showcase_surface::ArthurSurface::create() {
        Ok(surface) => surface,
        Err(code) => return code,
    };
    #[cfg(feature = "arthur-compositor")]
    {
        if showcase_logic::output_route(true, true) != showcase_logic::OutputRoute::G1ClientSurface
            || sys_send(CONVERSATION, b"G") != 1
        {
            return 35;
        }
        showcase_log_memory(b"surface_resident");
    }
    #[cfg(feature = "arthur-act")]
    let mut actions = RuntimeActions::new();
    #[cfg(feature = "arthur-memory")]
    let mut memory = MemoryRuntime::new();
    #[cfg(feature = "arthur-grants")]
    if receive_startup_grants().is_err() {
        return 13;
    }

    // F3: Try to load the real vocab sidecar from NeoFS.
    // If unavailable or malformed, falls back to fixture outside the Stage-B
    // real-brain gate.
    #[cfg(feature = "arthur-real-vocab")]
    let vocab_loaded = backend.try_load_sidecar();
    #[cfg(all(feature = "arthur-real-vocab", not(feature = "arthur-real-brain-boot"),))]
    let _ = vocab_loaded;
    #[cfg(all(
        feature = "arthur-real-brain-boot",
        not(feature = "arthur-boot-module"),
        not(feature = "desktop-session")
    ))]
    if !vocab_loaded {
        return 14;
    }
    #[cfg(all(
        feature = "arthur-real-brain-boot",
        not(feature = "arthur-boot-module"),
        not(feature = "desktop-session")
    ))]
    if backend.preload_model().is_err() {
        return 15;
    }
    // B1b: a missing or damaged boot-module model degrades Arthur; it never
    // exits (THINK Q4). Every line is then answered with the state's sentence.
    #[cfg(all(feature = "arthur-boot-module", not(feature = "desktop-session")))]
    let boot_model_state = {
        // B1c (F8): the vocab outcome is typed on the backend; `compose`
        // reads it there.
        let _ = vocab_loaded;
        backend.preload_boot_model()
    };
    // Desktop S1: the session must come up whether or not a model has been
    // provisioned on NeoFS. A missing model degrades Arthur to an honest
    // "no model installed" reply; it never takes the desktop down.
    #[cfg(feature = "desktop-session")]
    {
        #[cfg(not(feature = "arthur-boot-module"))]
        let model_ready = vocab_loaded && backend.preload_model().is_ok();
        #[cfg(feature = "arthur-boot-module")]
        let model_ready = {
            let _ = vocab_loaded;
            backend.preload_boot_model() == model_state_logic::ModelState::Ready
        };
        if model_ready {
            showcase_log_memory(b"model_resident");
        } else {
            stage_b_log_line(b"DESKTOP arthur model=absent reply_mode=no_model_notice");
        }
        // Committed once the session's handshake is done.
        let footer = backend.footer_model(model_ready);
        log_footer_model(&footer);
        showcase.set_footer(footer);
        // L7.1: the boot summary, the first line of the transcript.
        #[cfg(feature = "arthur-boot-time")]
        if let Some(line) = libneo::boot_summary_line(if model_ready {
            "Arthur ready"
        } else {
            "Arthur up (no model)"
        }) {
            let _ = libneo::sys_write(1, b"BOOT-TIME ".as_ptr(), 10);
            let _ = libneo::sys_write(1, line.as_ptr(), line.len());
            let _ = libneo::sys_write(1, b"\n".as_ptr(), 1);
            showcase.write_text(line.as_bytes());
            showcase.write_text(b"\n\n");
        }
        let code = run_desktop_session(
            &mut showcase,
            &mut input,
            &mut reply,
            &mut state,
            &mut backend,
            #[cfg(feature = "arthur-memory")]
            &mut memory,
            model_ready,
        );
        sys_exit(code)
    }
    #[cfg(all(feature = "arthur-compositor", not(feature = "desktop-session")))]
    {
        showcase_log_memory(b"model_resident");
        // Committed with Arthur's first answer.
        let footer = backend.footer_model(true);
        log_footer_model(&footer);
        showcase.set_footer(footer);
        stage_b_log_line(
            b"SHOWCASE footprint discriminator=model-485MB-plus-compositor source=qwen2.5-0.5b",
        );
        let code = run_arthur_compositor(
            &mut showcase,
            &mut input,
            &mut reply,
            &mut state,
            &mut backend,
            #[cfg(feature = "arthur-memory")]
            &mut memory,
        );
        // The model/vocab coexistence proof leaves large userspace allocations.
        // The surface and kernel model are already explicitly destroyed/dropped;
        // exit at the process boundary so kernel reap owns the remaining heap
        // instead of synchronously freeing it before SYS_EXIT.
        sys_exit(code)
    }
    #[cfg(any(
        feature = "arthur-real-brain-boot-15b",
        feature = "arthur-real-brain-boot-3b"
    ))]
    stage_b_log_line(b"pre_forward ready");

    #[cfg(feature = "console-interactive")]
    {
        run_console_interactive(
            &mut input,
            &mut reply,
            &mut state,
            &mut backend,
            #[cfg(feature = "arthur-act")]
            &mut actions,
            #[cfg(feature = "arthur-memory")]
            &mut memory,
        )
    }

    #[cfg(not(feature = "console-interactive"))]
    #[cfg(feature = "arthur-memory-encryption")]
    let mut ot43_turn = 0u64;
    #[cfg(not(feature = "console-interactive"))]
    loop {
        #[cfg(feature = "agentic-ipc-frequency")]
        actions.clear_agentic_measurement();
        #[cfg(feature = "agentic-ipc-frequency")]
        let agentic_frequency_turn = AgenticFrequencyTurn::begin();
        #[cfg(feature = "arthur-memory-encryption")]
        ot43_log_process(b"blocked_recv", ot43_turn);
        let n = sys_recv(CONVERSATION, &mut input);
        if n < 0 {
            return 10;
        }
        #[cfg(feature = "arthur-memory-encryption")]
        {
            ot43_turn = ot43_turn.saturating_add(1);
            ot43_log_process(b"recv_done", ot43_turn);
        }
        let len = n as usize;
        if is_exit(&input[..len]) {
            #[cfg(feature = "arthur-memory-encryption")]
            ot43_log_process(b"exit_received", ot43_turn);
            let _ = sys_send(CONVERSATION, b"arthur: bye");
            #[cfg(feature = "arthur-real-brain-boot")]
            {
                backend.drop_model_for_stage_b();
                sys_exit(0);
            }
            #[cfg(not(feature = "arthur-real-brain-boot"))]
            {
                return 0;
            }
        }

        #[cfg(all(
            feature = "arthur-boot-module",
            not(feature = "desktop-session"),
            not(feature = "arthur-model-from-neofs")
        ))]
        let state_line = model_state_logic::beginner_line(boot_model_state);
        #[cfg(all(feature = "arthur-model-from-neofs", not(feature = "desktop-session")))]
        let state_line = model_state_logic::line_for(boot_model_state, backend.reported_source());
        #[cfg(all(feature = "arthur-boot-module", not(feature = "desktop-session")))]
        if let Some(line) = state_line {
            reply.clear();
            reply.extend_from_slice(b"arthur: ");
            reply.extend_from_slice(line);
            stage_b_log_reply(&reply);
            if sys_send(CONVERSATION, &reply) < 0 {
                return 11;
            }
            continue;
        }

        match respond_to_input(
            &input[..len],
            &mut reply,
            &mut state,
            &mut backend,
            #[cfg(feature = "arthur-act")]
            &mut actions,
            #[cfg(feature = "arthur-memory")]
            &mut memory,
        ) {
            Ok(()) => {
                #[cfg(feature = "agentic-ipc-frequency")]
                if append_agentic_frequency_line(&mut reply, &mut actions, agentic_frequency_turn)
                    .is_err()
                {
                    return 12;
                }
                #[cfg(feature = "arthur-real-brain-boot")]
                stage_b_log_reply(&reply);
                #[cfg(feature = "arthur-memory-encryption")]
                ot43_log_process(b"send_reply", ot43_turn);
                if sys_send(CONVERSATION, &reply) < 0 {
                    return 11;
                }
                #[cfg(feature = "arthur-memory-encryption")]
                ot43_log_process(b"send_reply_done", ot43_turn);
            }
            Err(error) => {
                let message = error_message(error);
                #[cfg(feature = "arthur-memory-encryption")]
                ot43_log_process(b"send_error", ot43_turn);
                if sys_send(CONVERSATION, message) < 0 {
                    return 12;
                }
                #[cfg(feature = "arthur-memory-encryption")]
                ot43_log_process(b"send_error_done", ot43_turn);
            }
        }
    }
}

#[cfg(all(feature = "arthur-compositor", not(feature = "desktop-session")))]
fn run_arthur_compositor(
    surface: &mut showcase_surface::ArthurSurface,
    input: &mut [u8; MAX_INPUT_BYTES],
    reply: &mut Vec<u8>,
    state: &mut ConversationState,
    backend: &mut SyscallBackend,
    #[cfg(feature = "arthur-memory")] memory: &mut MemoryRuntime,
) -> i32 {
    #[cfg(not(any(
        feature = "arthur-compositor-baremetal",
        feature = "arthur-compositor-product-integration"
    )))]
    let _ = (
        &mut *input,
        &mut *reply,
        &mut *state,
        #[cfg(feature = "arthur-memory")]
        &mut *memory,
    );
    if sys_send(CONVERSATION, b"M") != 1 {
        return 50;
    }

    #[cfg(not(any(
        feature = "arthur-compositor-baremetal",
        feature = "arthur-compositor-product-integration"
    )))]
    {
        let mut command = [0u8; 1];
        if sys_recv(CONVERSATION, &mut command) != 1 || command[0] != b'Q' {
            return 51;
        }
        if surface.destroy().is_err() {
            return 52;
        }
        backend.drop_model_for_stage_b();
        return 0;
    }

    #[cfg(any(
        feature = "arthur-compositor-baremetal",
        feature = "arthur-compositor-product-integration"
    ))]
    {
        let mut command = [0u8; 1];
        if sys_recv(CONVERSATION, &mut command) != 1 || command[0] != b'K' {
            return 53;
        }
        stage_b_log_line(b"SHOWCASE input focus=active client=arthur");
        let mut input_len = 0usize;
        loop {
            let byte = match surface.poll_ascii() {
                Ok(Some(byte)) => byte,
                Ok(None) => {
                    core::hint::spin_loop();
                    continue;
                }
                Err(code) => return code,
            };
            stage_b_log_bytes(b"SHOWCASE keystroke ascii=");
            stage_b_log_u64(byte as u64);
            stage_b_log_newline();
            match byte {
                b'\n' if input_len != 0 => {
                    #[cfg(feature = "arthur-compositor-product-integration")]
                    {
                        const EXPECTED_PROMPT: &[u8] = b"hello world";
                        // Receiver-side gate: dropping or reordering any routed
                        // keystroke makes this exact comparison fail before the
                        // fixture-independent inference reply can be produced.
                        if &input[..input_len] != EXPECTED_PROMPT {
                            let _ = surface.destroy();
                            return 61;
                        }
                        stage_b_log_line(
                            b"SHOWCASE product received prompt=hello_world bytes=11 byte_equal=true source=input_focus",
                        );
                    }
                    surface.write_text(b"\n\nArthur> Thinking...\n");
                    if surface.commit_next().is_err() {
                        return 54;
                    }
                    stage_b_log_line(b"SHOWCASE inference start source=input_focus");
                    if respond_to_input(
                        &input[..input_len],
                        reply,
                        state,
                        backend,
                        #[cfg(feature = "arthur-memory")]
                        memory,
                    )
                    .is_err()
                    {
                        let _ = surface.destroy();
                        return 55;
                    }
                    stage_b_log_reply(reply);
                    #[cfg(feature = "arthur-compositor-product-integration")]
                    {
                        if !product_reply_is_coherent(&input[..input_len], reply) {
                            let _ = surface.destroy();
                            return 60;
                        }
                        stage_b_log_bytes(
                            b"SHOWCASE product answer coherent=true bytes_nonzero=true non_echo=true provenance=",
                        );
                        stage_b_log_bytes(backend.active_model_entry().path.as_bytes());
                        stage_b_log_newline();
                    }
                    stage_b_log_line(b"SHOWCASE inference complete forward=real");
                    surface.write_text(b"\nArthur> ");
                    surface.write_text(reply);
                    surface.write_byte(b'\n');
                    if surface.commit_answer().is_err() {
                        return 56;
                    }
                    if surface.destroy().is_err() {
                        return 57;
                    }
                    backend.drop_model_for_stage_b();
                    return 0;
                }
                b'\n' => {}
                0x08 | 0x7f if input_len != 0 => {
                    input_len -= 1;
                    surface.write_byte(0x08);
                    if surface.commit_next().is_err() {
                        return 58;
                    }
                }
                0x08 | 0x7f => {}
                0x20..=0x7e if input_len < input.len() => {
                    input[input_len] = byte;
                    input_len += 1;
                    surface.write_byte(byte);
                    if surface.commit_next().is_err() {
                        return 59;
                    }
                }
                _ => {}
            }
        }
    }
}

/// Desktop S1: Arthur as the resident client of the session WM. Same
/// ADR-073 surface + ADR-074 InputFocus as the showcase, but the loop never
/// ends in production: every line typed on the live keyboard is answered (by
/// the model when one is resident, otherwise with an honest notice).
/// `desktop-session-boot-test` ends the session after the first answered
/// line so the QEMU scenario can read the framebuffer back.
/// `ARTHUR footer model=<"name"|unnamed|none|loading>`: what the footer
/// says about the model.
#[cfg(feature = "arthur-compositor")]
fn log_footer_model(model: &showcase_logic::FooterModel) {
    use showcase_logic::FooterModel;
    stage_b_log_bytes(b"ARTHUR footer model=");
    match model {
        FooterModel::Named(name) => {
            stage_b_log_bytes(b"\"");
            stage_b_log_bytes(name.as_bytes());
            stage_b_log_bytes(b"\"");
        }
        FooterModel::Unnamed => stage_b_log_bytes(b"unnamed"),
        FooterModel::NoModel => stage_b_log_bytes(b"none"),
        FooterModel::Loading => stage_b_log_bytes(b"loading"),
    }
    stage_b_log_newline();
}

#[cfg(feature = "desktop-session")]
fn run_desktop_session(
    surface: &mut showcase_surface::ArthurSurface,
    input: &mut [u8; MAX_INPUT_BYTES],
    reply: &mut Vec<u8>,
    state: &mut ConversationState,
    backend: &mut SyscallBackend,
    #[cfg(feature = "arthur-memory")] memory: &mut MemoryRuntime,
    model_ready: bool,
) -> i32 {
    const NO_MODEL: &[u8] = b"I can hear you, but no model is installed yet.";
    // OT-66 K1b: Arthur's root-form opens reach only his scopes:
    // - outside (`/neofs`): EPERM;
    // - inside (his memory directory): opens;
    // - `..` out of a scope (`/neofs/.arthur/..` is `/neofs`): EPERM;
    // - a name sharing the scope's prefix (`/neofs/.arthurx`): EPERM, before
    //   anything resolves (it doesn't exist);
    // - inside, but asking for a right the scope lacks (`EXECUTE`; not
    //   `MOUNT`, which `mount-authority` masks off a plain open first):
    //   EACCES.
    #[cfg(feature = "arthur-scope-probe-test")]
    {
        let dir_rights = libneo::FileRights::LOOKUP.union(libneo::FileRights::READDIR);
        let probe = |path: &str, rights: libneo::FileRights| -> &'static [u8] {
            match libneo::sys_open(path, libneo::OPEN_ROOT_TEMPORARY, rights) {
                Ok(dir) => {
                    let _ = libneo::sys_close(dir);
                    b"ok"
                }
                Err(libneo::EPERM) => b"eperm",
                Err(libneo::EACCES) => b"eacces",
                Err(_) => b"other",
            }
        };
        stage_b_log_bytes(b"ARTHUR scope outside=");
        stage_b_log_bytes(probe("/neofs", dir_rights));
        stage_b_log_bytes(b" inside=");
        stage_b_log_bytes(probe("/neofs/.arthur/memory", dir_rights));
        stage_b_log_bytes(b" dotdot=");
        stage_b_log_bytes(probe("/neofs/.arthur/..", dir_rights));
        stage_b_log_bytes(b" sibling=");
        stage_b_log_bytes(probe("/neofs/.arthurx", dir_rights));
        stage_b_log_bytes(b" narrow=");
        stage_b_log_bytes(probe(
            "/neofs/.arthur/memory",
            dir_rights.union(libneo::FileRights::EXECUTE),
        ));
        stage_b_log_bytes(b"\n");
    }
    // OT-66 L2 boot test (AAG F3): a client that stalls in its handshake
    // (here before 'M') must not keep the WM from locking.
    #[cfg(feature = "arthur-lock-stall-test")]
    {
        stage_b_log_line(b"DESKTOP arthur lock stall before=M");
        let until = libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC)
            .unwrap_or(0)
            .saturating_add(8_000_000_000);
        while libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC).unwrap_or(u64::MAX) < until {
            libneo::sys_sched_yield();
        }
        stage_b_log_line(b"DESKTOP arthur lock stall over");
    }
    if sys_send(CONVERSATION, b"M") != 1 {
        return 50;
    }
    // F8: under the WM's `desktop-clock` the `K` carries the user's zone id
    // (`[K, lo, hi]`); a bare `K` means no zone (UTC).
    let mut command = [0u8; 3];
    let got = sys_recv(CONVERSATION, &mut command);
    if !(got == 1 || got == 3) || command[0] != b'K' {
        return 53;
    }
    #[cfg(feature = "arthur-clock")]
    let clock_zone = if got == 3 {
        u16::from_le_bytes([command[1], command[2]])
    } else {
        libneo::tz::UTC
    };
    stage_b_log_line(b"DESKTOP arthur input focus=active source=live_hid");
    #[cfg(feature = "desktop-pointer-probe")]
    if !showcase_surface::probe_input_system_authority() {
        return 58;
    }
    #[cfg(not(feature = "desktop-home"))]
    let mut input_len = 0usize;
    // DS-1: the line the user typed into Home's intent field arrives first
    // (b"I" + bytes, possibly empty); answer it as if typed here.
    #[cfg(feature = "desktop-home")]
    let (mut input_len, mut injected) = match recv_intent(surface, input, b"intent") {
        Ok(v) => v,
        Err(code) => return code,
    };
    // Chad #177 item 4 (`desktop_home_cycle`): the WM's silent rounds launch
    // with the intent "cycle"; such a session ends after its one answer.
    #[cfg(feature = "desktop-home-cycle-test")]
    let cycle_round = &input[..input_len] == b"cycle";
    // DS-1b: the broker grants this session a LaunchCap (ADVISORY).
    #[cfg(feature = "desktop-home")]
    let launch_cap = match recv_launch_cap() {
        Ok(cap) => cap,
        Err(code) => return code,
    };
    // DI-1b resize: the WM's configure mailbox, granted after the LaunchCap.
    #[cfg(feature = "desktop-resize")]
    if let Err(code) = surface.recv_configure_mailbox(CONVERSATION) {
        return code;
    }
    #[cfg(all(feature = "desktop-home", feature = "desktop-session-boot-test"))]
    if let Err(code) = launch_probes(surface, launch_cap) {
        return code;
    }
    #[cfg(all(
        feature = "desktop-home",
        not(feature = "desktop-session-boot-test"),
        not(feature = "desktop-lens")
    ))]
    let _ = launch_cap;
    #[cfg(feature = "desktop-lens")]
    let mut lens_token = LENS_TOKEN;
    // The boot test answers twice — the launch intent, then one more after a
    // hold (Esc) and resume — before it ends the session.
    #[cfg(all(feature = "desktop-home", feature = "desktop-session-boot-test"))]
    let mut answers = 0u32;
    // The footer now names the loaded model (or says there is none), in a
    // commit of its own. After the whole handshake: the WM serves commits
    // only from its main loop.
    if surface.commit_footer().is_err() {
        return 63;
    }
    // OT-66 L2 boot test: the first picture, under the lock when Arthur was
    // spawned under it.
    #[cfg(feature = "arthur-lock-probe-test")]
    {
        if let Err(code) = surface.lock_probe(0) {
            return code;
        }
        stage_b_log_line(b"DESKTOP arthur lock probe committed picture=0 busy=sent");
    }
    #[cfg(feature = "arthur-refusal-probe-test")]
    if let Err(code) = surface.probe_refusals() {
        return code;
    }
    #[cfg(feature = "arthur-lock-park-test")]
    stage_b_log_line(b"DESKTOP arthur lock park ready");
    // OT-88 S1 boot test, Arthur standing in for the settings app: Reduce
    // Transparency on, Reduce Motion off; then the idle lock (WM-only), a bad
    // value and a bad key, which the WM must refuse; then a burst the rate
    // limit cuts after the bucket's last two writes. `login_settings_refused`
    // (Arthur as itself): the idle lock off and Reduce Transparency on, both
    // refused.
    #[cfg(feature = "arthur-setting-test")]
    const SETTINGS: &[(u16, u32)] = &[
        (1, 1),
        (2, 2),
        (3, 0),
        (2, 7),
        (9, 0),
        (2, 0),
        (2, 2),
        (2, 0),
        (2, 2),
        (2, 0),
    ];
    #[cfg(feature = "arthur-setting-refused-test")]
    const SETTINGS: &[(u16, u32)] = &[(3, 0), (1, 1)];
    #[cfg(any(
        feature = "arthur-setting-test",
        feature = "arthur-setting-refused-test"
    ))]
    for &(key, value) in SETTINGS {
        let status = match surface.setting(key, value) {
            Ok(status) => status,
            Err(code) => return code,
        };
        stage_b_log_bytes(b"DESKTOP arthur setting key=");
        stage_b_log_u64(u64::from(key));
        stage_b_log_bytes(b" value=");
        stage_b_log_u64(u64::from(value));
        stage_b_log_bytes(b" status=");
        stage_b_log_u64(u64::from(status));
        stage_b_log_newline();
    }
    // Then wait for the WM to end the boot test: the receive fails once it
    // has exited (it sends nothing more on this channel).
    if cfg!(any(
        feature = "arthur-setting-test",
        feature = "arthur-setting-refused-test"
    )) {
        let mut byte = [0u8; 1];
        let _ = sys_recv(CONVERSATION, &mut byte);
        return 0;
    }
    loop {
        // An injected byte (DS-1: the intent's Enter) takes the same path as
        // a typed one, but is not logged as a keystroke.
        #[cfg(feature = "desktop-home")]
        let next = injected.take();
        #[cfg(not(feature = "desktop-home"))]
        let next: Option<u8> = None;
        let typed = next.is_none();
        let byte = match next {
            Some(byte) => byte,
            None => match surface.poll_ascii() {
                Ok(Some(byte)) => {
                    // A9-C: typing brings the newest text back into view.
                    #[cfg(feature = "arthur-scrollback")]
                    surface.follow();
                    byte
                }
                Ok(None) => {
                    // A9-C: PgUp/PgDn, and Up/Down on an empty input line,
                    // scroll the transcript.
                    #[cfg(feature = "arthur-scrollback")]
                    if let Some(key) = surface.take_scroll() {
                        if showcase_logic::scroll_allowed(key, input_len == 0)
                            && surface.scroll(key)
                        {
                            #[cfg(feature = "desktop-session-boot-test")]
                            log_scroll(key, surface.view_top());
                            if surface.commit_next().is_err() {
                                return 64;
                            }
                        }
                        continue;
                    }
                    // DI-1b resize: idle is when a published size is taken.
                    #[cfg(feature = "desktop-resize")]
                    if let Err(code) = surface.poll_configure() {
                        return code;
                    }
                    // OT-22: park until a key (or the mailbox tick).
                    #[cfg(feature = "desktop-park")]
                    surface.park();
                    #[cfg(not(feature = "desktop-park"))]
                    core::hint::spin_loop();
                    continue;
                }
                Err(code) => return code,
            },
        };
        // OT-66 L2 boot test: the first key typed to Arthur ends its turn
        // for good. It never reads or yields again; it draws a new picture
        // and asks for its emblem every 300 ms, so the lock that follows
        // locks a client that holds the focus and keeps committing.
        #[cfg(feature = "arthur-lock-probe-test")]
        if typed {
            log_desktop_keystroke(byte);
            lock_probe_forever(surface);
        }
        // OT-66 L2b boot test: the first key typed to Arthur parks the WM in
        // its reply to a commit Arthur doesn't receive for 12 s, reading its
        // keys meanwhile (a key typed after Super+L must not reach it). Then
        // Arthur waits for the WM to end the boot test.
        #[cfg(feature = "arthur-lock-park-test")]
        if typed {
            log_desktop_keystroke(byte);
            stage_b_log_line(b"DESKTOP arthur lock park begin");
            if let Err(code) = surface.lock_park(12_000_000_000, log_desktop_keystroke) {
                return code;
            }
            stage_b_log_line(b"DESKTOP arthur lock park over");
            surface.until_wm_gone();
            return 0;
        }
        match byte {
            b'\n' if input_len != 0 => {
                if typed {
                    log_desktop_keystroke(byte);
                }
                stage_b_log_bytes(b"DESKTOP arthur line=");
                stage_b_log_bytes(&input[..input_len]);
                stage_b_log_newline();
                surface.write_text(b"\nArthur> ");
                // DS-1c: a listing of `/` is a real, read-only action that needs
                // no model: Arthur blooms the Lens on it through its LaunchCap.
                #[cfg(feature = "desktop-lens")]
                let action = showcase_logic::wants_root_lens(&input[..input_len]);
                #[cfg(not(feature = "desktop-lens"))]
                let action = false;
                // L1.3a: an explicit `read`, `list` or `write` line is done
                // through the WM's permission prompt, never the model.
                #[cfg(feature = "desktop-actions-probe")]
                if let Some(code) = core::str::from_utf8(&input[..input_len])
                    .ok()
                    .and_then(|line| desktop_actions::probe(surface, line, reply))
                {
                    if code != 0 {
                        return code;
                    }
                    surface.write_text(reply);
                    surface.write_text(b"\n\nYou> ");
                    input_len = 0;
                    if surface.commit_next().is_err() {
                        return 56;
                    }
                    continue;
                }
                #[cfg(feature = "desktop-actions")]
                let line = core::str::from_utf8(&input[..input_len]).unwrap_or("");
                #[cfg(feature = "desktop-actions")]
                let handled = match desktop_action_logic::parse(line) {
                    Some(act) => {
                        if let Err(code) =
                            desktop_actions::respond(surface, act, line.trim(), reply)
                        {
                            return code;
                        }
                        surface.write_text(reply);
                        true
                    }
                    None => false,
                };
                #[cfg(not(feature = "desktop-actions"))]
                let handled = false;
                // F8: the time and the date, from the system clock, before
                // the model (which has none).
                #[cfg(feature = "arthur-clock")]
                let handled = handled || clock_reply(surface, &input[..input_len], clock_zone);
                if handled {
                } else if action {
                    #[cfg(feature = "desktop-lens")]
                    {
                        lens_token = lens_token.wrapping_add(1);
                        open_lens(surface, launch_cap, lens_token);
                    }
                } else if model_ready {
                    // A9-D: the WM turns the emblem while the model runs;
                    // the reply's commit ends it.
                    #[cfg(feature = "arthur-busy-emblem")]
                    if surface.show_busy().is_err() {
                        return 54;
                    }
                    #[cfg(not(feature = "arthur-busy-emblem"))]
                    {
                        surface.write_text(b"Thinking...");
                        if surface.commit_next().is_err() {
                            return 54;
                        }
                    }
                    match respond_to_input(
                        &input[..input_len],
                        reply,
                        state,
                        backend,
                        #[cfg(feature = "arthur-memory")]
                        memory,
                    ) {
                        Ok(()) => {
                            #[cfg(not(feature = "arthur-busy-emblem"))]
                            for _ in 0..b"Thinking...".len() {
                                surface.write_byte(0x08);
                            }
                            surface.write_text(reply);
                        }
                        Err(error) => surface.write_text(error_message(error)),
                    }
                } else {
                    let _ = (&mut *reply, &mut *state, &mut *backend);
                    #[cfg(feature = "arthur-memory")]
                    let _ = &mut *memory;
                    surface.write_text(backend.no_model_line(NO_MODEL));
                }
                surface.write_text(b"\n\nYou> ");
                #[cfg(all(feature = "desktop-home", feature = "desktop-session-boot-test"))]
                {
                    answers += 1;
                }
                #[cfg(all(
                    feature = "desktop-home",
                    feature = "desktop-session-boot-test",
                    not(feature = "desktop-home-cycle-test")
                ))]
                let session_done = answers >= 2;
                #[cfg(feature = "desktop-home-cycle-test")]
                let session_done = answers >= 2 || cycle_round;
                #[cfg(all(feature = "desktop-session-boot-test", not(feature = "desktop-home")))]
                let session_done = true;
                #[cfg(feature = "desktop-session-boot-test")]
                if !session_done {
                    input_len = 0;
                    if surface.commit_next().is_err() {
                        return 56;
                    }
                    stage_b_log_line(b"DESKTOP arthur answer composited round=1");
                }
                #[cfg(feature = "desktop-session-boot-test")]
                if session_done {
                    if surface.commit_answer().is_err() {
                        return 56;
                    }
                    stage_b_log_line(b"DESKTOP arthur answer composited serial=0x4152544855520001");
                    // DS-1c: the scene (Arthur + the Lens) stays up so the
                    // scenario can capture it and switch to the Lens and back
                    // (Esc holds this Arthur; Home resumes it); any other key
                    // ends the session.
                    #[cfg(feature = "desktop-lens")]
                    loop {
                        match surface.poll_ascii() {
                            Ok(Some(0x1b)) => {
                                if let Err(code) = surface.yield_focus() {
                                    return code;
                                }
                                stage_b_log_line(b"DESKTOP arthur yield focus=home held=true");
                                if let Err(code) = recv_intent(surface, input, b"resume intent") {
                                    return code;
                                }
                            }
                            Ok(Some(_)) => break,
                            Ok(None) => {
                                // DL-1: the docked scene can still be resized.
                                #[cfg(feature = "desktop-resize")]
                                if let Err(code) = surface.poll_configure() {
                                    return code;
                                }
                                // OT-22: park until a key (or the mailbox tick).
                                #[cfg(feature = "desktop-park")]
                                surface.park();
                                #[cfg(not(feature = "desktop-park"))]
                                core::hint::spin_loop();
                            }
                            Err(code) => return code,
                        }
                    }
                    if surface.destroy().is_err() {
                        return 57;
                    }
                    if model_ready {
                        backend.drop_model_for_stage_b();
                    }
                    return 0;
                }
                #[cfg(not(feature = "desktop-session-boot-test"))]
                {
                    input_len = 0;
                    if surface.commit_next().is_err() {
                        return 56;
                    }
                }
            }
            b'\n' => {}
            // DS-1: Esc hands the keyboard back to Home; this session is held
            // (blocked on its channel) until Home resumes it, possibly with a
            // new intent.
            #[cfg(feature = "desktop-home")]
            0x1b => {
                if let Err(code) = surface.yield_focus() {
                    return code;
                }
                stage_b_log_line(b"DESKTOP arthur yield focus=home held=true");
                match recv_intent(surface, input, b"resume intent") {
                    Ok((len, inject)) => {
                        input_len = len;
                        injected = inject;
                    }
                    Err(code) => return code,
                }
                continue;
            }
            0x08 | 0x7f if input_len != 0 => {
                input_len -= 1;
                surface.write_byte(0x08);
                if surface.commit_next().is_err() {
                    return 58;
                }
            }
            0x08 | 0x7f => {}
            0x20..=0x7e if input_len < input.len() => {
                input[input_len] = byte;
                input_len += 1;
                surface.write_byte(byte);
                if surface.commit_next().is_err() {
                    return 59;
                }
            }
            _ => {}
        }
        // Logged only after the byte's commit has been composited by the WM
        // (commit_next returns once the WM replied, i.e. after its present):
        // the serial line therefore means "this keystroke is on screen".
        // Enter is logged in its own branch, ahead of the line it submits.
        if byte != b'\n' {
            log_desktop_keystroke(byte);
        }
    }
}

/// DS-1c: the first token Arthur's Lens requests count up from.
#[cfg(feature = "desktop-lens")]
const LENS_TOKEN: u64 = 0x4C45_4E53_544F_4B00;

/// DS-1c: bloom the Lens on `/` through the LaunchCap and say so.
#[cfg(feature = "desktop-lens")]
fn open_lens(surface: &mut showcase_surface::ArthurSurface, cap: libneo::Handle, token: u64) {
    // Chad DS-1c F1 boot test: the WM fails the first blooms late; ask again
    // (each refusal spends one of the session's attempts) until shown.
    #[cfg(feature = "desktop-lens-fault-test")]
    for _ in 1..libneo::atrium_launch::MAX_LENS_ATTEMPTS {
        if surface
            .launch(cap, libneo::atrium_launch::MODULE_LENS, token, Some(token))
            .is_ok()
        {
            stage_b_log_line(b"DESKTOP arthur lens opened path=/");
            surface.write_text(b"Here is / in a Lens, read-only.");
            return;
        }
        stage_b_log_line(b"DESKTOP arthur lens refused path=/");
    }
    match surface.launch(cap, libneo::atrium_launch::MODULE_LENS, token, Some(token)) {
        Ok(()) => {
            stage_b_log_line(b"DESKTOP arthur lens opened path=/");
            surface.write_text(b"Here is / in a Lens, read-only.");
        }
        Err(_) => {
            stage_b_log_line(b"DESKTOP arthur lens refused path=/");
            surface.write_text(b"I could not open a Lens on / just now.");
        }
    }
}

/// DS-1b: receive the LaunchCap endpoint the broker grants on the session
/// channel (the same offer / ready / grant / ack exchange as the surface).
#[cfg(feature = "desktop-home")]
fn recv_launch_cap() -> Result<libneo::Handle, i32> {
    let mut offer = [0u8; 1];
    if sys_recv(CONVERSATION, &mut offer) != 1
        || offer[0] != b'O'
        || sys_send(CONVERSATION, b"R") != 1
    {
        return Err(62);
    }
    let cap = libneo::sys_recv_grant(CONVERSATION, libneo::FULL_CHANNEL_RIGHTS).map_err(|_| 63)?;
    if sys_send(CONVERSATION, b"A") != 1 {
        return Err(64);
    }
    Ok(cap)
}

/// DS-1b boot test: what the LaunchCap does and — pinned — what it does NOT.
/// Granted: the allowlisted probe with the matching token on the cap. Refused:
/// a module off the allowlist (status 10) and a mismatched proof on the cap
/// (status 7 — no proof of possession). ADVISORY: a direct
/// `sys_spawn` that bypasses the broker still succeeds today; the kernel
/// SpawnCap rung (OT-66) must flip exactly this assertion.
#[cfg(all(feature = "desktop-home", feature = "desktop-session-boot-test"))]
fn launch_probes(
    surface: &mut showcase_surface::ArthurSurface,
    cap: libneo::Handle,
) -> Result<(), i32> {
    use libneo::atrium_client::RequestError;
    use libneo::atrium_launch::{MODULE_ARTHUR, MODULE_PROBE};
    const TOKEN: u64 = 0x4C41_554E_4348_0001;
    surface
        .launch(cap, MODULE_PROBE, TOKEN, Some(TOKEN))
        .map_err(|_| 66)?;
    stage_b_log_line(b"DESKTOP arthur launch module=probe granted=true");
    if surface.launch(cap, MODULE_ARTHUR, TOKEN, Some(TOKEN)) != Err(RequestError::Refused(10)) {
        return Err(68);
    }
    stage_b_log_line(b"DESKTOP arthur launch module=arthur refused=not-permitted");
    if surface.launch(cap, MODULE_PROBE, TOKEN, Some(TOKEN ^ 2)) != Err(RequestError::Refused(7)) {
        return Err(70);
    }
    stage_b_log_line(b"DESKTOP arthur launch module=probe refused=cap-required");
    // Janus DS-1b: a client that never sends its proof must not trap the WM.
    // With the pointer the broker polls for it (servicing the SAK and the
    // pointer meanwhile) and refuses once its deadline passes.
    // (Not with the Lens: a late proof revokes the LaunchCap the Lens needs.)
    #[cfg(all(feature = "desktop-pointer-probe", not(feature = "desktop-lens")))]
    {
        stage_b_log_line(b"DESKTOP arthur launch module=probe proof=withheld");
        if surface.launch(cap, MODULE_PROBE, TOKEN, None) != Err(RequestError::Refused(7)) {
            return Err(73);
        }
        stage_b_log_line(b"DESKTOP arthur launch module=probe refused=no-proof");
    }
    let (_pid, process) = libneo::sys_spawn("worker", &[]).map_err(|_| 71)?;
    let exit = libneo::sys_wait(process, 0).map_err(|_| 72)?;
    stage_b_log_bytes(b"DESKTOP launchcap advisory direct_spawn=allowed exit=");
    stage_b_log_u64(exit as u64);
    stage_b_log_newline();
    Ok(())
}

/// F8: if `line` asks the time or the date, write the answer from the
/// system clock in `zone` and log it; `false` leaves the line to the model.
#[cfg(feature = "arthur-clock")]
fn clock_reply(
    surface: &mut showcase_surface::ArthurSurface,
    line: &[u8],
    zone: libneo::tz::ZoneId,
) -> bool {
    use libneo::clock_answer::{Question, answer, question};
    use libneo::tz::Basis;
    let Some(q) = question(line) else {
        return false;
    };
    // The status before the clock: a confirmed status means the clock read
    // after it is UTC.
    let basis = Basis::of(libneo::sys_clock_realtime_status());
    let Ok(ns) = libneo::sys_clock_gettime(libneo::CLOCK_REALTIME) else {
        return false;
    };
    let realtime = ns / 1_000_000_000;
    let a = answer(q, realtime as i64, basis, zone);
    surface.write_text(a.as_bytes());
    stage_b_log_bytes(match q {
        Question::Time => b"DESKTOP arthur clock question=time",
        Question::Date => b"DESKTOP arthur clock question=date",
    });
    stage_b_log_bytes(match basis {
        Basis::Utc => b" basis=utc",
        Basis::Rtc => b" basis=rtc",
        Basis::Legacy => b" basis=legacy",
    });
    stage_b_log_bytes(b" zone=");
    stage_b_log_u64(u64::from(zone));
    stage_b_log_bytes(b" realtime_s=");
    stage_b_log_u64(realtime);
    stage_b_log_bytes(b" reply=\"");
    stage_b_log_bytes(a.as_bytes());
    stage_b_log_bytes(b"\"");
    stage_b_log_newline();
    true
}

/// DS-1: receive Home's intent (`b"I"` + printable ASCII, possibly empty)
/// on the session channel; show it as typed and return it with the Enter to
/// inject when it is non-empty.
#[cfg(feature = "desktop-home")]
fn recv_intent(
    surface: &mut showcase_surface::ArthurSurface,
    input: &mut [u8; MAX_INPUT_BYTES],
    what: &[u8],
) -> Result<(usize, Option<u8>), i32> {
    let mut intent = [0u8; 1 + MAX_INPUT_BYTES];
    let len = sys_recv(CONVERSATION, &mut intent);
    if len < 1 || intent[0] != b'I' {
        return Err(60);
    }
    let text = &intent[1..len as usize];
    let text = &text[..text.len().min(input.len())];
    if text.iter().any(|b| !(0x20..=0x7e).contains(b)) {
        return Err(61);
    }
    input[..text.len()].copy_from_slice(text);
    // Chad DS-1a F1: the intent begins this turn; nothing queued before it
    // (typed while held, or during the handoff) is replayed into it.
    let discarded = surface.discard_queued();
    // Logged before the acknowledgement: the WM logs its view switch once it
    // has it, so the two processes' lines never interleave.
    stage_b_log_bytes(b"DESKTOP arthur ");
    stage_b_log_bytes(what);
    stage_b_log_bytes(b" bytes=");
    stage_b_log_u64(text.len() as u64);
    stage_b_log_bytes(b" discarded=");
    stage_b_log_u64(discarded as u64);
    stage_b_log_newline();
    // The WM presents this turn's view only after this acknowledgement.
    if sys_send(CONVERSATION, b"D") != 1 {
        return Err(62);
    }
    if !text.is_empty() {
        surface.write_text(text);
    }
    Ok((text.len(), (!text.is_empty()).then_some(b'\n')))
}

#[cfg(feature = "arthur-lock-probe-test")]
fn lock_probe_forever(surface: &mut showcase_surface::ArthurSurface) -> ! {
    let mut n = 1usize;
    loop {
        if surface.lock_probe(n).is_err() {
            libneo::sys_exit(48);
        }
        if n <= 3 {
            stage_b_log_bytes(b"DESKTOP arthur lock probe committed picture=");
            stage_b_log_u64(n as u64);
            stage_b_log_bytes(b" busy=sent");
            stage_b_log_newline();
        }
        n += 1;
        // Between pictures, a flood the WM can't decode (AAG F2): its
        // channel is never empty, and the lock must still engage.
        let until = libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC)
            .unwrap_or(0)
            .saturating_add(300_000_000);
        let sent = surface.lock_flood(until);
        if n == 3 {
            stage_b_log_bytes(b"DESKTOP arthur lock flood sent=");
            stage_b_log_u64(sent);
            stage_b_log_newline();
        }
    }
}

#[cfg(feature = "desktop-session")]
fn log_desktop_keystroke(byte: u8) {
    stage_b_log_bytes(b"DESKTOP arthur keystroke ascii=");
    stage_b_log_u64(byte as u64);
    stage_b_log_newline();
}

#[cfg(feature = "arthur-compositor-product-integration")]
fn product_reply_is_coherent(input: &[u8], reply: &[u8]) -> bool {
    reply.len() >= 4
        && reply != input
        && reply.iter().any(u8::is_ascii_alphabetic)
        && reply
            .iter()
            .all(|byte| byte.is_ascii_graphic() || byte.is_ascii_whitespace())
}

#[cfg(feature = "arthur-compositor")]
fn showcase_log_memory(stage: &[u8]) {
    stage_b_log_bytes(b"SHOWCASE memory stage=");
    stage_b_log_bytes(stage);
    match sys_sysstat() {
        Ok(stats) => {
            stage_b_log_bytes(b" used_bytes=");
            stage_b_log_u64(stats.memory_used_bytes);
            stage_b_log_bytes(b" frames_used=");
            stage_b_log_u64(stats.frames_used);
            stage_b_log_bytes(b" cores=");
            stage_b_log_u64(stats.cores_online as u64);
        }
        Err(error) => {
            stage_b_log_bytes(b" sysstat_error=");
            stage_b_log_i64(error);
        }
    }
    stage_b_log_newline();
}

fn respond_to_input(
    input: &[u8],
    reply: &mut Vec<u8>,
    state: &mut ConversationState,
    backend: &mut SyscallBackend,
    #[cfg(feature = "arthur-act")] actions: &mut RuntimeActions,
    #[cfg(feature = "arthur-memory")] memory: &mut MemoryRuntime,
) -> Result<(), ConversationError> {
    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    if let Some(response) = backend.respond_model_command(input, reply) {
        return response;
    }

    #[cfg(feature = "arthur-memory")]
    if let Some(response) = memory.respond(input, reply) {
        return response;
    }

    #[cfg(feature = "arthur-grants")]
    if let Some(response) = respond_grant_probe(input, reply) {
        return response;
    }

    let mode = if cfg!(any(
        feature = "arthur-real-infer",
        feature = "agentic-actions"
    )) {
        ReplyMode::Model
    } else {
        ReplyMode::DummyEcho
    };
    #[cfg(feature = "agentic-actions")]
    {
        respond_to_turn_with_agentic_actions(mode, state, backend, actions, input, reply)
    }
    #[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
    {
        respond_to_turn_with_actions(mode, state, backend, actions, input, reply)
    }
    #[cfg(not(feature = "arthur-act"))]
    {
        respond_to_turn(mode, state, backend, input, reply)
    }
}

#[cfg(feature = "console-interactive")]
fn run_console_interactive(
    input: &mut [u8; MAX_INPUT_BYTES],
    reply: &mut Vec<u8>,
    state: &mut ConversationState,
    backend: &mut SyscallBackend,
    #[cfg(feature = "arthur-act")] actions: &mut RuntimeActions,
    #[cfg(feature = "arthur-memory")] memory: &mut MemoryRuntime,
) -> i32 {
    #[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
    v1_console_ready();

    loop {
        console_write_all(CONSOLE_PROMPT);
        let len = match console_read_line(input) {
            Ok(len) => len,
            Err(()) => {
                console_write_all(CONSOLE_INPUT_TOO_LONG);
                continue;
            }
        };
        if is_exit(&input[..len]) {
            console_write_all(CONSOLE_BYE);
            #[cfg(feature = "arthur-real-brain-boot")]
            {
                backend.drop_model_for_stage_b();
                sys_exit(0);
            }
            #[cfg(not(feature = "arthur-real-brain-boot"))]
            {
                return 0;
            }
        }

        match respond_to_input(
            &input[..len],
            reply,
            state,
            backend,
            #[cfg(feature = "arthur-act")]
            actions,
            #[cfg(feature = "arthur-memory")]
            memory,
        ) {
            Ok(()) => console_write_all(reply),
            Err(error) => console_write_all(error_message(error)),
        }
        console_write_all(CONSOLE_NEWLINE);
    }
}

#[cfg(feature = "console-interactive")]
fn console_read_line(out: &mut [u8]) -> Result<usize, ()> {
    let mut len = 0;
    let mut overflow = false;
    loop {
        let byte = console_read_byte();
        match byte {
            b'\r' | b'\n' => {
                console_write_all(CONSOLE_NEWLINE);
                return if overflow { Err(()) } else { Ok(len) };
            }
            0x08 | 0x7f if len > 0 => {
                len -= 1;
                console_write_all(b"\x08 \x08");
            }
            0x08 | 0x7f => {}
            0x00..=0x1f => {}
            byte if len < out.len() => {
                out[len] = byte;
                len += 1;
                console_write_byte(byte);
            }
            _ => overflow = true,
        }
    }
}

#[cfg(feature = "console-interactive")]
fn console_read_byte() -> u8 {
    let mut byte = [0u8; 1];
    loop {
        match libneo::sys_read_fd(libneo::STDIN_FD, &mut byte) {
            1 => return byte[0],
            _ => sys_exit(2),
        }
    }
}

#[cfg(feature = "console-interactive")]
fn console_write_byte(byte: u8) {
    let buf = [byte; 1];
    console_write_all(&buf);
}

#[cfg(feature = "console-interactive")]
fn console_write_all(mut bytes: &[u8]) {
    while !bytes.is_empty() {
        let n = libneo::sys_write(libneo::STDOUT_FD, bytes.as_ptr(), bytes.len());
        if n <= 0 {
            return;
        }
        bytes = &bytes[n as usize..];
    }
}

fn is_exit(bytes: &[u8]) -> bool {
    matches!(
        core::str::from_utf8(bytes),
        Ok("exit") | Ok("exit\n") | Ok("exit\r\n")
    )
}

#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_ticks() -> u64 {
    libneo::sys_sysstat()
        .map(|stats| stats.uptime_ticks)
        .unwrap_or(0)
}

#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_elapsed_ms(start_ticks: u64) -> u64 {
    stage_b_ticks()
        .saturating_sub(start_ticks)
        .saturating_mul(10)
}

/// A9-C boot-test witness: the key and where the view is (`follow` while it
/// shows the newest text).
#[cfg(all(feature = "arthur-scrollback", feature = "desktop-session-boot-test"))]
fn log_scroll(key: showcase_logic::ScrollKey, top: Option<u64>) {
    let name: &[u8] = match key {
        showcase_logic::ScrollKey::PageUp => b"pgup",
        showcase_logic::ScrollKey::PageDown => b"pgdn",
        showcase_logic::ScrollKey::LineUp => b"up",
        showcase_logic::ScrollKey::LineDown => b"down",
        showcase_logic::ScrollKey::Wheel(_) => b"wheel",
    };
    let mut line = [0u8; 64];
    let mut len = 0;
    let mut put = |bytes: &[u8]| {
        line[len..len + bytes.len()].copy_from_slice(bytes);
        len += bytes.len();
    };
    put(b"DESKTOP arthur scroll key=");
    put(name);
    match top {
        Some(mut top) => {
            put(b" view_top=");
            let mut digits = [0u8; 20];
            let mut n = 0;
            loop {
                digits[n] = b'0' + (top % 10) as u8;
                n += 1;
                top /= 10;
                if top == 0 {
                    break;
                }
            }
            digits[..n].reverse();
            put(&digits[..n]);
        }
        None => put(b" view=follow"),
    }
    put(b"\n");
    let _ = libneo::sys_write(1, line.as_ptr(), len);
}

#[cfg(feature = "arthur-real-brain-boot")]
fn stage_b_log_line(bytes: &[u8]) {
    stage_b_log_bytes(bytes);
    stage_b_log_newline();
}

#[cfg(feature = "arthur-real-brain-boot")]
fn stage_b_log_bytes(bytes: &[u8]) {
    let _ = libneo::sys_write(1, bytes.as_ptr(), bytes.len());
}

#[cfg(feature = "arthur-real-brain-boot")]
fn stage_b_log_newline() {
    stage_b_log_bytes(b"\n");
}

#[cfg(feature = "arthur-real-brain-boot")]
fn stage_b_log_u64(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut idx = buf.len();
    if value == 0 {
        stage_b_log_bytes(b"0");
        return;
    }
    while value != 0 {
        idx -= 1;
        buf[idx] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    stage_b_log_bytes(&buf[idx..]);
}

#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_log_i64(value: i64) {
    if value < 0 {
        stage_b_log_bytes(b"-");
        stage_b_log_u64(value.unsigned_abs());
    } else {
        stage_b_log_u64(value as u64);
    }
}

#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_log_token_count(prefix: &[u8], count: usize) {
    stage_b_log_bytes(prefix);
    stage_b_log_u64(count as u64);
    stage_b_log_newline();
}

/// OT-37 ISC-1 instrument: log a token-ID array (capped) so a boot-vs-host diff
/// can localize a divergence at the tokenize/prompt-assembly layer.
#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_log_ids(prefix: &[u8], ids: &[u32]) {
    const MAX_LOGGED: usize = 48;
    stage_b_log_bytes(prefix);
    stage_b_log_u64(ids.len() as u64);
    stage_b_log_bytes(b" [");
    for (i, &id) in ids.iter().take(MAX_LOGGED).enumerate() {
        if i != 0 {
            stage_b_log_bytes(b",");
        }
        stage_b_log_u64(u64::from(id));
    }
    if ids.len() > MAX_LOGGED {
        stage_b_log_bytes(b",...");
    }
    stage_b_log_bytes(b"]");
    stage_b_log_newline();
}

#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_log_decode_done(tokens: usize, ms: u64) {
    let per_token = if tokens == 0 { 0 } else { ms / tokens as u64 };
    stage_b_log_bytes(b"decode done tokens=");
    stage_b_log_u64(tokens as u64);
    stage_b_log_bytes(b" ms=");
    stage_b_log_u64(ms);
    stage_b_log_bytes(b" per_token_ms=");
    stage_b_log_u64(per_token);
    stage_b_log_newline();
}

#[cfg(feature = "arthur-real-brain-boot")]
fn stage_b_log_reply(reply: &[u8]) {
    stage_b_log_bytes(b"reply=\"");
    for &byte in reply {
        match byte {
            b'\\' => stage_b_log_bytes(b"\\\\"),
            b'"' => stage_b_log_bytes(b"\\\""),
            b'\n' => stage_b_log_bytes(b"\\n"),
            b'\r' => stage_b_log_bytes(b"\\r"),
            0x20..=0x7e => stage_b_log_bytes(core::slice::from_ref(&byte)),
            _ => stage_b_log_bytes(b"?"),
        }
    }
    stage_b_log_bytes(b"\"");
    stage_b_log_newline();
}

#[cfg(feature = "arthur-real-brain-boot")]
#[allow(dead_code)]
fn stage_b_log_agentic_ast_kind(ast: &[u8]) {
    let kind = if ast == b"ReadFile(\"/scratch\")" || ast == b"ReadFile(\"/hello.txt\")" {
        Some(b"read_file".as_slice())
    } else if ast
        == b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>"
        || ast
            == b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/forbidden\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>"
    {
        Some(b"write_file".as_slice())
    } else if ast == b"<tool_call>{\"name\":\"confirm\"}</tool_call>" {
        Some(b"confirm".as_slice())
    } else if ast
        == b"<tool_call>{\"name\":\"done\",\"arguments\":{\"summary\":\"multi-step complete\"}}</tool_call>"
    {
        Some(b"done".as_slice())
    } else if ast == b"<tool_call>{\"name\":\"undo\"}</tool_call>" {
        Some(b"undo".as_slice())
    } else {
        None
    };
    if let Some(kind) = kind {
        stage_b_log_bytes(b"agentic_model_ast_kind=");
        stage_b_log_line(kind);
    }
}

fn error_message(error: ConversationError) -> &'static [u8] {
    match error {
        ConversationError::InvalidUtf8 => b"arthur: error: invalid utf8",
        ConversationError::EmptyPrompt => b"arthur: error: empty prompt",
        #[cfg(not(feature = "agentic-actions"))]
        ConversationError::Tokenize => b"arthur: error: tokenize failed",
        #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
        ConversationError::Generate => b"arthur: error: infer failed",
        ConversationError::Decode => b"arthur: error: decode failed",
        #[cfg(feature = "arthur-act")]
        ConversationError::Action => b"arthur: error: action failed",
        ConversationError::ReplyTooLarge => b"arthur: error: reply too large",
    }
}

#[cfg(feature = "arthur-memory")]
struct MemoryRuntime {
    available: bool,
    path: Vec<u8>,
    line: Vec<u8>,
    file: Vec<u8>,
    records: Vec<OwnedMemoryRecord>,
    #[cfg(feature = "arthur-memory-encryption")]
    encryption: MemoryEncryption,
    #[cfg(feature = "arthur-memory-encryption")]
    key_path: Vec<u8>,
    #[cfg(feature = "arthur-memory-encryption")]
    key_file: Vec<u8>,
    #[cfg(feature = "arthur-memory-encryption")]
    write_attempts: u64,
    #[cfg(feature = "arthur-memory-delegation")]
    delegated_caps: [Option<Handle>; 4],
}

#[cfg(feature = "arthur-memory")]
impl MemoryRuntime {
    fn new() -> Self {
        Self {
            available: memory_dir_cap_is_valid(),
            path: Vec::new(),
            line: Vec::new(),
            file: Vec::new(),
            records: Vec::new(),
            #[cfg(feature = "arthur-memory-encryption")]
            encryption: MemoryEncryption::new(),
            #[cfg(feature = "arthur-memory-encryption")]
            key_path: Vec::new(),
            #[cfg(feature = "arthur-memory-encryption")]
            key_file: Vec::new(),
            #[cfg(feature = "arthur-memory-encryption")]
            write_attempts: 0,
            #[cfg(feature = "arthur-memory-delegation")]
            delegated_caps: [None; 4],
        }
    }

    fn respond(
        &mut self,
        input: &[u8],
        reply: &mut Vec<u8>,
    ) -> Option<Result<(), ConversationError>> {
        let text = match core::str::from_utf8(input) {
            Ok(text) => text,
            Err(_) => return Some(Err(ConversationError::InvalidUtf8)),
        };
        let command = match parse_memory_command(text) {
            Some(Ok(command)) => command,
            Some(Err(_)) => {
                reply.clear();
                return Some(push_memory_reply(reply, b"arthur: memory: invalid"));
            }
            None => return None,
        };

        let result = match command {
            MemoryCommand::Remember(record) => self.remember(record, reply),
            MemoryCommand::Recall(kind) => self.recall(kind, reply),
            #[cfg(feature = "arthur-memory-encryption")]
            MemoryCommand::Forget(kind) => self.forget(kind, reply),
            #[cfg(feature = "arthur-memory-forgetting")]
            MemoryCommand::ForgetEverything => self.forget_everything(reply),
            #[cfg(feature = "arthur-memory-delegation")]
            MemoryCommand::Delegate(kind, access) => self.delegate(kind, access, reply),
            #[cfg(feature = "arthur-memory-delegation")]
            MemoryCommand::RevokeDelegation(kind) => self.revoke_delegation(kind, reply),
        };
        Some(result)
    }

    fn remember(
        &mut self,
        record: MemoryRecord<'_>,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        #[cfg(feature = "arthur-memory-encryption")]
        self.ot43_event(b"remember_begin", record.kind);
        reply.clear();
        if !self.available {
            return push_memory_reply(reply, b"arthur: memory: ephemeral");
        }
        if self.append_record(record).is_err() {
            self.available = false;
            return push_memory_reply(reply, b"arthur: memory: ephemeral");
        }
        #[cfg(feature = "arthur-memory-encryption")]
        self.ot43_event(b"remember_done", record.kind);
        push_memory_reply(reply, b"arthur: memory: remembered")
    }

    fn recall(&mut self, kind: MemoryType, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
        #[cfg(feature = "arthur-memory-encryption")]
        self.ot43_event(b"recall_begin", kind);
        reply.clear();
        if !self.available {
            return push_memory_reply(reply, b"arthur: memory: empty");
        }
        if self.load_records(kind).is_err() {
            self.available = false;
            return push_memory_reply(reply, b"arthur: memory: empty");
        }
        let Some(record) = self.records.iter().rev().find(|record| record.kind == kind) else {
            #[cfg(feature = "arthur-memory-encryption")]
            self.ot43_event(b"recall_empty", kind);
            return push_memory_reply(reply, b"arthur: memory: empty");
        };
        #[cfg(feature = "arthur-memory-encryption")]
        self.ot43_event(b"recall_done", kind);
        push_memory_reply(reply, b"arthur: memory: ")?;
        push_memory_reply(reply, record.content.as_bytes())
    }

    /// Crypto-erase a category (M2): destroy the key (NIST 800-88) and reset the
    /// category to a fresh, valid, empty state so a later `remember` re-keys
    /// cleanly. After this, the prior ciphertext is irrecoverable.
    #[cfg(feature = "arthur-memory-encryption")]
    fn forget(&mut self, kind: MemoryType, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
        self.ot43_event(b"forget_begin", kind);
        reply.clear();
        if !self.available {
            return push_memory_reply(reply, b"arthur: memory: ephemeral");
        }
        if self.crypto_erase(kind).is_err() {
            self.available = false;
            return push_memory_reply(reply, b"arthur: memory: ephemeral");
        }
        self.ot43_event(b"forget_done", kind);
        push_memory_reply(reply, b"arthur: memory: forgotten")
    }

    /// M4 total wipe: burn every category key before any `.mem` overwrite, then
    /// leave each existing memory object as a verified overwrite pattern. This is
    /// intentionally irreversible-by-design; the command surface is the explicit
    /// deterministic Arthur memory command, not model-inferred action glue.
    #[cfg(feature = "arthur-memory-forgetting")]
    fn forget_everything(&mut self, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
        self.ot43_event(b"forget_everything_begin", MemoryType::Fact);
        reply.clear();
        if !self.available {
            return push_memory_reply(reply, b"arthur: memory: ephemeral");
        }
        if self.crypto_erase_all().is_err() {
            self.available = false;
            return push_memory_reply(reply, b"arthur: memory: ephemeral");
        }
        self.ot43_event(b"forget_everything_done", MemoryType::Fact);
        push_memory_reply(reply, b"arthur: memory: forgotten everything")
    }

    #[cfg(feature = "arthur-memory-delegation")]
    fn delegate(
        &mut self,
        kind: MemoryType,
        access: MemoryDelegationAccess,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        if !self.available {
            return push_memory_reply(reply, b"arthur: memory: ephemeral");
        }
        if self.delegate_category(kind, access).is_err() {
            return push_memory_reply(reply, b"arthur: memory: delegation failed");
        }
        push_memory_reply(reply, b"arthur: memory: delegated ")?;
        push_memory_reply(reply, kind.as_str().as_bytes())?;
        push_memory_reply(reply, b" ")?;
        push_memory_reply(reply, access.as_str().as_bytes())
    }

    #[cfg(feature = "arthur-memory-delegation")]
    fn revoke_delegation(
        &mut self,
        kind: MemoryType,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        let revoked = self.revoke_delegated_category(kind);
        if revoked {
            push_memory_reply(reply, b"arthur: memory: revoked")
        } else {
            push_memory_reply(reply, b"arthur: memory: revoke noop")
        }
    }

    fn append_record(&mut self, record: MemoryRecord<'_>) -> Result<(), MemoryError> {
        encode_record(record, &mut self.line)?;

        #[cfg(feature = "arthur-memory-encryption")]
        {
            // M2: single-blob whole-file-rewrite (B1 fix). Each `.mem` is ONE AEAD
            // blob under ONE nonce; we decrypt the existing blob, append the new
            // record to the whole plaintext, re-encrypt it all, and overwrite the
            // file. The reader (`load_records`) already decrypts exactly one blob,
            // so it was the *writer* that was wrong — it appended a second blob the
            // reader could never see. The write-ahead ordering (B2 fix) is:
            // reserve nonce (advance counter) -> persist+fsync .key -> encrypt ->
            // write+fsync .mem.
            if !self.encryption.is_valid() {
                return Err(MemoryError::BadContent);
            }

            // 1. Load (or generate) the category key.
            let key_bytes_opt = self.read_key_file(record.kind).ok();
            if self
                .encryption
                .ensure_key(record.kind, key_bytes_opt.as_deref())
                .is_err()
            {
                self.encryption.invalidate();
                return Err(MemoryError::BadContent);
            }

            // 2. Read the existing single-blob .mem (empty if the category is new).
            self.read_category_into_file(record.kind)?;

            // 3. Decrypt it to the existing whole-file plaintext. apply_rollback_safety
            //    first, so a restored old .key can never reuse a committed nonce.
            let mut plaintext = Vec::new();
            #[cfg(feature = "arthur-memory-forgetting")]
            let existing_is_forget_overwrite = is_forget_overwrite_pattern(&self.file);
            #[cfg(not(feature = "arthur-memory-forgetting"))]
            let existing_is_forget_overwrite = false;
            if !self.file.is_empty() && !existing_is_forget_overwrite {
                self.encryption
                    .apply_rollback_safety(record.kind, &self.file);
                if self
                    .encryption
                    .decrypt_for_read(record.kind, &self.file, &mut plaintext)
                    .is_err()
                {
                    // Fail-closed: never silently drop existing memory on a bad blob.
                    self.encryption.invalidate();
                    return Err(MemoryError::BadContent);
                }
            }

            // 4. Append the new record line to the whole-file plaintext.
            plaintext.extend_from_slice(&self.line);

            // 5. Write-ahead: reserve the nonce (advances the counter in RAM),
            //    then persist + fsync the .key DURABLY before touching .mem.
            let nonce_val = match self.encryption.reserve_nonce(record.kind) {
                Ok(n) => n,
                Err(_) => {
                    self.encryption.invalidate();
                    return Err(MemoryError::BadContent);
                }
            };
            if self.persist_key_file(record.kind).is_err() {
                return Err(MemoryError::BadContent);
            }

            // 6. Encrypt the WHOLE plaintext as one blob, then overwrite .mem.
            let mut blob = Vec::new();
            if self
                .encryption
                .encrypt_with_nonce(record.kind, nonce_val, &plaintext, &mut blob)
                .is_err()
            {
                self.encryption.invalidate();
                return Err(MemoryError::BadContent);
            }
            self.write_category_blob(record.kind, &blob)?;
            Ok(())
        }

        #[cfg(not(feature = "arthur-memory-encryption"))]
        {
            // M1: append the plaintext line (UNCHANGED — kill-switch ≡ M1 bit-for-bit).
            let (file, created) = self.open_category(record.kind, true)?;
            if !created && drain_file(file).is_err() {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            if sys_write(file.0, self.line.as_ptr(), self.line.len()) != self.line.len() as i64 {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            if sys_fsync(file) != 0 {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            if sys_close(file) != 0 {
                return Err(MemoryError::BadContent);
            }
            Ok(())
        }
    }

    fn load_records(&mut self, kind: MemoryType) -> Result<(), MemoryError> {
        self.file.clear();
        let file = match self.open_category(kind, false) {
            Ok((file, _created)) => file,
            Err(MemoryError::BadContent) => return Ok(()),
            Err(err) => return Err(err),
        };
        let mut scratch = [0u8; 128];
        loop {
            let n = sys_read(file, &mut scratch);
            if n < 0 {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            if n == 0 {
                break;
            }
            let n = n as usize;
            if self.file.len().saturating_add(n) > MAX_MEMORY_FILE_BYTES {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            self.file.extend_from_slice(&scratch[..n]);
        }
        let _ = sys_close(file);

        #[cfg(feature = "arthur-memory-encryption")]
        {
            // M2: decrypt the encrypted file to plaintext records.
            if !self.encryption.is_valid() || self.file.is_empty() {
                return Ok(());
            }
            #[cfg(feature = "arthur-memory-forgetting")]
            if is_forget_overwrite_pattern(&self.file) {
                self.records.clear();
                return Ok(());
            }

            // Load key for this category.
            let key_bytes_opt = self.read_key_file(kind).ok();
            if self
                .encryption
                .ensure_key(kind, key_bytes_opt.as_deref())
                .is_err()
            {
                self.encryption.invalidate();
                return Err(MemoryError::BadContent);
            }

            // Apply rollback safety (ISC-4b): check counter vs max observed nonce.
            self.encryption.apply_rollback_safety(kind, &self.file);
            // Re-persist key if counter was advanced by rollback detection.
            let _ = self.persist_key_file(kind);

            // Decrypt to plaintext.
            let mut plaintext = Vec::new();
            if self
                .encryption
                .decrypt_for_read(kind, &self.file, &mut plaintext)
                .is_err()
            {
                // ISC-6: fail-closed on auth failure — degrade, never surface unverified bytes.
                self.encryption.invalidate();
                return Err(MemoryError::BadContent);
            }
            let _malformed = parse_records_lossy(&plaintext, &mut self.records);
        }

        #[cfg(not(feature = "arthur-memory-encryption"))]
        {
            // M1: parse plaintext records directly.
            let _malformed = parse_records_lossy(&self.file, &mut self.records);
        }

        Ok(())
    }

    fn open_category(
        &mut self,
        kind: MemoryType,
        create: bool,
    ) -> Result<(FileHandle, bool), MemoryError> {
        category_path(kind, &mut self.path);
        let path = core::str::from_utf8(&self.path).map_err(|_| MemoryError::BadContent)?;
        let rights = FileRights::READ.union(FileRights::WRITE);
        match sys_open(path, OPEN_ROOT_TEMPORARY, rights) {
            Ok(file) => Ok((file, false)),
            Err(ENOENT) if create => sys_open(path, OPEN_CREATE_REGULAR, rights)
                .map(|file| (file, true))
                .map_err(|_| MemoryError::BadContent),
            Err(ENOENT) => Err(MemoryError::BadContent),
            Err(_) => Err(MemoryError::BadContent),
        }
    }

    #[cfg(feature = "arthur-memory-delegation")]
    fn delegate_category(
        &mut self,
        kind: MemoryType,
        access: MemoryDelegationAccess,
    ) -> Result<(), MemoryError> {
        let file_rights = delegation_file_rights(access);
        let (file, _created) = self.open_category(kind, false)?;
        let semantic = file_semantic_byte(file_rights);
        let grantable = match sys_derive(
            Handle(file.0),
            CapRights::new(meta_rights::USE | meta_rights::GRANT, semantic),
        ) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
        };
        let _ = sys_close(file);

        let previous = self.take_delegated_category(kind);
        if let Some(handle) = previous {
            let _ = sys_revoke(handle);
        }

        let offered = CapRights::new(meta_rights::USE, semantic);
        let rc = sys_grant(CONVERSATION, grantable, offered);
        if rc != 0 {
            let _ = sys_close(FileHandle(grantable.0));
            return Err(MemoryError::BadContent);
        }
        self.delegated_caps[memory_type_index(kind)] = Some(grantable);
        Ok(())
    }

    #[cfg(feature = "arthur-memory-delegation")]
    fn take_delegated_category(&mut self, kind: MemoryType) -> Option<Handle> {
        let slot = &mut self.delegated_caps[memory_type_index(kind)];
        let handle = *slot;
        *slot = None;
        handle
    }

    #[cfg(feature = "arthur-memory-delegation")]
    fn revoke_delegated_category(&mut self, kind: MemoryType) -> bool {
        let Some(handle) = self.take_delegated_category(kind) else {
            return false;
        };
        sys_revoke(handle) == 0
    }

    #[cfg(feature = "arthur-memory-encryption")]
    fn key_path_for(&mut self, kind: MemoryType) {
        self.key_path.clear();
        self.key_path
            .extend_from_slice(memory_logic::MEMORY_ROOT_PATH.as_bytes());
        self.key_path.extend_from_slice(kind.as_str().as_bytes());
        self.key_path.extend_from_slice(b".key");
    }

    #[cfg(feature = "arthur-memory-encryption")]
    fn read_key_file(&mut self, kind: MemoryType) -> Result<Vec<u8>, MemoryError> {
        self.key_path_for(kind);
        let path = core::str::from_utf8(&self.key_path).map_err(|_| MemoryError::BadContent)?;
        let rights = FileRights::READ;
        let file =
            sys_open(path, OPEN_ROOT_TEMPORARY, rights).map_err(|_| MemoryError::BadContent)?;

        self.key_file.clear();
        let mut scratch = [0u8; 64];
        loop {
            let n = sys_read(file, &mut scratch);
            if n < 0 {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            if n == 0 {
                break;
            }
            self.key_file.extend_from_slice(&scratch[..n as usize]);
        }
        let _ = sys_close(file);

        if self.key_file.len() != memory_crypto::KEY_FILE_LEN {
            return Err(MemoryError::BadContent);
        }
        Ok(self.key_file.clone())
    }

    #[cfg(feature = "arthur-memory-encryption")]
    fn persist_key_file(&mut self, kind: MemoryType) -> Result<(), MemoryError> {
        // The serialized `key || counter` image to persist write-ahead.
        let key_bytes = self
            .encryption
            .key_bytes(kind)
            .ok_or(MemoryError::BadContent)?;
        self.write_key_raw(kind, &key_bytes)
    }

    /// Overwrite the `.key` file with exactly `bytes` from offset 0.
    ///
    /// Do NOT drain (B1 fix). A fresh open leaves the offset at 0; draining would
    /// seek to EOF and make the write APPEND, which grew the `.key` file by 48
    /// bytes every save and broke `read_key_file`'s exact-length check on the
    /// second write. The `.key` file is ALWAYS exactly `KEY_FILE_LEN` bytes, so a
    /// same-length overwrite fully replaces it (no stale tail).
    #[cfg(feature = "arthur-memory-encryption")]
    fn write_key_raw(&mut self, kind: MemoryType, bytes: &[u8]) -> Result<(), MemoryError> {
        self.ot43_write_attempt(b"key_write_begin", kind, bytes.len());
        self.key_path_for(kind);
        let path = core::str::from_utf8(&self.key_path).map_err(|_| MemoryError::BadContent)?;
        let rights = FileRights::READ.union(FileRights::WRITE);

        let file = sys_open(path, OPEN_ROOT_TEMPORARY, rights)
            .or_else(|_| sys_open(path, OPEN_CREATE_REGULAR, rights))
            .map_err(|_| MemoryError::BadContent)?;

        if sys_write(file.0, bytes.as_ptr(), bytes.len()) != bytes.len() as i64 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        if sys_fsync(file) != 0 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        if sys_close(file) != 0 {
            return Err(MemoryError::BadContent);
        }
        self.ot43_event(b"key_write_done", kind);
        Ok(())
    }

    /// Read the whole single-blob `.mem` file into `self.file`. An absent file
    /// leaves `self.file` empty and returns Ok (a new category has no records).
    /// Enforces `MAX_MEMORY_FILE_BYTES`. A genuine read error returns Err.
    #[cfg(feature = "arthur-memory-encryption")]
    fn read_category_into_file(&mut self, kind: MemoryType) -> Result<(), MemoryError> {
        self.ot43_event(b"category_read_begin", kind);
        self.file.clear();
        let file = match self.open_category(kind, false) {
            Ok((file, _created)) => file,
            Err(MemoryError::BadContent) => return Ok(()),
            Err(err) => return Err(err),
        };
        let mut scratch = [0u8; 128];
        loop {
            let n = sys_read(file, &mut scratch);
            if n < 0 {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            if n == 0 {
                break;
            }
            let n = n as usize;
            if self.file.len().saturating_add(n) > MAX_MEMORY_FILE_BYTES {
                let _ = sys_close(file);
                return Err(MemoryError::BadContent);
            }
            self.file.extend_from_slice(&scratch[..n]);
        }
        let _ = sys_close(file);
        self.ot43_event(b"category_read_done", kind);
        Ok(())
    }

    /// Overwrite the whole `.mem` file with a freshly-encrypted single blob.
    ///
    /// A fresh open positions the offset at 0; we deliberately do NOT drain
    /// (drain would seek to EOF -> append). With M3 delegation enabled, a
    /// truncate-backed rewrite cuts the object to the exact blob length, so a
    /// shrinking rewrite cannot leave stale ciphertext tail bytes. With M3 off,
    /// this remains the M2 grow-only overwrite path.
    #[cfg(feature = "arthur-memory-encryption")]
    fn write_category_blob(&mut self, kind: MemoryType, blob: &[u8]) -> Result<(), MemoryError> {
        self.ot43_write_attempt(b"mem_write_begin", kind, blob.len());
        let (file, _created) = self.open_category(kind, true)?;
        if sys_write(file.0, blob.as_ptr(), blob.len()) != blob.len() as i64 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        #[cfg(feature = "arthur-memory-delegation")]
        if sys_ftruncate(file, blob.len() as u64) != 0 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        if sys_fsync(file) != 0 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        if sys_close(file) != 0 {
            return Err(MemoryError::BadContent);
        }
        self.ot43_event(b"mem_write_done", kind);
        Ok(())
    }

    /// Crypto-erase a category: destroy the old key (NIST 800-88), durably
    /// replace it with a fresh category key, then overwrite the logical `.mem`
    /// object so a later `remember` re-keys cleanly.
    ///
    /// With M3 delegation enabled, any outstanding delegated category cap is
    /// revoked first, then `.mem` is rewritten as a fresh empty blob and truncated
    /// to that exact blob length. With M3 off, this preserves the M2 padding
    /// workaround so the default encrypted path remains unchanged.
    #[cfg(feature = "arthur-memory-encryption")]
    fn crypto_erase(&mut self, kind: MemoryType) -> Result<(), MemoryError> {
        if !self.encryption.is_valid() {
            return Err(MemoryError::BadContent);
        }

        #[cfg(feature = "arthur-memory-forgetting")]
        {
            self.replace_category_key_for_forget(kind)?;
            self.overwrite_category_after_burn(kind)?;
            return Ok(());
        }

        #[cfg(not(feature = "arthur-memory-forgetting"))]
        {
            #[cfg(feature = "arthur-memory-delegation")]
            let _ = self.revoke_delegated_category(kind);

            // Load whatever key currently backs the category (so burn has a key to
            // drop); a missing/corrupt .key is tolerated — we erase regardless.
            let key_bytes_opt = self.read_key_file(kind).ok();
            let _ = self.encryption.ensure_key(kind, key_bytes_opt.as_deref());

            // 1. NIST 800-88: overwrite the .key with CSPRNG bytes (destroys the old
            //    key on disk) and drop it from RAM. A crash here leaves the key gone
            //    — the erase intent holds (data irrecoverable), the safe failure.
            let burn_bytes = self
                .encryption
                .burn_key(kind)
                .map_err(|_| MemoryError::BadContent)?;
            self.write_key_raw(kind, &burn_bytes)?;

            // 2. Measure the existing .mem for M2's padding path. M3 uses truncate.
            self.read_category_into_file(kind)?;
            #[cfg(not(feature = "arthur-memory-delegation"))]
            let old_len = self.file.len();

            // 3. Re-key fresh (keys[idx] is None after burn) and write a valid,
            //    empty-records .mem padded so blob_len >= old_len. Write-ahead the
            //    fresh .key before the .mem (same ordering as a normal save).
            if self.encryption.ensure_key(kind, None).is_err() {
                self.encryption.invalidate();
                return Err(MemoryError::BadContent);
            }
            let nonce_val = self
                .encryption
                .reserve_nonce(kind)
                .map_err(|_| MemoryError::BadContent)?;
            self.persist_key_file(kind)?;

            #[cfg(feature = "arthur-memory-delegation")]
            let rewritten_plaintext = Vec::new();
            #[cfg(not(feature = "arthur-memory-delegation"))]
            let mut rewritten_plaintext = Vec::new();
            #[cfg(not(feature = "arthur-memory-delegation"))]
            {
                // blob_len = pad_len + HEADER_LEN + TAG_LEN; pick pad so blob_len ==
                // old_len (>= old_len for tiny files), keeping it within
                // MAX_MEMORY_FILE_BYTES. This is the M2 no-truncate compatibility path.
                let overhead = memory_crypto::HEADER_LEN + memory_crypto::TAG_LEN;
                let pad_len = old_len.saturating_sub(overhead);
                rewritten_plaintext.resize(pad_len, b'\n');
            }
            let mut blob = Vec::new();
            if self
                .encryption
                .encrypt_with_nonce(kind, nonce_val, &rewritten_plaintext, &mut blob)
                .is_err()
            {
                self.encryption.invalidate();
                return Err(MemoryError::BadContent);
            }
            self.write_category_blob(kind, &blob)
        }
    }

    #[cfg(feature = "arthur-memory-forgetting")]
    fn crypto_erase_all(&mut self) -> Result<(), MemoryError> {
        if !self.encryption.is_valid() {
            return Err(MemoryError::BadContent);
        }
        for kind in MemoryType::ALL {
            self.replace_category_key_for_forget(kind)?;
        }
        for kind in MemoryType::ALL {
            self.overwrite_category_after_burn(kind)?;
        }
        Ok(())
    }

    #[cfg(feature = "arthur-memory-forgetting")]
    fn replace_category_key_for_forget(&mut self, kind: MemoryType) -> Result<(), MemoryError> {
        let _ = self.revoke_delegated_category(kind);

        // Load whatever key currently backs the category so the burn drops it
        // from RAM too. Missing/corrupt key files are already unrecoverable, but
        // we still durably install a fresh replacement key before touching .mem.
        let key_bytes_opt = self.read_key_file(kind).ok();
        let _ = self.encryption.ensure_key(kind, key_bytes_opt.as_deref());
        let burn_bytes = self
            .encryption
            .burn_key(kind)
            .map_err(|_| MemoryError::BadContent)?;
        self.write_key_raw(kind, &burn_bytes)?;

        if self.encryption.ensure_key(kind, None).is_err() {
            self.encryption.invalidate();
            return Err(MemoryError::BadContent);
        }
        self.persist_key_file(kind)
    }

    #[cfg(feature = "arthur-memory-forgetting")]
    fn overwrite_category_after_burn(&mut self, kind: MemoryType) -> Result<(), MemoryError> {
        self.read_category_into_file(kind)?;
        let len = self.file.len();
        if len == 0 {
            return Ok(());
        }
        self.write_category_overwrite_pattern(kind, len)?;
        self.verify_category_overwrite_pattern(kind, len)
    }

    #[cfg(feature = "arthur-memory-forgetting")]
    fn write_category_overwrite_pattern(
        &mut self,
        kind: MemoryType,
        len: usize,
    ) -> Result<(), MemoryError> {
        self.ot43_write_attempt(b"overwrite_pattern_begin", kind, len);
        let (file, _created) = self.open_category(kind, true)?;
        self.file.clear();
        self.file.resize(len, FORGET_OVERWRITE_BYTE);
        if sys_write(file.0, self.file.as_ptr(), self.file.len()) != self.file.len() as i64 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        if sys_ftruncate(file, len as u64) != 0 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        if sys_fsync(file) != 0 {
            let _ = sys_close(file);
            return Err(MemoryError::BadContent);
        }
        if sys_close(file) != 0 {
            return Err(MemoryError::BadContent);
        }
        self.ot43_event(b"overwrite_pattern_done", kind);
        Ok(())
    }

    #[cfg(feature = "arthur-memory-encryption")]
    fn ot43_event(&self, event: &[u8], kind: MemoryType) {
        ot43_log_memory(event, kind, self.write_attempts, 0);
    }

    #[cfg(feature = "arthur-memory-encryption")]
    fn ot43_write_attempt(&mut self, event: &[u8], kind: MemoryType, bytes: usize) {
        self.write_attempts = self.write_attempts.saturating_add(1);
        ot43_log_memory(event, kind, self.write_attempts, bytes as u64);
    }

    #[cfg(feature = "arthur-memory-forgetting")]
    fn verify_category_overwrite_pattern(
        &mut self,
        kind: MemoryType,
        expected_len: usize,
    ) -> Result<(), MemoryError> {
        self.read_category_into_file(kind)?;
        if self.file.len() != expected_len || !is_forget_overwrite_pattern(&self.file) {
            return Err(MemoryError::BadContent);
        }
        Ok(())
    }
}

#[cfg(feature = "arthur-memory")]
fn memory_dir_cap_is_valid() -> bool {
    let mut cursor = 0u64;
    let mut buf = [0u8; 32];
    sys_readdir(MEMORY_DIR, &mut buf, &mut cursor) >= 0
}

#[cfg(feature = "arthur-memory-delegation")]
fn memory_type_index(kind: MemoryType) -> usize {
    match kind {
        MemoryType::Fact => 0,
        MemoryType::Preference => 1,
        MemoryType::Event => 2,
        MemoryType::Relationship => 3,
    }
}

#[cfg(feature = "arthur-memory-delegation")]
fn delegation_file_rights(access: MemoryDelegationAccess) -> FileRights {
    match access {
        MemoryDelegationAccess::Read => FileRights::READ,
        MemoryDelegationAccess::ReadWrite => FileRights::READ.union(FileRights::WRITE),
    }
}

#[cfg(feature = "arthur-memory-delegation")]
fn file_semantic_byte(rights: FileRights) -> u8 {
    (rights.bits() & 0x7f) as u8
}

#[cfg(feature = "arthur-memory-forgetting")]
fn is_forget_overwrite_pattern(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.iter().all(|byte| *byte == FORGET_OVERWRITE_BYTE)
}

#[cfg(feature = "arthur-memory-encryption")]
fn ot43_log_process(event: &[u8], turn: u64) {
    let mut line = Ot43Line::new();
    line.push(b"[ ot43 ] src=neo-arthur scenario=");
    line.push(ot43_scenario());
    line.push(b" tick=");
    line.push_u64(ot43_ticks());
    line.push(b" event=");
    line.push(event);
    line.push(b" turn=");
    line.push_u64(turn);
    line.emit();
}

#[cfg(feature = "arthur-memory-encryption")]
fn ot43_log_memory(event: &[u8], kind: MemoryType, write_attempts: u64, bytes: u64) {
    let mut line = Ot43Line::new();
    line.push(b"[ ot43 ] src=neo-arthur scenario=");
    line.push(ot43_scenario());
    line.push(b" tick=");
    line.push_u64(ot43_ticks());
    line.push(b" event=");
    line.push(event);
    line.push(b" kind=");
    line.push(kind.as_str().as_bytes());
    line.push(b" writes=");
    line.push_u64(write_attempts);
    line.push(b" bytes=");
    line.push_u64(bytes);
    line.emit();
}

#[cfg(feature = "arthur-memory-encryption")]
fn ot43_ticks() -> u64 {
    libneo::sys_sysstat()
        .map(|stats| stats.uptime_ticks)
        .unwrap_or(0)
}

#[cfg(feature = "arthur-memory-encryption")]
fn ot43_scenario() -> &'static [u8] {
    if cfg!(feature = "arthur-memory-forgetting") {
        b"arthur_memory_forgetting"
    } else if cfg!(feature = "arthur-memory-delegation") {
        b"arthur_memory_delegation"
    } else {
        b"arthur_memory_encrypted"
    }
}

#[cfg(feature = "arthur-memory-encryption")]
struct Ot43Line {
    buf: [u8; 192],
    len: usize,
}

#[cfg(feature = "arthur-memory-encryption")]
impl Ot43Line {
    fn new() -> Self {
        Self {
            buf: [0u8; 192],
            len: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        let remaining = self.buf.len().saturating_sub(self.len);
        let count = core::cmp::min(remaining, bytes.len());
        self.buf[self.len..self.len + count].copy_from_slice(&bytes[..count]);
        self.len += count;
    }

    fn push_u64(&mut self, mut value: u64) {
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
        self.push(&buf[idx..]);
    }

    fn emit(&mut self) {
        self.push(b"\n");
        let _ = sys_write(1, self.buf.as_ptr(), self.len);
    }
}

// M1-only: the encrypted (M2) append path does a whole-file rewrite and never
// drains. Gating to `not(encryption)` keeps it from being dead code in M2 builds.
#[cfg(all(feature = "arthur-memory", not(feature = "arthur-memory-encryption")))]
fn drain_file(file: FileHandle) -> Result<(), MemoryError> {
    let mut scratch = [0u8; 128];
    let mut total = 0usize;
    loop {
        let n = sys_read(file, &mut scratch);
        if n < 0 {
            return Err(MemoryError::BadContent);
        }
        if n == 0 {
            return Ok(());
        }
        total = total.saturating_add(n as usize);
        if total > MAX_MEMORY_FILE_BYTES {
            return Err(MemoryError::BadContent);
        }
    }
}

#[cfg(feature = "arthur-memory")]
fn push_memory_reply(reply: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ConversationError> {
    if reply.len().saturating_add(bytes.len()) > conversation_logic::MAX_REPLY_BYTES {
        return Err(ConversationError::ReplyTooLarge);
    }
    reply.extend_from_slice(bytes);
    Ok(())
}

struct SyscallBackend {
    #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
    model: Option<ModelHandle>,
    /// The path `model` was loaded from; `None` for an inherited handle.
    /// The footer names the model from it.
    #[cfg(feature = "arthur-compositor")]
    model_path: Option<&'static str>,
    #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
    active_model: usize,
    #[cfg(not(feature = "agentic-actions"))]
    last_prompt: PromptKind,
    #[cfg(feature = "arthur-real-vocab")]
    real_vocab: Option<RealVocab>,
    #[cfg(feature = "arthur-real-vocab")]
    template: QwenTemplate,
    /// B1b: the errno of the last failed model load, and the state it meant.
    #[cfg(feature = "arthur-boot-module")]
    last_load_errno: i64,
    /// B1c (F8): how the vocab load went; the model is taken only on `Loaded`.
    #[cfg(feature = "arthur-boot-module")]
    vocab_load: model_state_logic::VocabLoad,
    #[cfg(feature = "arthur-boot-module")]
    boot_model_state: model_state_logic::ModelState,
    /// OT-87 R2: `Neofs` once the boot modules had no model and the
    /// installed volume was tried.
    #[cfg(feature = "arthur-model-from-neofs")]
    model_source: model_state_logic::ModelSource,
    /// The same for the vocab, kept apart so its fallback never relabels the
    /// model (`model_state_logic::reported_source`).
    #[cfg(feature = "arthur-model-from-neofs")]
    vocab_source: model_state_logic::ModelSource,
}

impl SyscallBackend {
    const fn new() -> Self {
        Self {
            #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
            model: None,
            #[cfg(feature = "arthur-compositor")]
            model_path: None,
            #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
            active_model: DEFAULT_MODEL_INDEX,
            #[cfg(not(feature = "agentic-actions"))]
            last_prompt: PromptKind::Conversation,
            #[cfg(feature = "arthur-real-vocab")]
            real_vocab: None,
            #[cfg(feature = "arthur-real-vocab")]
            template: QwenTemplate::new(),
            #[cfg(feature = "arthur-boot-module")]
            last_load_errno: 0,
            #[cfg(feature = "arthur-boot-module")]
            vocab_load: model_state_logic::VocabLoad::Absent,
            #[cfg(feature = "arthur-boot-module")]
            boot_model_state: model_state_logic::ModelState::Ready,
            #[cfg(feature = "arthur-model-from-neofs")]
            model_source: model_state_logic::ModelSource::BootModule,
            #[cfg(feature = "arthur-model-from-neofs")]
            vocab_source: model_state_logic::ModelSource::BootModule,
        }
    }

    /// The source the model state is reported with.
    #[cfg(feature = "arthur-model-from-neofs")]
    fn reported_source(&self) -> model_state_logic::ModelSource {
        model_state_logic::reported_source(self.vocab_load, self.vocab_source, self.model_source)
    }

    /// The desktop's no-model sentence: the boot-module state's own sentence
    /// when there is one, otherwise the session's generic notice.
    #[cfg(feature = "desktop-session")]
    fn no_model_line(&self, generic: &'static [u8]) -> &'static [u8] {
        #[cfg(all(
            feature = "arthur-boot-module",
            not(feature = "arthur-model-from-neofs")
        ))]
        if let Some(line) = model_state_logic::beginner_line(self.boot_model_state) {
            return line;
        }
        #[cfg(feature = "arthur-model-from-neofs")]
        if let Some(line) =
            model_state_logic::line_for(self.boot_model_state, self.reported_source())
        {
            return line;
        }
        generic
    }

    /// B1b: load the boot-module model once and name the result. Logs
    /// `ARTHUR model source=boot-module state=<tag>` and, when not ready, the
    /// sentence Arthur will answer with.
    #[cfg(feature = "arthur-boot-module")]
    fn preload_boot_model(&mut self) -> model_state_logic::ModelState {
        #[cfg(not(feature = "arthur-model-from-neofs"))]
        use model_state_logic::beginner_line;
        use model_state_logic::{compose, state_tag};
        // B1c (F8): the vocab is part of the model. `compose` takes the model
        // only when the vocab loaded, so a bad vocab never consumes it.
        let state = compose(self.vocab_load, || match self.model() {
            Ok(_) => Ok(()),
            Err(_) => Err(self.last_load_errno),
        });
        self.boot_model_state = state;
        #[cfg(not(feature = "arthur-model-from-neofs"))]
        stage_b_log_bytes(b"ARTHUR model source=boot-module state=");
        #[cfg(feature = "arthur-model-from-neofs")]
        {
            stage_b_log_bytes(b"ARTHUR model source=");
            stage_b_log_bytes(model_state_logic::source_tag(self.reported_source()));
            stage_b_log_bytes(b" state=");
        }
        stage_b_log_bytes(state_tag(state));
        stage_b_log_newline();
        #[cfg(not(feature = "arthur-model-from-neofs"))]
        if let Some(line) = beginner_line(state) {
            stage_b_log_bytes(b"ARTHUR says: ");
            stage_b_log_line(line);
        }
        #[cfg(feature = "arthur-model-from-neofs")]
        if let Some(line) = model_state_logic::line_for(state, self.reported_source()) {
            stage_b_log_bytes(b"ARTHUR says: ");
            stage_b_log_line(line);
        }
        state
    }

    #[cfg(feature = "arthur-real-vocab")]
    fn try_load_sidecar(&mut self) -> bool {
        #[cfg(feature = "arthur-real-brain-boot")]
        let read_start = stage_b_ticks();
        // Heap, not stack: even the fixture sidecar buffer exceeds the 64 KiB
        // user stack, and the Stage-B real sidecar is about 8 MiB.
        let mut buf = alloc::vec![0u8; VOCAB_READ_BUF_LEN];
        #[cfg(not(feature = "arthur-model-from-neofs"))]
        let opened = libneo::sys_open(
            VOCAB_SIDECAR_PATH,
            OPEN_ROOT_TEMPORARY,
            libneo::FileRights::READ,
        );
        // OT-87 R2: no vocab module at all ⇒ the installed volume's.
        #[cfg(feature = "arthur-model-from-neofs")]
        let opened = match libneo::sys_open(
            VOCAB_SIDECAR_PATH,
            OPEN_ROOT_TEMPORARY,
            libneo::FileRights::READ,
        ) {
            Err(model_state_logic::ENOENT) => {
                self.vocab_source = model_state_logic::ModelSource::Neofs;
                libneo::sys_open(
                    NEOFS_VOCAB_PATH,
                    OPEN_ROOT_TEMPORARY,
                    libneo::FileRights::READ,
                )
            }
            other => other,
        };
        let file = match opened {
            Ok(f) => f,
            Err(_errno) => {
                #[cfg(feature = "arthur-boot-module")]
                {
                    self.vocab_load = model_state_logic::vocab_load_for_error(_errno);
                }
                #[cfg(feature = "arthur-real-brain-boot")]
                stage_b_log_line(b"vocab_load failed reason=open");
                return false;
            }
        };

        let mut total = 0usize;
        loop {
            let remaining = buf.len().saturating_sub(total);
            if remaining == 0 {
                let _ = libneo::sys_close(file);
                #[cfg(feature = "arthur-boot-module")]
                {
                    self.vocab_load = model_state_logic::VocabLoad::Unparseable;
                }
                #[cfg(feature = "arthur-real-brain-boot")]
                stage_b_log_line(b"vocab_load failed reason=buffer_full");
                return false;
            }
            let read_len = remaining.min(VOCAB_READ_CHUNK_LEN);
            let n = libneo::sys_read(file, &mut buf[total..total + read_len]);
            if n < 0 {
                let _ = libneo::sys_close(file);
                #[cfg(feature = "arthur-boot-module")]
                {
                    self.vocab_load = model_state_logic::vocab_load_for_error(n as i64);
                }
                #[cfg(feature = "arthur-real-brain-boot")]
                stage_b_log_line(b"vocab_load failed reason=read");
                return false;
            }
            total += n as usize;
            if n == 0 {
                break;
            }
        }
        let _ = libneo::sys_close(file);
        #[cfg(feature = "arthur-real-brain-boot")]
        let parse_start = stage_b_ticks();

        #[cfg(not(feature = "arthur-vocab-binary"))]
        let built = vocab_sidecar::try_build_real_vocab(&buf[..total]);
        // The buffer becomes the vocab: trimmed to the file, then validated
        // in one pass and kept.
        #[cfg(feature = "arthur-vocab-binary")]
        let built = {
            buf.truncate(total);
            buf.shrink_to_fit();
            vocab_sidecar::try_build_binary_vocab(buf).ok()
        };
        if let Some(real_vocab) = built {
            #[cfg(feature = "arthur-real-brain-boot")]
            {
                stage_b_log_bytes(b"vocab_load tokens=");
                stage_b_log_u64(real_vocab.token_count as u64);
                stage_b_log_bytes(b" bytes=");
                stage_b_log_u64(total as u64);
                // Startup latency (A6): where Arthur's vocab time goes.
                stage_b_log_bytes(b" read_ms=");
                stage_b_log_u64(parse_start.saturating_sub(read_start).saturating_mul(10));
                stage_b_log_bytes(b" parse_ms=");
                stage_b_log_u64(stage_b_elapsed_ms(parse_start));
                #[cfg(feature = "arthur-vocab-binary")]
                stage_b_log_bytes(b" format=neovocb1");
                stage_b_log_newline();
            }
            self.real_vocab = Some(real_vocab);
            #[cfg(feature = "arthur-boot-module")]
            {
                self.vocab_load = model_state_logic::VocabLoad::Loaded;
            }
            return true;
        }
        #[cfg(feature = "arthur-boot-module")]
        {
            self.vocab_load = model_state_logic::VocabLoad::Unparseable;
        }
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"vocab_load failed reason=parse");
        false
    }

    #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
    fn active_model_entry(&self) -> &'static ModelEntry {
        &MODEL_REGISTRY[self.active_model]
    }

    #[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
    fn model(&mut self) -> Result<ModelHandle, ConversationError> {
        if let Some(model) = self.model {
            return Ok(model);
        }
        let inherited = INHERITED_MODEL;
        let mut query = ModelQueryDesc::empty();
        if sys_model_query(inherited, &mut query) == 0 {
            self.model = Some(inherited);
            return Ok(inherited);
        }
        let entry = self.active_model_entry();
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_bytes(b"model_load start path=");
            stage_b_log_bytes(entry.path.as_bytes());
            stage_b_log_newline();
        }
        #[cfg(not(feature = "arthur-model-from-neofs"))]
        let loaded = sys_model_load(entry.path, OPEN_ROOT_TEMPORARY).map_err(|_errno| {
            #[cfg(feature = "arthur-boot-module")]
            {
                self.last_load_errno = _errno;
            }
            ConversationError::Generate
        })?;
        // OT-87 R2: no model module at all ⇒ the installed volume's. A
        // damaged module is not skipped: it stays the answer.
        #[cfg(feature = "arthur-model-from-neofs")]
        let (loaded, loaded_path) = {
            let mut path = entry.path;
            let mut result = sys_model_load(path, OPEN_ROOT_TEMPORARY);
            if path == MODEL_05B_PATH && matches!(result, Err(model_state_logic::ENOENT)) {
                self.model_source = model_state_logic::ModelSource::Neofs;
                path = NEOFS_MODEL_05B_PATH;
                stage_b_log_bytes(b"model_load start path=");
                stage_b_log_bytes(path.as_bytes());
                stage_b_log_newline();
                result = sys_model_load(path, OPEN_ROOT_TEMPORARY);
            }
            let loaded = result.map_err(|errno| {
                self.last_load_errno = errno;
                ConversationError::Generate
            })?;
            (loaded, path)
        };
        self.model = Some(loaded);
        #[cfg(feature = "arthur-compositor")]
        {
            #[cfg(not(feature = "arthur-model-from-neofs"))]
            {
                self.model_path = Some(entry.path);
            }
            #[cfg(feature = "arthur-model-from-neofs")]
            {
                self.model_path = Some(loaded_path);
            }
        }
        #[cfg(all(
            feature = "arthur-model-from-neofs",
            not(feature = "arthur-compositor")
        ))]
        let _ = loaded_path;
        Ok(loaded)
    }

    /// What the footer says about the model: its name when one is loaded
    /// from a path, never a default.
    #[cfg(feature = "arthur-compositor")]
    fn footer_model(&self, ready: bool) -> showcase_logic::FooterModel {
        use showcase_logic::{FooterModel, model_display_name};
        if !ready || self.model.is_none() {
            return FooterModel::NoModel;
        }
        match self.model_path.and_then(model_display_name) {
            Some(name) => FooterModel::Named(name),
            None => FooterModel::Unnamed,
        }
    }

    /// OT-37 route discriminator: is the loaded model a real autoregressive LM
    /// (the inventory brain) or the embedding-only CI fixture? Mirrors the
    /// kernel's `is_stage_b_inventory_model` exactly (no graph inputs, outputs, or
    /// nodes) so the boot routes to the same path the kernel will accept.
    #[cfg(feature = "arthur-real-vocab")]
    fn model_is_inventory(&mut self) -> Result<bool, ConversationError> {
        let model = self.model()?;
        let mut query = ModelQueryDesc::empty();
        if sys_model_query(model, &mut query) != 0 {
            return Err(ConversationError::Generate);
        }
        let inventory = query.input_count == 0 && query.output_count == 0 && query.node_count == 0;
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_bytes(if inventory {
                b"model_kind=inventory(real-lm)\n"
            } else {
                b"model_kind=graph(fixture)\n"
            });
        }
        Ok(inventory)
    }

    /// OT-37 real-brain generation: hand the full templated context to the kernel
    /// `sys_generate` (proven forward + greedy sampler + EOS-stop loop) and return
    /// the real generated tokens. Replaces the old stub that fed clamped fixture
    /// tokens to the forward and then hardcoded the EOS token as the reply.
    #[cfg(all(feature = "arthur-real-vocab", not(feature = "agentic-actions")))]
    fn generate_real_brain(
        &mut self,
        context: &[u32],
        out: &mut Vec<u32>,
    ) -> Result<(), ConversationError> {
        let model = self.model()?;
        let eos = self.real_vocab.as_ref().and_then(|v| v.eos_token_id);
        #[cfg(feature = "arthur-real-brain-boot-qwen35")]
        let _ = context;
        #[cfg(feature = "arthur-real-brain-boot-qwen35")]
        let qwen35_prompt = self
            .real_vocab
            .as_ref()
            .ok_or(ConversationError::Generate)?
            .tokenizer
            .encode("Hello")
            .map_err(|_| ConversationError::Generate)?;
        #[cfg(feature = "arthur-real-brain-boot-qwen35")]
        let context = qwen35_prompt.as_slice();
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_line(b"generate sys_generate enter");
            stage_b_log_ids(b"prompt_ids=", context);
        }
        let mut buf = [0u32; STAGE_B_MAX_NEW_TOKENS];
        let max_new = REPLY_MAX_NEW_TOKENS;
        #[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
        v1_console_liveness_begin();
        let count = libneo::sys_generate(model, context, &mut buf[..max_new], max_new as u32, eos)
            .map_err(|_| ConversationError::Generate)?;
        if count == 0 {
            return Err(ConversationError::Generate);
        }
        #[cfg(feature = "arthur-real-brain-boot-qwen35")]
        if count != QWEN35_HELLO_ORACLE.len() || buf[..count] != QWEN35_HELLO_ORACLE {
            stage_b_log_ids(b"qwen35_oracle_mismatch=", &buf[..count]);
            return Err(ConversationError::Generate);
        }
        #[cfg(feature = "arthur-real-brain-boot-qwen35")]
        stage_b_log_line(b"qwen35_token_oracle=ok prompt=Hello tokens=32");
        out.extend_from_slice(&buf[..count]);
        #[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
        v1_console_liveness_done(count);
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_ids(b"generated_ids=", &buf[..count]);
        Ok(())
    }

    #[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
    fn decode_agentic_action_with_model(
        &mut self,
        user_text: &str,
        available_actions: &[AgenticActionKind],
        out: &mut Vec<u8>,
    ) -> Result<bool, ConversationError> {
        if self.real_vocab.is_none() {
            return Ok(false);
        }
        let model = self.model()?;
        let inventory_model = self.model_is_inventory()?;
        let real_vocab = self
            .real_vocab
            .as_ref()
            .ok_or(ConversationError::Generate)?;

        let mut prompt = Vec::new();
        if inventory_model {
            if self
                .template
                .frame_turn_ids(
                    user_text,
                    |segment| real_vocab.tokenizer.encode(segment).ok(),
                    &mut prompt,
                )
                .is_none()
            {
                return Ok(false);
            }
        } else {
            let Ok(tokens) = real_vocab.tokenizer.encode(user_text) else {
                return Ok(false);
            };
            if tokens.is_empty() {
                return Ok(false);
            }
            prompt.extend_from_slice(&tokens);
        }

        let mut asts = Vec::new();
        push_agentic_candidate_asts(user_text, available_actions, &mut asts)
            .map_err(|_| ConversationError::Decode)?;
        let mut flat_tokens = Vec::new();
        let mut sequences = Vec::new();
        let mut max_new = 0usize;
        if inventory_model {
            Self::push_tokenized_agentic_candidates(
                &asts,
                user_text,
                real_vocab,
                &mut flat_tokens,
                &mut sequences,
                &mut max_new,
            );
        } else {
            Self::push_fixture_agentic_candidates(
                &asts,
                &mut flat_tokens,
                &mut sequences,
                &mut max_new,
            );
        }
        if sequences.is_empty() || max_new == 0 {
            return Ok(false);
        }
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_bytes(b"agentic_model_mode=");
            let mode: &[u8] = if inventory_model {
                b"inventory"
            } else {
                b"graph-fixture"
            };
            stage_b_log_line(mode);
            stage_b_log_bytes(b"agentic_candidates=");
            stage_b_log_u64(sequences.len() as u64);
            stage_b_log_bytes(b" candidate_tokens=");
            stage_b_log_u64(flat_tokens.len() as u64);
            stage_b_log_newline();
        }
        let max_new = max_new.min(AGENTIC_AST_MAX_NEW_TOKENS);
        let mask_bits = real_vocab.token_count;
        if mask_bits == 0 || mask_bits > u32::MAX as usize {
            return Err(ConversationError::Decode);
        }
        let mask_bytes = mask_bits.checked_add(7).ok_or(ConversationError::Decode)? / 8;
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_bytes(b"agentic_mask_bits=");
            stage_b_log_u64(mask_bits as u64);
            stage_b_log_bytes(b" mask_bytes=");
            stage_b_log_u64(mask_bytes as u64);
            stage_b_log_newline();
        }
        #[cfg(feature = "arthur-reversibility-stakes")]
        if !inventory_model && Self::use_a3_stakes_fixture_decode(user_text) {
            let action = fallback_agentic_action(user_text, available_actions);
            let mut ast = String::new();
            render_agentic_ast(action, &mut ast).map_err(|_| ConversationError::Decode)?;
            if agentic_fixture_oracle_token_for_ast(&ast).is_none() {
                return Ok(false);
            }
            #[cfg(feature = "arthur-real-brain-boot")]
            {
                stage_b_log_line(b"agentic_model_fallback reason=a3_stakes_fixture");
                stage_b_log_bytes(b"agentic_model_ast=");
                stage_b_log_bytes(ast.as_bytes());
                stage_b_log_newline();
                stage_b_log_agentic_ast_kind(ast.as_bytes());
            }
            out.clear();
            out.extend_from_slice(ast.as_bytes());
            return Ok(true);
        }
        let mut mask = alloc::vec![0u8; mask_bytes];
        let mut context = prompt;
        let mut generated = Vec::new();
        for _step in 0..max_new {
            if !Self::fill_agentic_next_token_mask(
                &generated,
                &flat_tokens,
                &sequences,
                &mut mask,
                mask_bits,
            )? {
                #[cfg(feature = "arthur-real-brain-boot")]
                stage_b_log_line(b"agentic_model_fallback reason=mask_deadend");
                return Ok(false);
            }
            let mut next = 0u32;
            let Ok(token) =
                sys_generate_masked_step(model, &context, &mut next, &mask, mask_bits as u32, 0)
            else {
                #[cfg(feature = "arthur-real-brain-boot")]
                stage_b_log_line(b"agentic_model_fallback reason=masked_syscall");
                return Ok(false);
            };
            generated.push(token);
            context.push(token);
            if real_vocab.eos_token_id == Some(token)
                || Self::agentic_candidate_is_complete(&generated, &flat_tokens, &sequences)?
            {
                break;
            }
        }
        if generated.is_empty() {
            #[cfg(feature = "arthur-real-brain-boot")]
            stage_b_log_line(b"agentic_model_fallback reason=stalled");
            return Ok(false);
        }
        let Ok(ast) = real_vocab.tokenizer.decode(&generated) else {
            #[cfg(feature = "arthur-real-brain-boot")]
            stage_b_log_line(b"agentic_model_fallback reason=decode");
            return Ok(false);
        };
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_bytes(b"agentic_model_ast=");
            stage_b_log_bytes(ast.as_bytes());
            stage_b_log_newline();
            stage_b_log_agentic_ast_kind(ast.as_bytes());
        }
        out.clear();
        out.extend_from_slice(ast.as_bytes());
        Ok(true)
    }

    #[cfg(all(
        feature = "agentic-actions",
        feature = "arthur-real-vocab",
        feature = "arthur-reversibility-stakes"
    ))]
    fn use_a3_stakes_fixture_decode(user_text: &str) -> bool {
        user_text.starts_with("multi-step goal=")
            || user_text.eq_ignore_ascii_case("confirm")
            || user_text.eq_ignore_ascii_case("read /scratch")
    }

    #[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
    fn fill_agentic_next_token_mask(
        generated: &[u32],
        flat_tokens: &[u32],
        sequences: &[GenerateConstrainedSequenceDesc],
        mask: &mut [u8],
        mask_bits: usize,
    ) -> Result<bool, ConversationError> {
        mask.fill(0);
        let mut any = false;
        for sequence in sequences {
            let start = sequence.offset as usize;
            let end = start
                .checked_add(sequence.len as usize)
                .ok_or(ConversationError::Decode)?;
            let tokens = flat_tokens
                .get(start..end)
                .ok_or(ConversationError::Decode)?;
            if tokens.len() <= generated.len() {
                continue;
            }
            if tokens.get(..generated.len()) != Some(generated) {
                continue;
            }
            let token = tokens[generated.len()] as usize;
            if token >= mask_bits {
                return Err(ConversationError::Decode);
            }
            mask[token / 8] |= 1u8 << (token % 8);
            any = true;
        }
        Ok(any)
    }

    #[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
    fn agentic_candidate_is_complete(
        generated: &[u32],
        flat_tokens: &[u32],
        sequences: &[GenerateConstrainedSequenceDesc],
    ) -> Result<bool, ConversationError> {
        for sequence in sequences {
            let start = sequence.offset as usize;
            let end = start
                .checked_add(sequence.len as usize)
                .ok_or(ConversationError::Decode)?;
            if flat_tokens
                .get(start..end)
                .ok_or(ConversationError::Decode)?
                == generated
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
    fn push_tokenized_agentic_candidates(
        asts: &[String],
        _user_text: &str,
        real_vocab: &RealVocab,
        flat_tokens: &mut Vec<u32>,
        sequences: &mut Vec<GenerateConstrainedSequenceDesc>,
        max_new: &mut usize,
    ) {
        for ast in asts {
            let Ok(tokens) = real_vocab.tokenizer.encode(ast) else {
                continue;
            };
            if tokens.is_empty() {
                continue;
            }
            let offset = flat_tokens.len();
            *max_new = (*max_new).max(tokens.len());
            flat_tokens.extend_from_slice(&tokens);
            sequences.push(GenerateConstrainedSequenceDesc {
                offset: offset as u32,
                len: tokens.len() as u32,
            });
        }
    }

    #[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
    fn push_fixture_agentic_candidates(
        asts: &[String],
        flat_tokens: &mut Vec<u32>,
        sequences: &mut Vec<GenerateConstrainedSequenceDesc>,
        max_new: &mut usize,
    ) {
        for ast in asts {
            let Some(token) = agentic_fixture_oracle_token_for_ast(ast) else {
                continue;
            };
            let offset = flat_tokens.len();
            *max_new = (*max_new).max(1);
            flat_tokens.push(token);
            sequences.push(GenerateConstrainedSequenceDesc {
                offset: offset as u32,
                len: 1,
            });
        }
    }

    #[cfg(all(
        feature = "arthur-real-brain-boot",
        not(feature = "arthur-boot-module")
    ))]
    fn preload_model(&mut self) -> Result<(), ConversationError> {
        self.model().map(|_| ())
    }

    #[cfg(feature = "arthur-real-brain-boot")]
    fn drop_model_for_stage_b(&mut self) {
        if let Some(model) = self.model.take() {
            stage_b_log_line(b"model_drop start");
            let _ = sys_model_drop(model);
            stage_b_log_line(b"model_drop done");
        }
    }

    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    fn respond_model_command(
        &mut self,
        input: &[u8],
        reply: &mut Vec<u8>,
    ) -> Option<Result<(), ConversationError>> {
        let command = parse_model_command(input)?;
        Some(match command {
            ModelCommand::List => self.list_models(reply),
            ModelCommand::Switch(name) => self.switch_model(name, reply),
        })
    }

    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    fn list_models(&self, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
        reply.clear();
        push_reply_bytes(reply, b"arthur: model: active ")?;
        push_reply_bytes(reply, self.active_model_entry().name.as_bytes())?;
        push_reply_bytes(reply, b" available ")?;
        for (idx, entry) in MODEL_REGISTRY.iter().enumerate() {
            if idx != 0 {
                push_reply_bytes(reply, b",")?;
            }
            push_reply_bytes(reply, entry.name.as_bytes())?;
        }
        Ok(())
    }

    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    fn switch_model(&mut self, name: &str, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
        let Some(model_index) = find_model_index(name) else {
            return self.model_switch_unknown(name, reply);
        };
        let entry = &MODEL_REGISTRY[model_index];
        let loaded = match sys_model_load(entry.path, OPEN_ROOT_TEMPORARY) {
            Ok(model) => model,
            Err(_) => return self.model_switch_load_failed(entry, reply),
        };
        if let Some(previous) = self.model.replace(loaded) {
            let _ = sys_model_drop(previous);
        }
        self.active_model = model_index;
        self.model_switch_ok(entry, reply)
    }

    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    fn model_switch_ok(
        &self,
        entry: &ModelEntry,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        push_reply_bytes(reply, b"arthur: model: active ")?;
        push_reply_bytes(reply, entry.name.as_bytes())
    }

    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    fn model_switch_unknown(
        &self,
        name: &str,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        push_reply_bytes(reply, b"arthur: model: unknown ")?;
        push_reply_bytes(reply, name.as_bytes())?;
        push_reply_bytes(reply, b" active ")?;
        push_reply_bytes(reply, self.active_model_entry().name.as_bytes())
    }

    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    fn model_switch_load_failed(
        &self,
        entry: &ModelEntry,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        push_reply_bytes(reply, b"arthur: model: load failed ")?;
        push_reply_bytes(reply, entry.name.as_bytes())?;
        push_reply_bytes(reply, b" active ")?;
        push_reply_bytes(reply, self.active_model_entry().name.as_bytes())
    }
}

#[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
impl Drop for SyscallBackend {
    fn drop(&mut self) {
        if let Some(model) = self.model.take() {
            #[cfg(feature = "arthur-real-brain-boot")]
            stage_b_log_line(b"model_drop start");
            let _ = sys_model_drop(model);
            #[cfg(feature = "arthur-real-brain-boot")]
            stage_b_log_line(b"model_drop done");
        }
    }
}

#[cfg(not(feature = "agentic-actions"))]
impl ConversationBackend for SyscallBackend {
    fn tokenize(&mut self, input: &str, out: &mut Vec<u32>) -> Result<(), ConversationError> {
        self.last_prompt = PromptKind::classify(input);

        #[cfg(feature = "arthur-real-vocab")]
        if let Some(ref real_vocab) = self.real_vocab {
            // Real-vocab path: frame as a proper Qwen2.5 chat turn using ATOMIC
            // control-token IDs (<|im_start|>/<|im_end|>) interleaved with
            // BPE-encoded text segments. No BOS — the chat template starts at
            // <|im_start|>. A well-formed assistant turn is what makes the model
            // answer instead of emitting immediate EOS (OT-37 suspect #1).
            out.clear();
            if self
                .template
                .frame_turn_ids(input, |s| real_vocab.tokenizer.encode(s).ok(), out)
                .is_some()
            {
                #[cfg(feature = "arthur-real-brain-boot")]
                stage_b_log_token_count(b"prompt_tokenize n=", out.len());
                return Ok(());
            }
            // A segment failed to encode — fall through to the fixture path.
            out.clear();
        }

        // Fixture path (default or fallback).
        let tokenizer = BpeTokenizer::from_slices(BPE_VOCAB, BPE_MERGES)
            .map_err(|_| ConversationError::Tokenize)?;
        match tokenizer.encode(input) {
            Ok(tokens) => *out = tokens,
            Err(_) if cfg!(feature = "arthur-act") => {
                out.clear();
                out.push(self.last_prompt.token());
            }
            Err(_) => return Err(ConversationError::Tokenize),
        }
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_token_count(b"prompt_tokenize n=", out.len());
        Ok(())
    }

    fn generate(&mut self, context: &[u32], out: &mut Vec<u32>) -> Result<(), ConversationError> {
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward generate enter");
        out.clear();
        // OT-37: a real autoregressive LM (the inventory brain) generates in the
        // kernel via sys_generate (forward -> greedy argmax -> append -> stop at
        // EOS/max), so Arthur emits real tokens instead of the old stub's
        // hardcoded EOS. The hermetic CI fixture is an embedding model, NOT an
        // autoregressive LM, so it stays on the single-forward smoke path below
        // (which now also emits a non-EOS token, not the old hardcoded EOS).
        // Route by model shape, not merely by the presence of a real vocab — the
        // fixture carries a real vocab too.
        #[cfg(feature = "arthur-real-vocab")]
        if self.real_vocab.is_some() && self.model_is_inventory()? {
            return self.generate_real_brain(context, out);
        }
        let model = self.model()?;
        let mut data = [0u8; TOKEN_CONTEXT * core::mem::size_of::<i32>()];
        let start = context.len().saturating_sub(TOKEN_CONTEXT);
        for idx in 0..TOKEN_CONTEXT {
            let token = context
                .get(start + idx)
                .copied()
                .map(fixture_safe_token)
                .unwrap_or(TOKEN_PAD);
            let token = i32::try_from(token).map_err(|_| ConversationError::Generate)?;
            let off = idx * core::mem::size_of::<i32>();
            data[off..off + 4].copy_from_slice(&token.to_le_bytes());
        }

        let mut dims = [0u64; 8];
        dims[0] = TOKEN_CONTEXT as u64;
        let desc = TensorCreateDesc::new(1, TENSOR_DTYPE_I32, false, dims, &data);
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward tensor_create start");
        let input = sys_tensor_create(&desc).map_err(|_| ConversationError::Generate)?;
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward tensor_create done");
        let mut outputs = [TensorHandle(0)];
        #[cfg(feature = "arthur-real-brain-boot")]
        let forward_start = stage_b_ticks();
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward infer_run start");
        #[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
        v1_console_liveness_begin();
        let result = sys_infer_run(model, &[input], &mut outputs, 0);
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward infer_run returned");
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward input_drop start");
        let _ = sys_tensor_drop(input);
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward input_drop done");
        let count = match result {
            Ok(count) => {
                #[cfg(feature = "arthur-real-brain-boot")]
                {
                    stage_b_log_bytes(b"forward infer_run ok count=");
                    stage_b_log_u64(count as u64);
                    stage_b_log_bytes(b" output0=");
                    stage_b_log_u64(u64::from(outputs[0].0));
                    stage_b_log_newline();
                }
                count
            }
            Err(errno) => {
                #[cfg(not(feature = "arthur-real-brain-boot"))]
                let _ = errno;
                #[cfg(feature = "arthur-real-brain-boot")]
                {
                    stage_b_log_bytes(b"forward infer_run err=");
                    stage_b_log_i64(errno);
                    stage_b_log_newline();
                }
                return Err(ConversationError::Generate);
            }
        };
        if count != 1 {
            return Err(ConversationError::Generate);
        }
        let mut query = TensorQueryDesc::empty();
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward tensor_query start");
        if sys_tensor_query(outputs[0], &mut query) != 0 || !valid_output_query(&query) {
            let _ = sys_tensor_drop(outputs[0]);
            return Err(ConversationError::Generate);
        }
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward tensor_query done");
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward output_drop start");
        let _ = sys_tensor_drop(outputs[0]);
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_line(b"forward output_drop done");
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_bytes(b"forward token=0 ms=");
            stage_b_log_u64(stage_b_elapsed_ms(forward_start));
            stage_b_log_bytes(b" tensor_bytes=");
            stage_b_log_u64(query.byte_len);
            stage_b_log_newline();
        }

        // Fixture path: the real autoregressive brain returned early via
        // `generate_real_brain` above. This embedding fixture has no LM sampler,
        // so it emits its deterministic non-EOS reply token. We log the chosen id
        // so CI can assert the boot path picks a non-EOS token (ISC-4) — the bug
        // this build fixes was the fixture hardcoding the EOS token here.
        let reply_token = self.last_prompt.reply_token();
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_ids(b"fixture_reply_id=", &[reply_token]);
        out.push(reply_token);
        #[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
        v1_console_liveness_done(out.len());
        Ok(())
    }

    fn decode(&mut self, tokens: &[u32], out: &mut Vec<u8>) -> Result<(), ConversationError> {
        #[cfg(feature = "arthur-real-brain-boot")]
        let decode_start = stage_b_ticks();
        #[cfg(feature = "arthur-real-vocab")]
        if let Some(ref real_vocab) = self.real_vocab {
            // The reply ends at the stop token; its `<|im_end|>` text is not
            // part of the answer.
            match real_vocab.tokenizer.decode(real_vocab.reply_tokens(tokens)) {
                Ok(text) => {
                    out.clear();
                    out.extend_from_slice(text.as_bytes());
                    // Never mistaken for the whole answer.
                    if real_vocab.reply_was_cut(tokens, REPLY_MAX_NEW_TOKENS) {
                        out.extend_from_slice(vocab_sidecar::REPLY_CUT_MARKER.as_bytes());
                    }
                    #[cfg(feature = "arthur-real-brain-boot")]
                    stage_b_log_decode_done(tokens.len(), stage_b_elapsed_ms(decode_start));
                    return Ok(());
                }
                Err(_) => {
                    // Decode failed — fall through to fixture.
                }
            }
        }

        // Fixture path (default or fallback).
        let tokenizer = BpeTokenizer::from_slices(BPE_VOCAB, BPE_MERGES)
            .map_err(|_| ConversationError::Decode)?;
        let text = tokenizer
            .decode(tokens)
            .map_err(|_| ConversationError::Decode)?;
        out.clear();
        out.extend_from_slice(text.as_bytes());
        #[cfg(feature = "arthur-real-brain-boot")]
        stage_b_log_decode_done(tokens.len(), stage_b_elapsed_ms(decode_start));
        Ok(())
    }
}

#[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
fn v1_console_ready() {
    console_write_all(V1_CONSOLE_GREETING);
}

#[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
fn v1_console_liveness_begin() {
    console_write_all(V1_CONSOLE_LIVENESS_BEGIN);
}

#[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
fn v1_console_liveness_done(tokens: usize) {
    console_write_all(V1_CONSOLE_LIVENESS_DONE);
    console_write_usize(tokens);
    console_write_all(CONSOLE_NEWLINE);
}

#[cfg(all(feature = "console-interactive", feature = "v1-console-surface"))]
fn console_write_usize(mut value: usize) {
    let mut buf = [0u8; 20];
    let mut idx = buf.len();
    if value == 0 {
        console_write_all(b"0");
        return;
    }
    while value != 0 {
        idx -= 1;
        buf[idx] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    console_write_all(&buf[idx..]);
}

#[cfg(feature = "agentic-actions")]
impl AgenticActionDecoder for SyscallBackend {
    fn decode_agentic_action(
        &mut self,
        state: &mut ConversationState,
        user_text: &str,
        available_actions: &[AgenticActionKind],
        out: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        out.clear();
        let _ = state.context().len();
        state.push_tokens(&[]);
        #[cfg(all(feature = "agentic-actions", feature = "arthur-real-vocab"))]
        if self.decode_agentic_action_with_model(user_text, available_actions, out)? {
            return Ok(());
        }
        let action = fallback_agentic_action(user_text, available_actions);
        let mut ast = String::new();
        render_agentic_ast(action, &mut ast).map_err(|_| ConversationError::Decode)?;
        #[cfg(feature = "arthur-real-brain-boot")]
        {
            stage_b_log_bytes(b"agentic_model_ast=");
            stage_b_log_bytes(ast.as_bytes());
            stage_b_log_newline();
            stage_b_log_agentic_ast_kind(ast.as_bytes());
        }
        out.extend_from_slice(ast.as_bytes());
        Ok(())
    }
}

#[cfg(not(feature = "agentic-actions"))]
#[derive(Copy, Clone, Eq, PartialEq)]
enum PromptKind {
    Conversation,
    ReadFile,
    ReadMissingFile,
    WriteFile,
}

#[cfg(not(feature = "agentic-actions"))]
impl PromptKind {
    fn classify(input: &str) -> Self {
        if input.contains("/scratch") {
            Self::WriteFile
        } else if input.contains("/missing.txt") {
            Self::ReadMissingFile
        } else if input.contains("/hello.txt") {
            Self::ReadFile
        } else {
            Self::Conversation
        }
    }

    const fn token(self) -> u32 {
        match self {
            Self::Conversation => TOKEN_HELLO,
            Self::ReadFile | Self::ReadMissingFile | Self::WriteFile => TOKEN_HELLO,
        }
    }

    const fn reply_token(self) -> u32 {
        match self {
            Self::Conversation => TOKEN_REPLY,
            Self::ReadFile => TOKEN_READ_TOOL,
            Self::ReadMissingFile => TOKEN_READ_MISSING_TOOL,
            Self::WriteFile => TOKEN_WRITE_TOOL,
        }
    }
}

#[cfg(not(feature = "agentic-actions"))]
fn valid_output_query(query: &TensorQueryDesc) -> bool {
    query.device == TENSOR_DEVICE_CPU && query.dtype == TENSOR_DTYPE_F32 && query.byte_len > 0
}

#[cfg(not(feature = "agentic-actions"))]
fn fixture_safe_token(token: u32) -> u32 {
    if token <= TOKEN_REPLY {
        token
    } else {
        TOKEN_HELLO
    }
}

#[cfg(any(not(feature = "agentic-actions"), feature = "arthur-real-vocab"))]
struct ModelEntry {
    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    name: &'static str,
    path: &'static str,
    #[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
    aliases: &'static [&'static str],
}

#[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
impl ModelEntry {
    fn matches(&self, name: &str) -> bool {
        self.name == name || self.aliases.contains(&name)
    }
}

#[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
enum ModelCommand<'a> {
    List,
    Switch(&'a str),
}

#[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
fn parse_model_command(input: &[u8]) -> Option<ModelCommand<'_>> {
    let text = core::str::from_utf8(input).ok()?.trim();
    let rest = text.strip_prefix("/model")?;
    if !rest.is_empty() && !rest.as_bytes()[0].is_ascii_whitespace() {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() || rest == "list" {
        Some(ModelCommand::List)
    } else {
        Some(ModelCommand::Switch(rest))
    }
}

#[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
fn find_model_index(name: &str) -> Option<usize> {
    MODEL_REGISTRY
        .iter()
        .position(|entry| entry.matches(name.trim()))
}

#[cfg(all(feature = "arthur-real-infer", not(feature = "agentic-actions")))]
fn push_reply_bytes(reply: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ConversationError> {
    if reply.len().saturating_add(bytes.len()) > conversation_logic::MAX_REPLY_BYTES {
        return Err(ConversationError::ReplyTooLarge);
    }
    reply.extend_from_slice(bytes);
    Ok(())
}

#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
const RUNTIME_TOOLS: &[ToolKind] = &[ToolKind::ReadFile];
#[cfg(all(
    feature = "agentic-actions",
    not(any(feature = "agentic-actions-a2", feature = "agentic-actions-a3"))
))]
const AGENTIC_RUNTIME_ACTIONS: &[AgenticActionKind] = &[
    AgenticActionKind::ReadFile,
    AgenticActionKind::ListDir,
    AgenticActionKind::ChangeDir,
    AgenticActionKind::Status,
];
#[cfg(all(feature = "agentic-actions-a2", not(feature = "agentic-actions-a3")))]
const AGENTIC_RUNTIME_ACTIONS: &[AgenticActionKind] = &[
    AgenticActionKind::ReadFile,
    AgenticActionKind::WriteFile,
    AgenticActionKind::ListDir,
    AgenticActionKind::ChangeDir,
    AgenticActionKind::Status,
];
#[cfg(feature = "agentic-actions-a3")]
const AGENTIC_RUNTIME_ACTIONS: &[AgenticActionKind] = &[
    AgenticActionKind::ReadFile,
    AgenticActionKind::WriteFile,
    AgenticActionKind::ListDir,
    AgenticActionKind::ChangeDir,
    AgenticActionKind::Status,
    AgenticActionKind::TrashFile,
    AgenticActionKind::MoveFile,
    AgenticActionKind::KillProcess,
    AgenticActionKind::SpawnProcess,
    AgenticActionKind::ExternalSend,
    AgenticActionKind::LoadModel,
];
#[cfg(all(feature = "arthur-act", not(feature = "arthur-grants")))]
const HELLO_PATH: &str = "/hello.txt";
#[cfg(all(feature = "arthur-act", not(feature = "arthur-grants")))]
const ROOT_PATH: &str = "/";
#[cfg(all(feature = "agentic-actions", not(feature = "arthur-grants")))]
const SCRATCH_PATH: &str = "/scratch";
#[cfg(all(feature = "arthur-reversibility", not(feature = "arthur-grants")))]
const DANGER_KEY_PATH: &str = "/neofs/danger.key";
#[cfg(feature = "agentic-actions-a3")]
const AGENTIC_TRASH_CONTENT: &str = "NeoPlutarch trashes A3ok";
#[cfg(feature = "agentic-actions-a3")]
const AGENTIC_A3_KILL_HANDLE: u32 = 4242;
#[cfg(feature = "agentic-actions-a3")]
const AGENTIC_A3_SPAWN_NAME: &str = "worker";
#[cfg(feature = "agentic-actions-a3")]
const AGENTIC_A3_EXTERNAL_HOST: &str = "api.example.com";
#[cfg(feature = "agentic-actions-a3")]
const AGENTIC_A3_EXTERNAL_PORT: u16 = 443;
#[cfg(feature = "agentic-actions-a3")]
const AGENTIC_A3_MODEL_PATH: &str = "/models/a3.neom";
#[cfg(feature = "agentic-actions")]
const AGENTIC_UNDO_MAX_BYTES: usize = 128;

#[cfg(feature = "arthur-act")]
#[cfg(not(feature = "agentic-actions"))]
struct RuntimeActions;

#[cfg(feature = "agentic-actions")]
struct RuntimeActions {
    undo: Option<AgenticUndo>,
    #[cfg(feature = "arthur-reversibility")]
    undo_nonce: u128,
    #[cfg(feature = "agentic-ipc-frequency")]
    measured_action: Option<AgenticMeasuredAction>,
}

#[cfg(feature = "agentic-ipc-frequency")]
#[derive(Copy, Clone)]
struct AgenticMeasuredAction {
    tier: &'static [u8],
    action: &'static [u8],
}

#[cfg(feature = "agentic-actions")]
struct AgenticUndo {
    path: String,
    #[cfg(not(feature = "arthur-reversibility"))]
    data: Vec<u8>,
    #[cfg(feature = "arthur-reversibility")]
    artifact_path: String,
    #[cfg(feature = "arthur-reversibility")]
    key: [u8; memory_crypto::KEY_LEN],
}

#[cfg(feature = "arthur-reversibility")]
enum AgenticUndoPlan {
    Armed(AgenticUndo),
    ConfirmedIrreversible,
}

#[cfg(feature = "arthur-act")]
impl RuntimeActions {
    const fn new() -> Self {
        #[cfg(not(feature = "agentic-actions"))]
        {
            RuntimeActions
        }
        #[cfg(feature = "agentic-actions")]
        {
            #[cfg(feature = "arthur-grants")]
            {
                RuntimeActions {
                    undo: None,
                    #[cfg(feature = "arthur-reversibility")]
                    undo_nonce: 1,
                    #[cfg(feature = "agentic-ipc-frequency")]
                    measured_action: None,
                }
            }
            #[cfg(not(feature = "arthur-grants"))]
            {
                RuntimeActions {
                    undo: None,
                    #[cfg(feature = "arthur-reversibility")]
                    undo_nonce: 1,
                    #[cfg(feature = "agentic-ipc-frequency")]
                    measured_action: None,
                }
            }
        }
    }

    #[cfg(feature = "agentic-ipc-frequency")]
    fn clear_agentic_measurement(&mut self) {
        self.measured_action = None;
    }

    #[cfg(feature = "agentic-ipc-frequency")]
    fn take_agentic_measurement(&mut self) -> Option<AgenticMeasuredAction> {
        self.measured_action.take()
    }

    #[cfg(feature = "agentic-ipc-frequency")]
    fn record_agentic_measurement(&mut self, action: AgenticAction<'_>) {
        self.measured_action = Some(AgenticMeasuredAction::from_action(action));
    }
}

#[cfg(feature = "agentic-actions")]
impl AgenticUndo {
    #[cfg(not(feature = "arthur-reversibility"))]
    fn new(path: &str, data: &[u8]) -> Self {
        Self {
            path: String::from(path),
            data: Vec::from(data),
        }
    }

    #[cfg(feature = "arthur-reversibility")]
    fn new(path: &str, artifact_path: String, key: [u8; memory_crypto::KEY_LEN]) -> Self {
        Self {
            path: String::from(path),
            artifact_path,
            key,
        }
    }
}

#[cfg(feature = "agentic-ipc-frequency")]
impl AgenticMeasuredAction {
    fn from_action(action: AgenticAction<'_>) -> Self {
        match action {
            AgenticAction::ReadFile { .. } => Self {
                tier: b"A1",
                action: b"read_file",
            },
            AgenticAction::ListDir { .. } => Self {
                tier: b"A1",
                action: b"list_dir",
            },
            AgenticAction::ChangeDir { .. } => Self {
                tier: b"A1",
                action: b"cd",
            },
            AgenticAction::Status => Self {
                tier: b"A1",
                action: b"status",
            },
            AgenticAction::WriteFile { .. } => Self {
                tier: b"A2",
                action: b"write_file",
            },
            AgenticAction::Undo => Self {
                tier: b"A2",
                action: b"undo",
            },
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::TrashFile { .. } => Self {
                tier: b"A3",
                action: b"trash_file",
            },
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::MoveFile { .. } => Self {
                tier: b"A3",
                action: b"move_file",
            },
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::KillProcess { .. } => Self {
                tier: b"A3",
                action: b"kill_process",
            },
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::SpawnProcess { .. } => Self {
                tier: b"A3",
                action: b"spawn_process",
            },
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::ExternalSend { .. } => Self {
                tier: b"A3",
                action: b"external_send",
            },
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::LoadModel { .. } => Self {
                tier: b"A3",
                action: b"load_model",
            },
            AgenticAction::Clarify { .. }
            | AgenticAction::Restricted { .. }
            | AgenticAction::Confirm
            | AgenticAction::NoOp => Self {
                tier: b"NA",
                action: b"no_execute",
            },
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::DryRun { .. }
            | AgenticAction::ConfirmTyped { .. }
            | AgenticAction::Cancel
            | AgenticAction::Refuse { .. }
            | AgenticAction::Done { .. } => Self {
                tier: b"NA",
                action: b"no_execute",
            },
        }
    }
}

#[cfg(feature = "agentic-ipc-frequency")]
#[derive(Copy, Clone)]
struct AgenticFrequencyTurn {
    kernel: Option<AgenticKernelCounters>,
    action: libneo::action::ActionObservabilityV1,
}

#[cfg(feature = "agentic-ipc-frequency")]
#[derive(Copy, Clone)]
struct AgenticKernelCounters {
    total: u64,
    ipc: u64,
    file: u64,
    ai: u64,
    net: u64,
    other: u64,
}

#[cfg(feature = "agentic-ipc-frequency")]
impl AgenticFrequencyTurn {
    fn begin() -> Self {
        Self {
            kernel: AgenticKernelCounters::snapshot(),
            action: libneo::action::action_observability_snapshot(),
        }
    }
}

#[cfg(feature = "agentic-ipc-frequency")]
impl AgenticKernelCounters {
    fn snapshot() -> Option<Self> {
        let stats = libneo::sys_observe().ok()?;
        Some(Self {
            total: stats.syscall_total,
            ipc: stats.syscall_ipc,
            file: stats.syscall_file,
            ai: stats.syscall_ai,
            net: stats.syscall_net,
            other: stats.syscall_other,
        })
    }

    fn delta_since(self, before: Self) -> Self {
        Self {
            total: self.total.saturating_sub(before.total),
            ipc: self.ipc.saturating_sub(before.ipc),
            file: self.file.saturating_sub(before.file),
            ai: self.ai.saturating_sub(before.ai),
            net: self.net.saturating_sub(before.net),
            other: self.other.saturating_sub(before.other),
        }
    }

    fn without_measurement_observe(mut self) -> Self {
        self.total = self.total.saturating_sub(1);
        self.other = self.other.saturating_sub(1);
        self
    }
}

#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
impl ToolActionExecutor for RuntimeActions {
    fn available_tools(&self) -> &[ToolKind] {
        RUNTIME_TOOLS
    }

    fn execute_tool_call(
        &mut self,
        call: ToolCall<'_>,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        execute_runtime_tool(call, reply)
    }
}

#[cfg(feature = "agentic-actions")]
impl AgenticActionExecutor for RuntimeActions {
    fn available_agentic_actions(&self) -> &[AgenticActionKind] {
        AGENTIC_RUNTIME_ACTIONS
    }

    fn execute_agentic_action(
        &mut self,
        action: AgenticAction<'_>,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        let result = match action {
            AgenticAction::ReadFile { path } => execute_agentic_file_action(true, path, reply),
            AgenticAction::ListDir { path } => execute_agentic_file_action(false, path, reply),
            AgenticAction::ChangeDir { path } => execute_agentic_change_dir_action(path, reply),
            AgenticAction::Status => execute_agentic_status_action(reply),
            AgenticAction::WriteFile { path, content } => {
                self.execute_agentic_write_action(path, content, false, reply)
            }
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::TrashFile { path } => self.execute_agentic_trash_action(path, reply),
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::MoveFile { path, destination } => {
                self.execute_agentic_move_action(path, destination, reply)
            }
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::KillProcess { handle } => {
                self.execute_agentic_kill_action(handle, reply)
            }
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::SpawnProcess {
                name,
                inherit_handle,
            } => self.execute_agentic_spawn_action(name, inherit_handle, reply),
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::ExternalSend { host, port, bytes } => {
                self.execute_agentic_external_send_action(host, port, bytes, reply)
            }
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::LoadModel { path } => {
                self.execute_agentic_load_model_action(path, reply)
            }
            AgenticAction::Undo => self.execute_agentic_undo(reply),
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::DryRun { .. }
            | AgenticAction::ConfirmTyped { .. }
            | AgenticAction::Cancel
            | AgenticAction::Refuse { .. }
            | AgenticAction::Done { .. } => Err(ConversationError::Action),
            AgenticAction::Clarify { .. }
            | AgenticAction::Restricted { .. }
            | AgenticAction::Confirm
            | AgenticAction::NoOp => Err(ConversationError::Action),
        };
        #[cfg(feature = "agentic-ipc-frequency")]
        if result.is_ok() {
            self.record_agentic_measurement(action);
        }
        result
    }

    fn execute_confirmed_agentic_action(
        &mut self,
        action: AgenticAction<'_>,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        reply.clear();
        let result = match action {
            AgenticAction::WriteFile { path, content } => {
                self.execute_agentic_write_action(path, content, true, reply)
            }
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::TrashFile { path } => {
                self.execute_agentic_trash_action_confirmed(path, reply)
            }
            #[cfg(feature = "agentic-actions-a3")]
            AgenticAction::MoveFile { path, destination } => {
                self.execute_agentic_move_action_confirmed(path, destination, reply)
            }
            other => self.execute_agentic_action(other, reply),
        };
        #[cfg(feature = "agentic-ipc-frequency")]
        if result.is_ok() {
            self.record_agentic_measurement(action);
        }
        result
    }
}

#[cfg(feature = "arthur-grants")]
#[derive(Copy, Clone)]
enum BackedGrant<'a> {
    File {
        path: &'a str,
        handle: libneo::FileHandle,
    },
    NetDial {
        host: &'a str,
        port: u16,
        cap: libneo::NetDialCap,
    },
    Empty,
}

#[cfg(feature = "arthur-grants")]
struct ArthurGrantSyscalls<'a> {
    grants: &'a [BackedGrant<'a>],
}

#[cfg(feature = "arthur-grants")]
impl<'a> ArthurGrantSyscalls<'a> {
    fn find_file(&self, path: &str) -> Option<libneo::FileHandle> {
        self.grants.iter().find_map(|grant| match *grant {
            BackedGrant::File {
                path: granted_path,
                handle,
            } if granted_path == path => Some(handle),
            _ => None,
        })
    }

    fn find_net_dial(&self, host: &str, port: u16) -> Option<libneo::NetDialCap> {
        self.grants.iter().find_map(|grant| match *grant {
            BackedGrant::NetDial {
                host: granted_host,
                port: granted_port,
                cap,
            } if granted_host == host && granted_port == port => Some(cap),
            _ => None,
        })
    }

    fn owns(&self, handle: libneo::FileHandle) -> bool {
        self.grants.iter().any(
            |grant| matches!(*grant, BackedGrant::File { handle: owned, .. } if owned == handle),
        )
    }
}

#[cfg(feature = "arthur-grants")]
impl ActionSyscalls for ArthurGrantSyscalls<'_> {
    fn open(&mut self, path: &str, _rights: FileRights) -> Result<libneo::FileHandle, i64> {
        self.find_file(path).ok_or(libneo::EPERM)
    }

    fn read(&mut self, handle: libneo::FileHandle, out: &mut [u8]) -> i64 {
        libneo::sys_read(handle, out)
    }

    fn write(&mut self, handle: libneo::FileHandle, data: &[u8]) -> i64 {
        libneo::sys_write(handle.0, data.as_ptr(), data.len())
    }

    fn readdir(&mut self, handle: libneo::FileHandle, out: &mut [u8], cursor: &mut u64) -> i64 {
        libneo::sys_readdir(handle, out, cursor)
    }

    fn close(&mut self, handle: libneo::FileHandle) -> i64 {
        if self.owns(handle) {
            0
        } else {
            libneo::sys_close(handle)
        }
    }

    fn spawn(
        &mut self,
        name: &str,
        handles: &[Handle],
    ) -> Result<(u32, libneo::ProcessHandle), i64> {
        libneo::sys_spawn(name, handles)
    }

    fn kill(&mut self, handle: libneo::ProcessHandle) -> i64 {
        libneo::sys_kill(handle)
    }

    fn inspect(&mut self, handle: libneo::ProcessHandle) -> Result<libneo::ProcessSnapshotV1, i64> {
        libneo::sys_inspect(handle)
    }

    fn wait(&mut self, handle: libneo::ProcessHandle, flags: u64) -> Result<i32, i64> {
        libneo::sys_wait(handle, flags)
    }

    fn net_dial(
        &mut self,
        cap: libneo::NetDialCap,
        host: &str,
        port: u16,
    ) -> Result<TcpSocketHandle, i64> {
        let held = self.find_net_dial(host, port).ok_or(libneo::EACCES)?;
        if held != cap {
            return Err(libneo::EACCES);
        }
        libneo::sys_net_dial(cap, host, port)
    }

    fn tcp_send(&mut self, handle: TcpSocketHandle, data: &[u8]) -> i64 {
        libneo::sys_tcp_send(handle, data)
    }

    fn tcp_recv(&mut self, handle: TcpSocketHandle, out: &mut [u8]) -> i64 {
        libneo::sys_tcp_recv(handle, out)
    }

    fn infer(
        &mut self,
        model: ModelHandle,
        inputs: &[TensorHandle],
        outputs: &mut [TensorHandle],
        flags: u32,
    ) -> Result<usize, i64> {
        libneo::sys_infer_run(model, inputs, outputs, flags)
    }

    fn model_load(&mut self, path: &str, flags: u32) -> Result<ModelHandle, i64> {
        libneo::sys_model_load(path, flags)
    }
}

#[cfg(feature = "agentic-actions-a3")]
struct AgenticA3Syscalls;

#[cfg(feature = "agentic-actions-a3")]
impl ActionSyscalls for AgenticA3Syscalls {
    fn open(&mut self, _path: &str, _rights: FileRights) -> Result<libneo::FileHandle, i64> {
        Err(libneo::EPERM)
    }

    fn read(&mut self, _handle: libneo::FileHandle, _out: &mut [u8]) -> i64 {
        libneo::EPERM
    }

    fn write(&mut self, _handle: libneo::FileHandle, _data: &[u8]) -> i64 {
        libneo::EPERM
    }

    fn readdir(&mut self, _handle: libneo::FileHandle, _out: &mut [u8], _cursor: &mut u64) -> i64 {
        libneo::EPERM
    }

    fn close(&mut self, _handle: libneo::FileHandle) -> i64 {
        0
    }

    fn spawn(&mut self, _name: &str, _handles: &[Handle]) -> Result<(u32, ProcessHandle), i64> {
        Ok((7, ProcessHandle(7007)))
    }

    fn kill(&mut self, _handle: ProcessHandle) -> i64 {
        0
    }

    fn inspect(&mut self, _handle: ProcessHandle) -> Result<libneo::ProcessSnapshotV1, i64> {
        Err(libneo::EPERM)
    }

    fn wait(&mut self, _handle: ProcessHandle, _flags: u64) -> Result<i32, i64> {
        Err(libneo::EPERM)
    }

    fn net_dial(
        &mut self,
        _cap: libneo::NetDialCap,
        _host: &str,
        _port: u16,
    ) -> Result<TcpSocketHandle, i64> {
        Ok(TcpSocketHandle(8008))
    }

    fn tcp_send(&mut self, _handle: TcpSocketHandle, data: &[u8]) -> i64 {
        data.len() as i64
    }

    fn tcp_recv(&mut self, _handle: TcpSocketHandle, _out: &mut [u8]) -> i64 {
        libneo::EPERM
    }

    fn infer(
        &mut self,
        _model: ModelHandle,
        _inputs: &[TensorHandle],
        _outputs: &mut [TensorHandle],
        _flags: u32,
    ) -> Result<usize, i64> {
        Err(libneo::EPERM)
    }

    fn model_load(&mut self, _path: &str, _flags: u32) -> Result<ModelHandle, i64> {
        Ok(ModelHandle(9009))
    }
}

#[cfg(feature = "arthur-grants")]
fn read_grants_store(buf: &mut [u8]) -> Option<usize> {
    let file = libneo::sys_open(
        ARTHUR_GRANTS_STORE_PATH,
        OPEN_ROOT_TEMPORARY,
        FileRights::READ,
    )
    .ok()?;
    let n = libneo::sys_read(file, buf);
    let _ = libneo::sys_close(file);
    if n < 0 { None } else { Some(n as usize) }
}

#[cfg(feature = "arthur-grants")]
fn receive_startup_grants() -> Result<usize, i64> {
    let mut buf = [0u8; GRANTS_BUF_LEN];
    let mut entries = [libneo::grant_store::GrantEntry::EMPTY; libneo::grant_store::MAX_GRANTS];
    let store = read_grants_store(&mut buf);
    let config = match store {
        Some(len) => libneo::grant_store::GrantConfig::Enabled(Some(&buf[..len])),
        None => libneo::grant_store::GrantConfig::Enabled(None),
    };
    let entry_count = libneo::grant_store::build_grant_entries(config, &mut entries);
    let mut count = 0usize;

    for entry in &entries[..entry_count] {
        if count >= MAX_BACKED_GRANTS {
            break;
        }
        match entry.scope {
            libneo::grant_store::ScopeKind::FileRead
            | libneo::grant_store::ScopeKind::FileWrite => {
                let rights = runtime_file_rights(entry);
                if rights.bits() == 0 {
                    continue;
                }
                let handle = sys_recv_grant(CONVERSATION, cap_rights_for_file_rights(rights))?;
                let expected = GRANT_HANDLE_START + count as u32;
                if handle.0 != expected {
                    return Err(EINVAL);
                }
                count += 1;
            }
            #[cfg(feature = "net-dial-cap")]
            libneo::grant_store::ScopeKind::Network => {
                let handle = sys_recv_grant(
                    CONVERSATION,
                    CapRights::new(
                        meta_rights::USE | meta_rights::REVOKE,
                        libneo::net_dial_rights::DIAL,
                    ),
                )?;
                let expected = GRANT_HANDLE_START + count as u32;
                if handle.0 != expected {
                    return Err(EINVAL);
                }
                count += 1;
            }
            #[cfg(not(feature = "net-dial-cap"))]
            libneo::grant_store::ScopeKind::Network => {}
            libneo::grant_store::ScopeKind::Process
            | libneo::grant_store::ScopeKind::Model
            | libneo::grant_store::ScopeKind::Memory => {}
        }
    }

    Ok(count)
}

#[cfg(feature = "arthur-grants")]
fn build_runtime_grants<'a>(
    entries: &[libneo::grant_store::GrantEntry<'a>],
    grants: &mut [Grant<'a>],
    backed: &mut [BackedGrant<'a>],
) -> usize {
    let mut count = 0usize;
    for entry in entries {
        if count >= grants.len() || count >= backed.len() {
            break;
        }
        match entry.scope {
            libneo::grant_store::ScopeKind::FileRead
            | libneo::grant_store::ScopeKind::FileWrite => {
                let rights = runtime_file_rights(entry);
                if rights.bits() == 0 {
                    continue;
                }
                let handle = libneo::FileHandle(GRANT_HANDLE_START + count as u32);
                grants[count] = Grant::File {
                    path: entry.resource,
                    rights,
                };
                backed[count] = BackedGrant::File {
                    path: entry.resource,
                    handle,
                };
                count += 1;
            }
            #[cfg(feature = "net-dial-cap")]
            libneo::grant_store::ScopeKind::Network => {
                let (host, port) = match libneo::grant_store::entry_to_grant(entry) {
                    Some(Grant::NetDial { host, port, .. }) => (host, port),
                    _ => continue,
                };
                let cap = libneo::NetDialCap(Handle(GRANT_HANDLE_START + count as u32));
                grants[count] = Grant::NetDial { host, port, cap };
                backed[count] = BackedGrant::NetDial { host, port, cap };
                count += 1;
            }
            #[cfg(not(feature = "net-dial-cap"))]
            libneo::grant_store::ScopeKind::Network => {}
            libneo::grant_store::ScopeKind::Process
            | libneo::grant_store::ScopeKind::Model
            | libneo::grant_store::ScopeKind::Memory => {}
        }
    }
    count
}

#[cfg(feature = "arthur-grants")]
fn runtime_file_rights(entry: &libneo::grant_store::GrantEntry<'_>) -> FileRights {
    normalize_file_rights_for_resource(entry.resource, effective_file_rights(entry))
}

#[cfg(feature = "arthur-grants")]
fn effective_file_rights(entry: &libneo::grant_store::GrantEntry<'_>) -> FileRights {
    match entry.scope {
        libneo::grant_store::ScopeKind::FileRead => {
            FileRights(entry.rights.bits() & !FileRights::WRITE.bits())
        }
        libneo::grant_store::ScopeKind::FileWrite => entry.rights,
        _ => FileRights(0),
    }
}

#[cfg(feature = "arthur-grants")]
fn normalize_file_rights_for_resource(resource: &str, rights: FileRights) -> FileRights {
    if resource.ends_with('/') {
        let dir_rights = FileRights::LOOKUP.union(FileRights::READDIR);
        return FileRights(rights.bits() & dir_rights.bits());
    }
    rights
}

#[cfg(feature = "arthur-grants")]
fn cap_rights_for_file_rights(rights: FileRights) -> CapRights {
    CapRights::new(meta_rights::USE, (rights.bits() & 0x7f) as u8)
}

#[cfg(feature = "arthur-grants")]
fn load_runtime_grants<'a>(
    buf: &'a mut [u8; GRANTS_BUF_LEN],
    entries: &mut [libneo::grant_store::GrantEntry<'a>; libneo::grant_store::MAX_GRANTS],
    grants: &mut [Grant<'a>; MAX_BACKED_GRANTS],
    backed: &mut [BackedGrant<'a>; MAX_BACKED_GRANTS],
) -> usize {
    let store = read_grants_store(buf);
    let config = match store {
        Some(len) => libneo::grant_store::GrantConfig::Enabled(Some(&buf[..len])),
        None => libneo::grant_store::GrantConfig::Enabled(None),
    };
    let entry_count = libneo::grant_store::build_grant_entries(config, entries);
    build_runtime_grants(&entries[..entry_count], grants, backed)
}

#[cfg(feature = "arthur-grants")]
fn respond_grant_probe(input: &[u8], reply: &mut Vec<u8>) -> Option<Result<(), ConversationError>> {
    let text = core::str::from_utf8(input).ok()?.trim();
    let path = text.strip_prefix("/grant-probe write ")?;
    reply.clear();

    let mut buf = [0u8; GRANTS_BUF_LEN];
    let mut entries = [libneo::grant_store::GrantEntry::EMPTY; libneo::grant_store::MAX_GRANTS];
    let mut grants = [PLACEHOLDER_GRANT; MAX_BACKED_GRANTS];
    let mut backed = [PLACEHOLDER_BACKED_GRANT; MAX_BACKED_GRANTS];
    let count = load_runtime_grants(&mut buf, &mut entries, &mut grants, &mut backed);

    let Some(file) = backed[..count].iter().find_map(|grant| match *grant {
        BackedGrant::File {
            path: granted_path,
            handle,
        } if granted_path == path => Some(handle),
        _ => None,
    }) else {
        return Some(push_bytes(reply, b"arthur: grant_probe: write errno=-1"));
    };
    let result = libneo::sys_write(file.0, b"x".as_ptr(), 1);
    let status = if result < 0 {
        push_bytes(reply, b"arthur: grant_probe: write errno=")
            .and_then(|()| push_i64(reply, result))
    } else {
        push_bytes(reply, b"arthur: grant_probe: write bytes=")
            .and_then(|()| push_i64(reply, result))
    };
    Some(status)
}

#[cfg(feature = "agentic-actions")]
fn execute_agentic_file_action(
    read_file: bool,
    path: &str,
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    #[cfg(feature = "arthur-grants")]
    let mut grants_buf = [0u8; GRANTS_BUF_LEN];
    #[cfg(feature = "arthur-grants")]
    let mut grant_entries =
        [libneo::grant_store::GrantEntry::EMPTY; libneo::grant_store::MAX_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let mut file_grants = [PLACEHOLDER_GRANT; MAX_BACKED_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let mut backed_file_grants = [PLACEHOLDER_BACKED_GRANT; MAX_BACKED_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let file_grant_count = load_runtime_grants(
        &mut grants_buf,
        &mut grant_entries,
        &mut file_grants,
        &mut backed_file_grants,
    );
    #[cfg(feature = "arthur-grants")]
    let mut file_syscalls = ArthurGrantSyscalls {
        grants: &backed_file_grants[..file_grant_count],
    };
    #[cfg(feature = "arthur-grants")]
    let file_grant_set = &file_grants[..file_grant_count];

    #[cfg(not(feature = "arthur-grants"))]
    let file_grants = [
        Grant::File {
            path: HELLO_PATH,
            rights: FileRights::READ,
        },
        #[cfg(feature = "agentic-actions")]
        Grant::File {
            path: SCRATCH_PATH,
            rights: FileRights::READ.union(FileRights::WRITE),
        },
        Grant::File {
            path: ROOT_PATH,
            rights: FileRights::LOOKUP.union(FileRights::READDIR),
        },
    ];
    #[cfg(not(feature = "arthur-grants"))]
    let mut file_syscalls = libneo::action::LibneoActionSyscalls;
    #[cfg(not(feature = "arthur-grants"))]
    let file_grant_set = &file_grants;

    if read_file {
        let mut out = [0u8; 96];
        match execute_action_with(
            &mut file_syscalls,
            file_grant_set,
            Action::ReadFile {
                path,
                out: &mut out,
            },
        ) {
            Ok(ActionOutcome::ReadFile { bytes }) => {
                push_bytes(reply, b"arthur: read_file: ")?;
                push_bytes(reply, &out[..bytes])
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_action_error(error, reply),
        }
    } else {
        let mut out = [0u8; 96];
        let mut cursor = 0u64;
        match execute_action_with(
            &mut file_syscalls,
            file_grant_set,
            Action::ListDir {
                path,
                out: &mut out,
                cursor: &mut cursor,
            },
        ) {
            Ok(ActionOutcome::ListDir { bytes }) => {
                push_bytes(reply, b"arthur: list_dir: bytes=")?;
                push_usize(reply, bytes)
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_action_error(error, reply),
        }
    }
}

#[cfg(feature = "agentic-actions")]
fn execute_agentic_change_dir_action(
    path: &str,
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    #[cfg(feature = "arthur-grants")]
    let mut grants_buf = [0u8; GRANTS_BUF_LEN];
    #[cfg(feature = "arthur-grants")]
    let mut grant_entries =
        [libneo::grant_store::GrantEntry::EMPTY; libneo::grant_store::MAX_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let mut file_grants = [PLACEHOLDER_GRANT; MAX_BACKED_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let mut backed_file_grants = [PLACEHOLDER_BACKED_GRANT; MAX_BACKED_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let file_grant_count = load_runtime_grants(
        &mut grants_buf,
        &mut grant_entries,
        &mut file_grants,
        &mut backed_file_grants,
    );
    #[cfg(feature = "arthur-grants")]
    let mut file_syscalls = ArthurGrantSyscalls {
        grants: &backed_file_grants[..file_grant_count],
    };
    #[cfg(feature = "arthur-grants")]
    let file_grant_set = &file_grants[..file_grant_count];

    #[cfg(not(feature = "arthur-grants"))]
    let file_grants = [
        Grant::File {
            path: HELLO_PATH,
            rights: FileRights::READ,
        },
        Grant::File {
            path: SCRATCH_PATH,
            rights: FileRights::READ.union(FileRights::WRITE),
        },
        Grant::File {
            path: ROOT_PATH,
            rights: FileRights::LOOKUP.union(FileRights::READDIR),
        },
    ];
    #[cfg(not(feature = "arthur-grants"))]
    let mut file_syscalls = libneo::action::LibneoActionSyscalls;
    #[cfg(not(feature = "arthur-grants"))]
    let file_grant_set = &file_grants;

    let mut out = [0u8; 96];
    let mut cursor = 0u64;
    match execute_action_with(
        &mut file_syscalls,
        file_grant_set,
        Action::ListDir {
            path,
            out: &mut out,
            cursor: &mut cursor,
        },
    ) {
        Ok(ActionOutcome::ListDir { .. }) => {
            push_bytes(reply, b"arthur: cd: ")?;
            push_bytes(reply, path.as_bytes())
        }
        Ok(_) => Err(ConversationError::Action),
        Err(error) => push_action_error(error, reply),
    }
}

#[cfg(feature = "agentic-actions")]
fn execute_agentic_status_action(reply: &mut Vec<u8>) -> Result<(), ConversationError> {
    push_bytes(reply, b"arthur: status: ready")
}

#[cfg(feature = "arthur-reversibility")]
const REVERSIBILITY_KEY_DOMAIN: &[u8; memory_crypto::KEY_LEN] = b"Arthur OT19 undo prewrite key v1";

#[cfg(feature = "arthur-reversibility")]
const REVERSIBILITY_IO_CHUNK: usize = 256;

#[cfg(feature = "arthur-reversibility")]
fn reversibility_artifact_key(path: &str) -> [u8; memory_crypto::KEY_LEN] {
    let hash = reversibility_logic::path_hash(path).to_le_bytes();
    let mut key = [0u8; memory_crypto::KEY_LEN];
    for idx in 0..key.len() {
        key[idx] = REVERSIBILITY_KEY_DOMAIN[idx] ^ hash[idx % hash.len()].wrapping_add(idx as u8);
    }
    key
}

#[cfg(feature = "arthur-reversibility")]
fn reversibility_read_file(path: &str, cap: usize, out: &mut Vec<u8>) -> Result<(), i64> {
    out.clear();
    let file = sys_open(path, libneo::OPEN_ROOT_TEMPORARY, FileRights::READ)?;
    let mut scratch = [0u8; REVERSIBILITY_IO_CHUNK];
    loop {
        if out.len() >= cap {
            let _ = sys_close(file);
            return Err(libneo::EINVAL);
        }
        let remaining = cap - out.len();
        let read_len = core::cmp::min(remaining, scratch.len());
        let n = sys_read(file, &mut scratch[..read_len]);
        if n < 0 {
            let _ = sys_close(file);
            return Err(n);
        }
        if n == 0 {
            let _ = sys_close(file);
            return Ok(());
        }
        let n = n as usize;
        out.extend_from_slice(&scratch[..n]);
    }
}

#[cfg(feature = "arthur-reversibility")]
fn reversibility_write_file(path: &str, bytes: &[u8]) -> Result<(), i64> {
    let rights = FileRights::WRITE;
    let (file, opened_existing) = match sys_open(path, libneo::OPEN_ROOT_TEMPORARY, rights) {
        Ok(file) => (file, true),
        Err(_) => {
            let created = sys_open(path, OPEN_CREATE_REGULAR, rights)?;
            let closed = sys_close(created);
            if closed < 0 {
                return Err(closed);
            }
            // Publish the new NeoFS inode, then write through a lookup handle.
            (sys_open(path, libneo::OPEN_ROOT_TEMPORARY, rights)?, false)
        }
    };
    // Fresh NeoFS artifacts are already empty; truncate only when overwriting.
    if opened_existing {
        let truncate = libneo::sys_ftruncate(file, 0);
        if truncate < 0 {
            let _ = sys_close(file);
            return Err(truncate);
        }
    }
    let written = sys_write(file.0, bytes.as_ptr(), bytes.len());
    if written != bytes.len() as i64 {
        let _ = sys_close(file);
        return Err(written);
    }
    // Same-session undo reads fresh artifacts through NeoFS after close; avoid
    // the fresh-create fsync path, which can wedge while the cache is draining.
    if opened_existing {
        let synced = sys_fsync(file);
        if synced < 0 {
            let _ = sys_close(file);
            return Err(synced);
        }
    }
    let closed = sys_close(file);
    if closed < 0 {
        return Err(closed);
    }
    Ok(())
}

#[cfg(feature = "arthur-reversibility")]
fn reversibility_write_confirm_required(
    path: &str,
    reason: &[u8],
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    push_bytes(reply, b"arthur: confirm required: write_file ")?;
    push_bytes(reply, path.as_bytes())?;
    push_bytes(reply, b" reason=")?;
    push_bytes(reply, reason)
}

#[cfg(all(
    feature = "arthur-reversibility",
    feature = "arthur-reversibility-stakes"
))]
fn reversibility_read_stakes_policy() -> Option<Vec<u8>> {
    let mut policy = Vec::new();
    reversibility_read_file(
        reversibility_logic::STAKES_POLICY_PATH,
        reversibility_logic::UNDO_STORAGE_MAX_BYTES.min(4096),
        &mut policy,
    )
    .ok()?;
    Some(policy)
}

#[cfg(feature = "agentic-actions")]
impl RuntimeActions {
    #[cfg(feature = "arthur-reversibility")]
    fn reversibility_stakes_confirm_required(&self, path: &str) -> bool {
        #[cfg(feature = "arthur-reversibility-stakes")]
        {
            let Some(policy) = reversibility_read_stakes_policy() else {
                return false;
            };
            let Ok(policy) = core::str::from_utf8(&policy) else {
                return false;
            };
            reversibility_logic::stakes_policy_requires_confirm(policy, "write_file", path)
        }
        #[cfg(not(feature = "arthur-reversibility-stakes"))]
        {
            let _ = path;
            false
        }
    }

    #[cfg(feature = "arthur-reversibility")]
    fn prepare_agentic_reversibility(
        &mut self,
        path: &str,
        snapshot: &[u8],
        confirmed: bool,
        reply: &mut Vec<u8>,
    ) -> Result<Option<AgenticUndoPlan>, ConversationError> {
        if !confirmed && self.reversibility_stakes_confirm_required(path) {
            reversibility_write_confirm_required(path, b"stakes-policy", reply)?;
            return Ok(None);
        }
        if !reversibility_logic::reversible_backup_allowed(path) {
            if confirmed {
                return Ok(Some(AgenticUndoPlan::ConfirmedIrreversible));
            }
            reversibility_write_confirm_required(path, b"key-file-not-backed-up", reply)?;
            return Ok(None);
        }

        let artifact_path = reversibility_logic::prewrite_artifact_path(path);
        let key = reversibility_artifact_key(path);
        let ck = CategoryKey::from_raw(key, self.undo_nonce);
        let nonce = self.undo_nonce;
        self.undo_nonce = self.undo_nonce.saturating_add(1);
        let mut encrypted = Vec::new();
        if encrypt_plaintext(&ck, nonce, snapshot, &mut encrypted).is_err()
            || encrypted.len() > reversibility_logic::UNDO_STORAGE_MAX_BYTES
        {
            if confirmed {
                return Ok(Some(AgenticUndoPlan::ConfirmedIrreversible));
            }
            reversibility_write_confirm_required(path, b"undo-encrypt-failed", reply)?;
            return Ok(None);
        }
        if reversibility_write_file(artifact_path.as_str(), encrypted.as_slice()).is_err() {
            if confirmed {
                return Ok(Some(AgenticUndoPlan::ConfirmedIrreversible));
            }
            reversibility_write_confirm_required(path, b"undo-storage-unavailable", reply)?;
            return Ok(None);
        }

        Ok(Some(AgenticUndoPlan::Armed(AgenticUndo::new(
            path,
            artifact_path,
            key,
        ))))
    }

    #[cfg(feature = "arthur-reversibility")]
    fn load_agentic_undo_artifact(
        undo: &AgenticUndo,
        out: &mut Vec<u8>,
    ) -> Result<(), &'static [u8]> {
        let mut encrypted = Vec::new();
        reversibility_read_file(
            undo.artifact_path.as_str(),
            reversibility_logic::UNDO_STORAGE_MAX_BYTES,
            &mut encrypted,
        )
        .map_err(|_| b"artifact-unavailable".as_slice())?;
        let ck = CategoryKey::from_raw(undo.key, 0);
        decrypt_file(encrypted.as_slice(), &ck, out).map_err(|_| b"artifact-corrupt".as_slice())
    }

    fn execute_agentic_write_action(
        &mut self,
        path: &str,
        content: &str,
        confirmed: bool,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        #[cfg(not(feature = "arthur-reversibility"))]
        let _ = confirmed;

        #[cfg(feature = "arthur-grants")]
        {
            let mut grants_buf = [0u8; GRANTS_BUF_LEN];
            let mut grant_entries =
                [libneo::grant_store::GrantEntry::EMPTY; libneo::grant_store::MAX_GRANTS];
            let mut file_grants = [PLACEHOLDER_GRANT; MAX_BACKED_GRANTS];
            let mut backed_file_grants = [PLACEHOLDER_BACKED_GRANT; MAX_BACKED_GRANTS];
            let file_grant_count = load_runtime_grants(
                &mut grants_buf,
                &mut grant_entries,
                &mut file_grants,
                &mut backed_file_grants,
            );
            let mut file_syscalls = ArthurGrantSyscalls {
                grants: &backed_file_grants[..file_grant_count],
            };
            let file_grant_set = &file_grants[..file_grant_count];

            if !agentic_grants_include_file_right(file_grant_set, path, FileRights::WRITE) {
                return match execute_action_with(
                    &mut file_syscalls,
                    file_grant_set,
                    Action::WriteFile {
                        path,
                        data: content.as_bytes(),
                    },
                ) {
                    Ok(ActionOutcome::WriteFile { bytes }) => {
                        push_bytes(reply, b"arthur: write_file: bytes=")?;
                        push_usize(reply, bytes)
                    }
                    Ok(_) => Err(ConversationError::Action),
                    Err(error) => push_action_error(error, reply),
                };
            }
            if !agentic_grants_include_file_right(file_grant_set, path, FileRights::READ) {
                return push_bytes(
                    reply,
                    b"arthur: restricted: write_file requires read grant for undo snapshot",
                );
            }

            let mut snapshot = [0u8; AGENTIC_UNDO_MAX_BYTES];
            let snapshot_len = match execute_action_with(
                &mut file_syscalls,
                file_grant_set,
                Action::ReadFile {
                    path,
                    out: &mut snapshot,
                },
            ) {
                Ok(ActionOutcome::ReadFile { bytes }) => bytes,
                Ok(_) => return Err(ConversationError::Action),
                Err(error) => return push_action_error(error, reply),
            };
            if !agentic_snapshot_supports_reversible_write(
                snapshot_len,
                snapshot.len(),
                content.len(),
                reply,
            )? {
                return Ok(());
            }
            #[cfg(feature = "arthur-reversibility")]
            let undo_plan = match self.prepare_agentic_reversibility(
                path,
                &snapshot[..snapshot_len],
                confirmed,
                reply,
            )? {
                Some(plan) => plan,
                None => return Ok(()),
            };

            match execute_action_with(
                &mut file_syscalls,
                file_grant_set,
                Action::WriteFile {
                    path,
                    data: content.as_bytes(),
                },
            ) {
                Ok(ActionOutcome::WriteFile { bytes }) => {
                    #[cfg(feature = "arthur-reversibility")]
                    match undo_plan {
                        AgenticUndoPlan::Armed(undo) => {
                            self.undo = Some(undo);
                            push_bytes(reply, b"arthur: write_file: bytes=")?;
                            push_usize(reply, bytes)?;
                            push_bytes(reply, b" undo=prewrite.bin path=")?;
                            push_bytes(reply, path.as_bytes())
                        }
                        AgenticUndoPlan::ConfirmedIrreversible => {
                            self.undo = None;
                            push_bytes(reply, b"arthur: write_file: bytes=")?;
                            push_usize(reply, bytes)?;
                            push_bytes(reply, b" undo=unavailable confirmed=destructive path=")?;
                            push_bytes(reply, path.as_bytes())
                        }
                    }
                    #[cfg(not(feature = "arthur-reversibility"))]
                    {
                        self.undo = Some(AgenticUndo::new(path, &snapshot[..snapshot_len]));
                        push_bytes(reply, b"arthur: write_file: bytes=")?;
                        push_usize(reply, bytes)?;
                        push_bytes(reply, b" undo=snapshot path=")?;
                        push_bytes(reply, path.as_bytes())
                    }
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }

        #[cfg(not(feature = "arthur-grants"))]
        {
            let file_grants = [
                Grant::File {
                    path: HELLO_PATH,
                    rights: FileRights::READ,
                },
                Grant::File {
                    path: SCRATCH_PATH,
                    rights: FileRights::READ.union(FileRights::WRITE),
                },
                #[cfg(feature = "arthur-reversibility")]
                Grant::File {
                    path: DANGER_KEY_PATH,
                    rights: FileRights::READ.union(FileRights::WRITE),
                },
                Grant::File {
                    path: ROOT_PATH,
                    rights: FileRights::LOOKUP.union(FileRights::READDIR),
                },
            ];
            let mut file_syscalls = libneo::action::LibneoActionSyscalls;
            if !agentic_grants_include_file_right(&file_grants, path, FileRights::WRITE) {
                return match execute_action_with(
                    &mut file_syscalls,
                    &file_grants,
                    Action::WriteFile {
                        path,
                        data: content.as_bytes(),
                    },
                ) {
                    Ok(ActionOutcome::WriteFile { bytes }) => {
                        push_bytes(reply, b"arthur: write_file: bytes=")?;
                        push_usize(reply, bytes)
                    }
                    Ok(_) => Err(ConversationError::Action),
                    Err(error) => push_action_error(error, reply),
                };
            }
            if !agentic_grants_include_file_right(&file_grants, path, FileRights::READ) {
                return push_bytes(
                    reply,
                    b"arthur: restricted: write_file requires read grant for undo snapshot",
                );
            }

            let mut snapshot = [0u8; AGENTIC_UNDO_MAX_BYTES];
            let snapshot_len = match execute_action_with(
                &mut file_syscalls,
                &file_grants,
                Action::ReadFile {
                    path,
                    out: &mut snapshot,
                },
            ) {
                Ok(ActionOutcome::ReadFile { bytes }) => bytes,
                Ok(_) => return Err(ConversationError::Action),
                Err(error) => return push_action_error(error, reply),
            };
            if !agentic_snapshot_supports_reversible_write(
                snapshot_len,
                snapshot.len(),
                content.len(),
                reply,
            )? {
                return Ok(());
            }
            #[cfg(feature = "arthur-reversibility")]
            let undo_plan = match self.prepare_agentic_reversibility(
                path,
                &snapshot[..snapshot_len],
                confirmed,
                reply,
            )? {
                Some(plan) => plan,
                None => return Ok(()),
            };

            match execute_action_with(
                &mut file_syscalls,
                &file_grants,
                Action::WriteFile {
                    path,
                    data: content.as_bytes(),
                },
            ) {
                Ok(ActionOutcome::WriteFile { bytes }) => {
                    #[cfg(feature = "arthur-reversibility")]
                    match undo_plan {
                        AgenticUndoPlan::Armed(undo) => {
                            self.undo = Some(undo);
                            push_bytes(reply, b"arthur: write_file: bytes=")?;
                            push_usize(reply, bytes)?;
                            push_bytes(reply, b" undo=prewrite.bin path=")?;
                            push_bytes(reply, path.as_bytes())
                        }
                        AgenticUndoPlan::ConfirmedIrreversible => {
                            self.undo = None;
                            push_bytes(reply, b"arthur: write_file: bytes=")?;
                            push_usize(reply, bytes)?;
                            push_bytes(reply, b" undo=unavailable confirmed=destructive path=")?;
                            push_bytes(reply, path.as_bytes())
                        }
                    }
                    #[cfg(not(feature = "arthur-reversibility"))]
                    {
                        self.undo = Some(AgenticUndo::new(path, &snapshot[..snapshot_len]));
                        push_bytes(reply, b"arthur: write_file: bytes=")?;
                        push_usize(reply, bytes)?;
                        push_bytes(reply, b" undo=snapshot path=")?;
                        push_bytes(reply, path.as_bytes())
                    }
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_trash_action(
        &mut self,
        path: &str,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        self.execute_agentic_recoverable_overwrite(
            path,
            AGENTIC_TRASH_CONTENT,
            b"trash_file",
            None,
            false,
            reply,
        )
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_trash_action_confirmed(
        &mut self,
        path: &str,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        self.execute_agentic_recoverable_overwrite(
            path,
            AGENTIC_TRASH_CONTENT,
            b"trash_file",
            None,
            true,
            reply,
        )
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_move_action(
        &mut self,
        path: &str,
        destination: &str,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        self.execute_agentic_recoverable_overwrite(
            path,
            AGENTIC_TRASH_CONTENT,
            b"move_file",
            Some(destination.as_bytes()),
            false,
            reply,
        )
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_move_action_confirmed(
        &mut self,
        path: &str,
        destination: &str,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        self.execute_agentic_recoverable_overwrite(
            path,
            AGENTIC_TRASH_CONTENT,
            b"move_file",
            Some(destination.as_bytes()),
            true,
            reply,
        )
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_recoverable_overwrite(
        &mut self,
        path: &str,
        content: &str,
        label: &[u8],
        destination: Option<&[u8]>,
        confirmed: bool,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        let mut inner = Vec::new();
        self.execute_agentic_write_action(path, content, confirmed, &mut inner)?;
        if !inner.starts_with(b"arthur: write_file:")
            || !(inner
                .windows(b"undo=snapshot".len())
                .any(|window| window == b"undo=snapshot")
                || inner
                    .windows(b"undo=prewrite.bin".len())
                    .any(|window| window == b"undo=prewrite.bin"))
        {
            reply.clear();
            return push_bytes(reply, &inner);
        }
        reply.clear();
        push_bytes(reply, b"arthur: ")?;
        push_bytes(reply, label)?;
        push_bytes(reply, b": moved_to_trash path=")?;
        push_bytes(reply, path.as_bytes())?;
        if let Some(destination) = destination {
            push_bytes(reply, b" destination=")?;
            push_bytes(reply, destination)?;
        }
        push_bytes(reply, b" undo=A2-snapshot")
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_kill_action(
        &mut self,
        handle: u32,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        let allowed = [Grant::Process {
            handle: ProcessHandle(AGENTIC_A3_KILL_HANDLE),
            rights: CapRights::new(meta_rights::USE, process_rights::KILL),
        }];
        let denied: [Grant<'_>; 0] = [];
        let grants = if handle == AGENTIC_A3_KILL_HANDLE {
            &allowed[..]
        } else {
            &denied[..]
        };
        let mut syscalls = AgenticA3Syscalls;
        match execute_action_with(
            &mut syscalls,
            grants,
            Action::KillProcess {
                handle: ProcessHandle(handle),
            },
        ) {
            Ok(ActionOutcome::KillProcess) => {
                push_bytes(reply, b"arthur: kill_process: confirmed handle=")?;
                push_u32(reply, handle)
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_a3_rechecked_action_error(error, reply),
        }
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_spawn_action(
        &mut self,
        name: &str,
        inherit_handle: u32,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        let grants = [Grant::SpawnProcess {
            name: AGENTIC_A3_SPAWN_NAME,
        }];
        let inherited = [Handle(inherit_handle)];
        let handles = if inherit_handle == 0 {
            &[][..]
        } else {
            &inherited[..]
        };
        let mut syscalls = AgenticA3Syscalls;
        match execute_action_with(
            &mut syscalls,
            &grants,
            Action::SpawnProcess { name, handles },
        ) {
            Ok(ActionOutcome::SpawnProcess { pid, handle }) => {
                push_bytes(reply, b"arthur: spawn_process: pid=")?;
                push_u32(reply, pid)?;
                push_bytes(reply, b" handle=")?;
                push_u32(reply, handle.0)
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_a3_rechecked_action_error(error, reply),
        }
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_external_send_action(
        &mut self,
        host: &str,
        port: u16,
        bytes: usize,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        if bytes > 64 {
            return push_bytes(reply, b"arthur: refused: external_send payload too large");
        }
        let dial_grants = [Grant::NetDial {
            host: AGENTIC_A3_EXTERNAL_HOST,
            port: AGENTIC_A3_EXTERNAL_PORT,
            cap: libneo::NO_NET_DIAL_CAP,
        }];
        let mut syscalls = AgenticA3Syscalls;
        let socket = match execute_action_with(
            &mut syscalls,
            &dial_grants,
            Action::NetDial { host, port },
        ) {
            Ok(ActionOutcome::NetDial { handle }) => handle,
            Ok(_) => return Err(ConversationError::Action),
            Err(error) => return push_a3_rechecked_action_error(error, reply),
        };
        let socket_grants = [Grant::TcpSocket {
            handle: socket,
            rights: TcpRights::WRITE,
        }];
        let payload = [b'x'; 64];
        match execute_action_with(
            &mut syscalls,
            &socket_grants,
            Action::NetSend {
                handle: socket,
                data: &payload[..bytes],
            },
        ) {
            Ok(ActionOutcome::NetSend { bytes }) => {
                push_bytes(reply, b"arthur: external_send: bytes=")?;
                push_usize(reply, bytes)?;
                push_bytes(reply, b" policy=per-send typed-confirm")
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_a3_rechecked_action_error(error, reply),
        }
    }

    #[cfg(feature = "agentic-actions-a3")]
    fn execute_agentic_load_model_action(
        &mut self,
        path: &str,
        reply: &mut Vec<u8>,
    ) -> Result<(), ConversationError> {
        let grants = [Grant::ModelLoad {
            path: AGENTIC_A3_MODEL_PATH,
        }];
        let mut syscalls = AgenticA3Syscalls;
        match execute_action_with(
            &mut syscalls,
            &grants,
            Action::LoadModel {
                path,
                flags: OPEN_ROOT_TEMPORARY,
            },
        ) {
            Ok(ActionOutcome::LoadModel { handle }) => {
                push_bytes(reply, b"arthur: load_model: handle=")?;
                push_u32(reply, handle.0)
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_a3_rechecked_action_error(error, reply),
        }
    }

    #[cfg(feature = "arthur-grants")]
    fn execute_agentic_undo(&mut self, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
        let Some(undo) = self.undo.take() else {
            return push_bytes(reply, b"arthur: undo: no snapshot");
        };

        let mut grants_buf = [0u8; GRANTS_BUF_LEN];
        let mut grant_entries =
            [libneo::grant_store::GrantEntry::EMPTY; libneo::grant_store::MAX_GRANTS];
        let mut file_grants = [PLACEHOLDER_GRANT; MAX_BACKED_GRANTS];
        let mut backed_file_grants = [PLACEHOLDER_BACKED_GRANT; MAX_BACKED_GRANTS];
        let file_grant_count = load_runtime_grants(
            &mut grants_buf,
            &mut grant_entries,
            &mut file_grants,
            &mut backed_file_grants,
        );
        let mut file_syscalls = ArthurGrantSyscalls {
            grants: &backed_file_grants[..file_grant_count],
        };
        let file_grant_set = &file_grants[..file_grant_count];
        #[cfg(feature = "arthur-reversibility")]
        let mut restored = Vec::new();
        #[cfg(feature = "arthur-reversibility")]
        if let Err(reason) = Self::load_agentic_undo_artifact(&undo, &mut restored) {
            if reason == b"artifact-corrupt" {
                return push_bytes(reply, b"arthur: undo artifact corrupt");
            }
            return push_bytes(reply, b"arthur: undo artifact unavailable");
        }
        #[cfg(feature = "arthur-reversibility")]
        let restore_data = restored.as_slice();
        #[cfg(not(feature = "arthur-reversibility"))]
        let restore_data = undo.data.as_slice();

        match execute_action_with(
            &mut file_syscalls,
            file_grant_set,
            Action::WriteFile {
                path: undo.path.as_str(),
                data: restore_data,
            },
        ) {
            Ok(ActionOutcome::WriteFile { bytes }) => {
                push_bytes(reply, b"arthur: undo: write_file restored bytes=")?;
                push_usize(reply, bytes)?;
                push_bytes(reply, b" path=")?;
                push_bytes(reply, undo.path.as_bytes())
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_action_error(error, reply),
        }
    }

    #[cfg(not(feature = "arthur-grants"))]
    fn execute_agentic_undo(&mut self, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
        let Some(undo) = self.undo.take() else {
            return push_bytes(reply, b"arthur: undo: no snapshot");
        };

        let file_grants = [
            Grant::File {
                path: HELLO_PATH,
                rights: FileRights::READ,
            },
            Grant::File {
                path: SCRATCH_PATH,
                rights: FileRights::READ.union(FileRights::WRITE),
            },
            Grant::File {
                path: ROOT_PATH,
                rights: FileRights::LOOKUP.union(FileRights::READDIR),
            },
        ];
        let mut file_syscalls = libneo::action::LibneoActionSyscalls;
        #[cfg(feature = "arthur-reversibility")]
        let mut restored = Vec::new();
        #[cfg(feature = "arthur-reversibility")]
        if let Err(reason) = Self::load_agentic_undo_artifact(&undo, &mut restored) {
            if reason == b"artifact-corrupt" {
                return push_bytes(reply, b"arthur: undo artifact corrupt");
            }
            return push_bytes(reply, b"arthur: undo artifact unavailable");
        }
        #[cfg(feature = "arthur-reversibility")]
        let restore_data = restored.as_slice();
        #[cfg(not(feature = "arthur-reversibility"))]
        let restore_data = undo.data.as_slice();
        match execute_action_with(
            &mut file_syscalls,
            &file_grants,
            Action::WriteFile {
                path: undo.path.as_str(),
                data: restore_data,
            },
        ) {
            Ok(ActionOutcome::WriteFile { bytes }) => {
                push_bytes(reply, b"arthur: undo: write_file restored bytes=")?;
                push_usize(reply, bytes)?;
                push_bytes(reply, b" path=")?;
                push_bytes(reply, undo.path.as_bytes())
            }
            Ok(_) => Err(ConversationError::Action),
            Err(error) => push_action_error(error, reply),
        }
    }
}

#[cfg(feature = "agentic-actions")]
fn agentic_grants_include_file_right(grants: &[Grant<'_>], path: &str, needed: FileRights) -> bool {
    grants.iter().any(|grant| match *grant {
        Grant::File {
            path: grant_path,
            rights,
        } => grant_path == path && (rights.bits() & needed.bits()) == needed.bits(),
        _ => false,
    })
}

#[cfg(feature = "agentic-actions")]
fn agentic_snapshot_supports_reversible_write(
    snapshot_len: usize,
    snapshot_cap: usize,
    content_len: usize,
    reply: &mut Vec<u8>,
) -> Result<bool, ConversationError> {
    if snapshot_len == 0 {
        push_bytes(
            reply,
            b"arthur: restricted: write_file cannot undo empty preimage without truncate",
        )?;
        return Ok(false);
    }
    if snapshot_len == snapshot_cap {
        push_bytes(
            reply,
            b"arthur: restricted: write_file snapshot exceeds A2 undo window",
        )?;
        return Ok(false);
    }
    if content_len != snapshot_len {
        push_bytes(
            reply,
            b"arthur: restricted: write_file content length must match snapshot without truncate",
        )?;
        return Ok(false);
    }
    Ok(true)
}

#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
fn execute_runtime_tool(call: ToolCall<'_>, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
    #[cfg(feature = "arthur-grants")]
    let mut grants_buf = [0u8; GRANTS_BUF_LEN];
    #[cfg(feature = "arthur-grants")]
    let mut grant_entries =
        [libneo::grant_store::GrantEntry::EMPTY; libneo::grant_store::MAX_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let mut file_grants = [PLACEHOLDER_GRANT; MAX_BACKED_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let mut backed_file_grants = [PLACEHOLDER_BACKED_GRANT; MAX_BACKED_GRANTS];
    #[cfg(feature = "arthur-grants")]
    let file_grant_count = load_runtime_grants(
        &mut grants_buf,
        &mut grant_entries,
        &mut file_grants,
        &mut backed_file_grants,
    );
    #[cfg(feature = "arthur-grants")]
    let mut file_syscalls = ArthurGrantSyscalls {
        grants: &backed_file_grants[..file_grant_count],
    };
    #[cfg(feature = "arthur-grants")]
    let file_grant_set = &file_grants[..file_grant_count];

    #[cfg(not(feature = "arthur-grants"))]
    let file_grants = [
        Grant::File {
            path: HELLO_PATH,
            rights: FileRights::READ,
        },
        Grant::File {
            path: ROOT_PATH,
            rights: FileRights::LOOKUP.union(FileRights::READDIR),
        },
    ];
    #[cfg(not(feature = "arthur-grants"))]
    let mut file_syscalls = libneo::action::LibneoActionSyscalls;
    #[cfg(not(feature = "arthur-grants"))]
    let file_grant_set = &file_grants;

    match call {
        ToolCall::ReadFile { path } => {
            let mut out = [0u8; 96];
            match execute_action_with(
                &mut file_syscalls,
                file_grant_set,
                Action::ReadFile {
                    path,
                    out: &mut out,
                },
            ) {
                Ok(ActionOutcome::ReadFile { bytes }) => {
                    push_bytes(reply, b"arthur: read_file: ")?;
                    push_bytes(reply, &out[..bytes])?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::WriteFile { path, content } => {
            match execute_action_with(
                &mut file_syscalls,
                file_grant_set,
                Action::WriteFile {
                    path,
                    data: content.as_bytes(),
                },
            ) {
                Ok(ActionOutcome::WriteFile { bytes }) => {
                    push_bytes(reply, b"arthur: write_file: bytes=")?;
                    push_usize(reply, bytes)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::ListDir { path } => {
            let mut out = [0u8; 96];
            let mut cursor = 0u64;
            match execute_action_with(
                &mut file_syscalls,
                file_grant_set,
                Action::ListDir {
                    path,
                    out: &mut out,
                    cursor: &mut cursor,
                },
            ) {
                Ok(ActionOutcome::ListDir { bytes }) => {
                    push_bytes(reply, b"arthur: list_dir: bytes=")?;
                    push_usize(reply, bytes)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::SpawnProcess { name } => {
            let grants = [Grant::SpawnProcess { name: "worker" }];
            match execute_action(&grants, Action::SpawnProcess { name, handles: &[] }) {
                Ok(ActionOutcome::SpawnProcess { pid, .. }) => {
                    push_bytes(reply, b"arthur: spawn_process: pid=")?;
                    push_u32(reply, pid)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::KillProcess { handle } => execute_process_action(
            Action::KillProcess {
                handle: ProcessHandle(handle),
            },
            reply,
        ),
        ToolCall::InspectProcess { handle } => execute_process_action(
            Action::InspectProcess {
                handle: ProcessHandle(handle),
            },
            reply,
        ),
        ToolCall::WaitProcess { handle } => execute_process_action(
            Action::WaitProcess {
                handle: ProcessHandle(handle),
                flags: 0,
            },
            reply,
        ),
        ToolCall::NetDial { host, port } => {
            let grants: [Grant<'_>; 0] = [];
            match execute_action(&grants, Action::NetDial { host, port }) {
                Ok(ActionOutcome::NetDial { handle }) => {
                    push_bytes(reply, b"arthur: net_dial: handle=")?;
                    push_u32(reply, handle.0)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::NetSend { handle, data } => {
            let grants: [Grant<'_>; 0] = [];
            match execute_action(
                &grants,
                Action::NetSend {
                    handle: TcpSocketHandle(handle),
                    data: data.as_bytes(),
                },
            ) {
                Ok(ActionOutcome::NetSend { bytes }) => {
                    push_bytes(reply, b"arthur: net_send: bytes=")?;
                    push_usize(reply, bytes)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::NetRecv { handle, max_bytes } => {
            let grants: [Grant<'_>; 0] = [];
            let mut out = [0u8; 96];
            let len = core::cmp::min(max_bytes, out.len());
            match execute_action(
                &grants,
                Action::NetRecv {
                    handle: TcpSocketHandle(handle),
                    out: &mut out[..len],
                },
            ) {
                Ok(ActionOutcome::NetRecv { bytes }) => {
                    push_bytes(reply, b"arthur: net_recv: bytes=")?;
                    push_usize(reply, bytes)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::Infer { model_handle, .. } => {
            let grants: [Grant<'_>; 0] = [];
            let mut outputs: [TensorHandle; 0] = [];
            match execute_action(
                &grants,
                Action::Infer {
                    model: ModelHandle(model_handle),
                    inputs: &[],
                    outputs: &mut outputs,
                    flags: 0,
                },
            ) {
                Ok(ActionOutcome::Infer { output_count }) => {
                    push_bytes(reply, b"arthur: infer: outputs=")?;
                    push_usize(reply, output_count)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
        ToolCall::LoadModel { path } => {
            let grants: [Grant<'_>; 0] = [];
            match execute_action(
                &grants,
                Action::LoadModel {
                    path,
                    flags: OPEN_ROOT_TEMPORARY,
                },
            ) {
                Ok(ActionOutcome::LoadModel { handle }) => {
                    push_bytes(reply, b"arthur: load_model: handle=")?;
                    push_u32(reply, handle.0)?;
                    Ok(())
                }
                Ok(_) => Err(ConversationError::Action),
                Err(error) => push_action_error(error, reply),
            }
        }
    }
}

#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
fn execute_process_action(
    action: Action<'_>,
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    let grants: [Grant<'_>; 0] = [];
    match execute_action(&grants, action) {
        Ok(outcome) => {
            push_bytes(reply, b"arthur: process action ok: ")?;
            push_action_kind(reply, outcome_kind(outcome))?;
            Ok(())
        }
        Err(error) => push_action_error(error, reply),
    }
}

#[cfg(feature = "arthur-act")]
fn push_action_error(error: ActionError<'_>, reply: &mut Vec<u8>) -> Result<(), ConversationError> {
    match error {
        ActionError::Denied { action, required } => {
            push_bytes(reply, b"arthur: Denied: ")?;
            push_action_kind(reply, action)?;
            push_bytes(reply, b" requires ")?;
            push_required(reply, required)
        }
        ActionError::Syscall { action, errno } => {
            push_bytes(reply, b"arthur: syscall failed: ")?;
            push_action_kind(reply, action)?;
            push_bytes(reply, b" errno=")?;
            push_i64(reply, errno)
        }
    }
}

#[cfg(feature = "agentic-actions-a3")]
fn push_a3_rechecked_action_error(
    error: ActionError<'_>,
    reply: &mut Vec<u8>,
) -> Result<(), ConversationError> {
    push_action_error(error, reply)?;
    push_bytes(reply, b" rechecked-after-confirm")
}

#[cfg(feature = "arthur-act")]
fn push_required(
    reply: &mut Vec<u8>,
    required: RequiredCapability<'_>,
) -> Result<(), ConversationError> {
    match required {
        RequiredCapability::File { path, rights } => {
            push_bytes(reply, b"file ")?;
            push_bytes(reply, path.as_bytes())?;
            push_bytes(reply, b" rights=0x")?;
            push_hex_u32(reply, rights.bits())
        }
        RequiredCapability::SpawnProcess { name } => {
            push_bytes(reply, b"spawn ")?;
            push_bytes(reply, name.as_bytes())
        }
        RequiredCapability::InheritHandle(handle) => {
            push_bytes(reply, b"inherit handle ")?;
            push_u32(reply, handle.0)
        }
        RequiredCapability::Process { handle, rights } => {
            push_bytes(reply, b"process ")?;
            push_u32(reply, handle.0)?;
            push_bytes(reply, b" rights=0x")?;
            push_hex_u32(reply, u32::from(rights.semantic))
        }
        RequiredCapability::NetDial { host, port } => {
            push_bytes(reply, b"net_dial ")?;
            push_bytes(reply, host.as_bytes())?;
            push_bytes(reply, b":")?;
            push_u32(reply, u32::from(port))
        }
        RequiredCapability::TcpSocket { handle, rights } => {
            push_bytes(reply, b"tcp ")?;
            push_u32(reply, handle.0)?;
            push_bytes(reply, b" rights=0x")?;
            push_hex_u32(reply, u32::from(rights.bits()))
        }
        RequiredCapability::ModelLoad { path } => {
            push_bytes(reply, b"model_load ")?;
            push_bytes(reply, path.as_bytes())
        }
        RequiredCapability::ModelUse(handle) => {
            push_bytes(reply, b"model_use ")?;
            push_u32(reply, handle.0)
        }
        RequiredCapability::TensorUse(handle) => {
            push_bytes(reply, b"tensor_use ")?;
            push_u32(reply, handle.0)
        }
    }
}

#[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
fn outcome_kind(outcome: ActionOutcome) -> ActionKind {
    match outcome {
        ActionOutcome::ReadFile { .. } => ActionKind::ReadFile,
        ActionOutcome::WriteFile { .. } => ActionKind::WriteFile,
        ActionOutcome::ListDir { .. } => ActionKind::ListDir,
        ActionOutcome::SpawnProcess { .. } => ActionKind::SpawnProcess,
        ActionOutcome::KillProcess => ActionKind::KillProcess,
        ActionOutcome::InspectProcess { .. } => ActionKind::InspectProcess,
        ActionOutcome::WaitProcess { .. } => ActionKind::WaitProcess,
        ActionOutcome::NetDial { .. } => ActionKind::NetDial,
        ActionOutcome::NetSend { .. } => ActionKind::NetSend,
        ActionOutcome::NetRecv { .. } => ActionKind::NetRecv,
        ActionOutcome::Infer { .. } => ActionKind::Infer,
        ActionOutcome::LoadModel { .. } => ActionKind::LoadModel,
    }
}

#[cfg(feature = "arthur-act")]
fn push_action_kind(reply: &mut Vec<u8>, action: ActionKind) -> Result<(), ConversationError> {
    let name = match action {
        ActionKind::ReadFile => b"read_file".as_slice(),
        ActionKind::WriteFile => b"write_file".as_slice(),
        ActionKind::ListDir => b"list_dir".as_slice(),
        ActionKind::SpawnProcess => b"spawn_process".as_slice(),
        ActionKind::KillProcess => b"kill_process".as_slice(),
        ActionKind::InspectProcess => b"inspect_process".as_slice(),
        ActionKind::WaitProcess => b"wait_process".as_slice(),
        ActionKind::NetDial => b"net_dial".as_slice(),
        ActionKind::NetSend => b"net_send".as_slice(),
        ActionKind::NetRecv => b"net_recv".as_slice(),
        ActionKind::Infer => b"infer".as_slice(),
        ActionKind::LoadModel => b"load_model".as_slice(),
        ActionKind::CloseFile => b"close_file".as_slice(),
    };
    push_bytes(reply, name)
}

#[cfg(feature = "agentic-ipc-frequency")]
fn append_agentic_frequency_line(
    reply: &mut Vec<u8>,
    actions: &mut RuntimeActions,
    before: AgenticFrequencyTurn,
) -> Result<(), ConversationError> {
    if reply.starts_with(b"arthur: multi-step ") {
        let _ = actions.take_agentic_measurement();
        return Ok(());
    }
    let Some(measured) = actions.take_agentic_measurement() else {
        return Ok(());
    };
    let after_action = libneo::action::action_observability_snapshot();
    let execute_round_trips = after_action
        .execute_round_trips
        .saturating_sub(before.action.execute_round_trips);
    let cap_checks = after_action
        .cap_checks
        .saturating_sub(before.action.cap_checks);
    let kernel_delta = match (before.kernel, AgenticKernelCounters::snapshot()) {
        (Some(start), Some(end)) => Some(end.delta_since(start).without_measurement_observe()),
        _ => None,
    };

    push_bytes(reply, b"\narthur: ipc_freq tier=")?;
    push_bytes(reply, measured.tier)?;
    push_bytes(reply, b" action=")?;
    push_bytes(reply, measured.action)?;
    if let Some(delta) = kernel_delta {
        push_bytes(reply, b" syscalls=")?;
        push_u64(reply, delta.total)?;
        push_bytes(reply, b" ipc=")?;
        push_u64(reply, delta.ipc)?;
        push_bytes(reply, b" file=")?;
        push_u64(reply, delta.file)?;
        push_bytes(reply, b" ai=")?;
        push_u64(reply, delta.ai)?;
        push_bytes(reply, b" net=")?;
        push_u64(reply, delta.net)?;
        push_bytes(reply, b" other=")?;
        push_u64(reply, delta.other)?;
    } else {
        push_bytes(reply, b" syscalls=unavailable ipc=unavailable")?;
    }
    push_bytes(reply, b" cap_checks=")?;
    push_u64(reply, cap_checks)?;
    push_bytes(reply, b" exec_round_trips=")?;
    push_u64(reply, execute_round_trips)
}

#[cfg(feature = "arthur-act")]
fn push_bytes(reply: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ConversationError> {
    if reply.len().saturating_add(bytes.len()) > conversation_logic::MAX_REPLY_BYTES {
        return Err(ConversationError::ReplyTooLarge);
    }
    reply.extend_from_slice(bytes);
    Ok(())
}

#[cfg(feature = "arthur-act")]
fn push_usize(reply: &mut Vec<u8>, value: usize) -> Result<(), ConversationError> {
    push_u64(reply, value as u64)
}

#[cfg(feature = "arthur-act")]
fn push_u32(reply: &mut Vec<u8>, value: u32) -> Result<(), ConversationError> {
    push_u64(reply, u64::from(value))
}

#[cfg(feature = "arthur-act")]
fn push_i64(reply: &mut Vec<u8>, value: i64) -> Result<(), ConversationError> {
    if value < 0 {
        push_bytes(reply, b"-")?;
        push_u64(reply, value.unsigned_abs())
    } else {
        push_u64(reply, value as u64)
    }
}

#[cfg(feature = "arthur-act")]
fn push_u64(reply: &mut Vec<u8>, mut value: u64) -> Result<(), ConversationError> {
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
    push_bytes(reply, &buf[idx..])
}

#[cfg(feature = "arthur-act")]
fn push_hex_u32(reply: &mut Vec<u8>, mut value: u32) -> Result<(), ConversationError> {
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
    push_bytes(reply, &buf[idx..])
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(253);
}
