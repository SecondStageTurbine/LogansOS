// SPDX-License-Identifier: MPL-2.0
//! Host tests for neo-arthur conversation relay logic.

#![allow(dead_code)]

// `memory_crypto.rs` is written `no_std`-style (`use alloc::vec::Vec`); this is a
// std test crate, so pull `alloc` in by name for the mounted module to resolve.
extern crate alloc;

#[path = "../../../userland/neo-arthur/src/agentic_action_logic.rs"]
mod agentic_action_logic;
#[path = "../../../userland/neo-arthur/src/conversation_logic.rs"]
mod conversation_logic;
#[path = "../../../userland/neo-arthur/src/desktop_action_logic.rs"]
mod desktop_action_logic;
#[path = "../../../userland/neo-arthur/src/intent_logic.rs"]
mod intent_logic;
#[path = "../../../userland/neo-arthur/src/memory_logic.rs"]
mod memory_logic;
// The REAL M2 crypto core (no libneo, no MemoryType deps), mounted so the
// byte-oracle below binds the exact functions the binary ships (B4 fix).
#[path = "../../../userland/neo-arthur/src/memory_crypto.rs"]
mod memory_crypto;
#[path = "../../../userland/neo-arthur/src/model_state_logic.rs"]
mod model_state_logic;
#[path = "../../../userland/neo-arthur/src/reversibility_logic.rs"]
mod reversibility_logic;
#[path = "../../../userland/neo-arthur/src/showcase_logic.rs"]
mod showcase_logic;
#[path = "../../../userland/neo-arthur/src/showcase_text.rs"]
mod showcase_text;

/// L1.3a: a desktop action is an explicit command line only; chat that
/// merely mentions a verb or a path is not one.
#[cfg(test)]
mod desktop_action_tests {
    use super::desktop_action_logic::{DesktopAction, LIST, READ, WRITE, parse};

    #[test]
    fn explicit_commands_parse() {
        assert_eq!(
            parse("read /neofs/notes.txt"),
            Some(DesktopAction::Read {
                path: "/neofs/notes.txt"
            })
        );
        assert_eq!(
            parse("  list /neofs/docs/  "),
            Some(DesktopAction::List {
                path: "/neofs/docs/"
            })
        );
        assert_eq!(
            parse("write /neofs/notes.txt buy milk"),
            Some(DesktopAction::Write {
                path: "/neofs/notes.txt",
                text: "buy milk"
            })
        );
        let kinds: Vec<u16> = ["read /a", "list /a", "write /a x"]
            .iter()
            .map(|l| parse(l).unwrap().kind())
            .collect();
        assert_eq!(kinds, [READ, LIST, WRITE]);
        assert_eq!(parse("read /neofs/x").unwrap().path(), "/neofs/x");
    }

    #[test]
    fn chat_is_not_an_action() {
        for line in [
            "hi",
            "show me the state of things",
            "please read /neofs/notes.txt",
            "read notes.txt",
            "read /",
            "read /neofs/a extra",
            "write /neofs/a",
            "list",
            "update /neofs/a x",
            "Read /neofs/a",
            "",
        ] {
            assert_eq!(parse(line), None, "{line:?}");
        }
    }
}

/// B1b (THINK Q4): Arthur's model states for a boot-module model.
#[cfg(test)]
mod model_state_tests {
    use super::model_state_logic::{
        EBADMSG, ENOENT, ModelState, beginner_line, state_for_load_error, state_tag,
    };

    const NOT_READY: [ModelState; 3] = [
        ModelState::Absent,
        ModelState::Damaged,
        ModelState::LoadFailed,
    ];

    /// OT-87 R2: from the installed disk, absent and damaged name the disk
    /// and never point at a USB; every other sentence is the boot-module one.
    #[test]
    fn installed_disk_lines_name_the_disk() {
        use super::model_state_logic::{ModelSource, line_for, source_tag};
        assert_eq!(source_tag(ModelSource::BootModule), b"boot-module");
        assert_eq!(source_tag(ModelSource::Neofs), b"neofs");
        for state in NOT_READY {
            assert_eq!(
                line_for(state, ModelSource::BootModule),
                beginner_line(state)
            );
        }
        assert_eq!(line_for(ModelState::Ready, ModelSource::Neofs), None);
        assert_eq!(
            line_for(ModelState::LoadFailed, ModelSource::Neofs),
            beginner_line(ModelState::LoadFailed)
        );
        for state in [ModelState::Absent, ModelState::Damaged] {
            let line = line_for(state, ModelSource::Neofs).unwrap();
            let text = core::str::from_utf8(line).unwrap();
            assert!(text.contains("disk"), "{text}");
            assert!(!text.contains("USB"), "{text}");
            assert_ne!(Some(line), beginner_line(state));
        }
    }

    /// Chad's R2 review, item 4: a boot-module model with a NeoFS vocab is
    /// reported as `source=boot-module`; the vocab's source is reported only
    /// when the vocab is what failed.
    #[test]
    fn the_reported_source_is_the_models_unless_the_vocab_failed() {
        use super::model_state_logic::{ModelSource, VocabLoad, reported_source, source_tag};
        let (boot, neofs) = (ModelSource::BootModule, ModelSource::Neofs);
        assert_eq!(
            source_tag(reported_source(VocabLoad::Loaded, neofs, boot)),
            b"boot-module",
            "the mixed case"
        );
        assert_eq!(reported_source(VocabLoad::Loaded, neofs, neofs), neofs);
        assert_eq!(reported_source(VocabLoad::Loaded, boot, boot), boot);
        for failed in [
            VocabLoad::Absent,
            VocabLoad::Damaged,
            VocabLoad::Unreadable(-5),
            VocabLoad::Unparseable,
        ] {
            assert_eq!(reported_source(failed, neofs, boot), neofs, "{failed:?}");
            assert_eq!(reported_source(failed, boot, neofs), boot, "{failed:?}");
        }
    }

    #[test]
    fn errnos_match_the_kernel_abi() {
        // kernel/src/errno.rs and libneo: ENOENT = -2, EBADMSG = -74.
        assert_eq!(ENOENT, -2);
        assert_eq!(EBADMSG, -74);
        assert_eq!(state_for_load_error(-2), ModelState::Absent);
        assert_eq!(state_for_load_error(-74), ModelState::Damaged);
        for other in [-1, -12, -22, -116, 0, i64::MIN] {
            assert_eq!(
                state_for_load_error(other),
                ModelState::LoadFailed,
                "{other}"
            );
        }
    }

    /// The THINK's falsifier: a beginner reading only the screen can tell the
    /// states apart, and no line carries an error code.
    #[test]
    fn lines_are_pairwise_distinct_and_carry_no_error_code() {
        assert_eq!(beginner_line(ModelState::Ready), None);
        let lines: Vec<&[u8]> = NOT_READY
            .iter()
            .map(|s| beginner_line(*s).unwrap())
            .collect();
        for (i, a) in lines.iter().enumerate() {
            for b in &lines[i + 1..] {
                assert_ne!(a, b);
            }
            assert!(
                !a.iter().any(u8::is_ascii_digit),
                "{}",
                String::from_utf8_lossy(a)
            );
            let text = String::from_utf8_lossy(a).to_ascii_lowercase();
            for code in ["errno", "enoent", "ebadmsg", "error"] {
                assert!(!text.contains(code), "{text}");
            }
            assert!(a.ends_with(b"."), "one full sentence per step");
        }
        assert_eq!(
            beginner_line(ModelState::Damaged).unwrap(),
            b"My model file is damaged, so I won't use it. Re-create the USB from the original image."
        );
    }

    #[test]
    fn tags_are_distinct() {
        let tags = [
            state_tag(ModelState::Ready),
            state_tag(ModelState::Absent),
            state_tag(ModelState::Damaged),
            state_tag(ModelState::LoadFailed),
        ];
        for (i, a) in tags.iter().enumerate() {
            for b in &tags[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}

#[cfg(test)]
mod showcase_s2_tests;

#[cfg(test)]
mod showcase_tests {
    use super::showcase_logic::{
        OutputRoute, ScrollKey, glyph_rows, input_event_to_ascii, input_event_to_scroll,
        keycode_to_ascii, output_route, wants_root_lens,
    };

    /// DS-1c: the Lens blooms only for the exact supported forms (Chad DS-1c
    /// F2: parsed positively — any other path, relative or absolute, or any
    /// extra word keeps the line with the model).
    #[test]
    fn root_lens_intent() {
        for yes in [
            &b"list /"[..],
            b"ls /",
            b"LS /",
            b"  ls   / ",
            b"list directory /",
            b"list the directory /",
            b"List The Directory /",
        ] {
            assert!(wants_root_lens(yes), "{:?}", core::str::from_utf8(yes));
        }
        for no in [
            &b"list /neofs"[..],
            b"ls / /tmp",
            // Chad DS-1c F2: relative paths after `/`.
            b"ls / tmp",
            b"list / ../scratch",
            b"ls / .",
            b"ls tmp /",
            b"ls / please",
            b"please ls /",
            b"list the directory / now",
            b"ls //",
            b"ls /.",
            b"ls\t/\tx",
            b"ls",
            b"/",
            b"list my favourite films",
            b"hello desktop",
            b"lists /",
            b"",
        ] {
            assert!(!wants_root_lens(no), "{:?}", core::str::from_utf8(no));
        }
    }

    #[test]
    fn integrated_output_route_is_two_layer_and_defaults_to_standalone() {
        assert_eq!(output_route(false, false), OutputRoute::StandaloneScenarios);
        assert_eq!(output_route(false, true), OutputRoute::StandaloneScenarios);
        assert_eq!(output_route(true, false), OutputRoute::StandaloneScenarios);
        assert_eq!(output_route(true, true), OutputRoute::G1ClientSurface);
    }

    #[test]
    fn adr074_keyboard_decoder_matches_hid_ascii_fixture() {
        assert_eq!(keycode_to_ascii(0x04, false), Some(b'a'));
        assert_eq!(keycode_to_ascii(0x04, true), Some(b'A'));
        assert_eq!(keycode_to_ascii(0x28, false), Some(b'\n'));
        assert_eq!(keycode_to_ascii(0x2a, false), Some(0x08));
        assert_eq!(keycode_to_ascii(0x38, true), Some(b'?'));
        // ADR-074 key events: modifiers in x, lock state (Caps Lock) in y.
        assert_eq!(
            input_event_to_ascii([1, 1, 0x33, 0, 0x02, 0, 0, 0]),
            Some(b':')
        );
        assert_eq!(
            input_event_to_ascii([1, 1, 0x34, 0, 0x20, 0, 0, 0]),
            Some(b'"')
        );
        assert_eq!(
            input_event_to_ascii([1, 1, 0x1d, 0, 0x00, 0, 1, 0]),
            Some(b'Z')
        );
        assert_eq!(
            input_event_to_ascii([1, 1, 0x1d, 0, 0x20, 0, 1, 0]),
            Some(b'z')
        );

        // Keypad: digits with Num Lock on, keypad Enter as Enter; with Num
        // Lock off keypad 3 types nothing and scrolls a page down.
        assert_eq!(
            input_event_to_ascii([1, 1, 0x62, 0, 0, 0, 0, 0]),
            Some(b'0')
        );
        assert_eq!(
            input_event_to_ascii([1, 1, 0x58, 0, 0, 0, 0, 0]),
            Some(b'\n')
        );
        assert_eq!(input_event_to_ascii([1, 1, 0x5b, 0, 0, 0, 2, 0]), None);
        assert_eq!(
            input_event_to_scroll([1, 1, 0x5b, 0, 0, 0, 2, 0]),
            Some(ScrollKey::PageDown)
        );
        assert_eq!(input_event_to_scroll([1, 1, 0x5b, 0, 0, 0, 0, 0]), None);

        let pressed_a = [1, 1, 0x04, 0, 0, 0, 0, 0];
        let released_a = [1, 0, 0x04, 0, 0, 0, 0, 0];
        let shifted_a = [1, 1, 0x04, 0, 0x02, 0, 0, 0];
        assert_eq!(input_event_to_ascii(pressed_a), Some(b'a'));
        assert_eq!(input_event_to_ascii(released_a), None);
        assert_eq!(input_event_to_ascii(shifted_a), Some(b'A'));
    }

    #[test]
    fn greeting_pixel_fixture_is_stable() {
        assert_eq!(glyph_rows(b'A'), [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]);
        assert_eq!(glyph_rows(b'a'), glyph_rows(b'A'));
    }
}

#[cfg(test)]
mod tests {
    use super::agentic_action_logic::{
        AGENTIC_FIXTURE_CONFIRM_KILL_4242_TOKEN, AGENTIC_FIXTURE_CONFIRM_TOKEN,
        AGENTIC_FIXTURE_CONFIRM_TRASH_TOKEN, AGENTIC_FIXTURE_DONE_TOKEN,
        AGENTIC_FIXTURE_EXTERNAL_SEND_TOKEN, AGENTIC_FIXTURE_KILL_4242_TOKEN,
        AGENTIC_FIXTURE_READ_HELLO_TOKEN, AGENTIC_FIXTURE_READ_MISSING_TOKEN,
        AGENTIC_FIXTURE_TRASH_SCRATCH_TOKEN, AGENTIC_FIXTURE_UNDO_TOKEN,
        AGENTIC_FIXTURE_WRITE_DANGER_KEY_TOKEN, AGENTIC_FIXTURE_WRITE_SCRATCH_TOKEN, AgenticAction,
        AgenticActionKind, AgenticAstError, AgenticEffectTier, agentic_effect_tier,
        agentic_fixture_oracle_token_for_ast, fallback_agentic_action, parse_agentic_ast,
        push_agentic_candidate_asts, render_agentic_ast,
    };
    #[cfg(not(feature = "agentic-actions"))]
    use super::conversation_logic::ConversationBackend;
    #[cfg(all(not(feature = "arthur-act"), not(feature = "agentic-actions")))]
    use super::conversation_logic::respond_to_turn;
    use super::conversation_logic::{
        AgenticActionDecoder, AgenticActionExecutor, AgenticPosture, ConversationError,
        ConversationState, ReplyMode, respond_to_turn_with_agentic_actions,
    };
    #[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
    use super::conversation_logic::{ToolActionExecutor, respond_to_turn_with_actions};
    use super::intent_logic::{
        ToolCall, ToolKind, ToolParseError, extract_tool_call, render_hermes_prompt,
    };
    use super::memory_logic::{
        MemoryError, MemoryMode, MemoryRecord, MemoryType, StoreFailure, category_path,
        encode_record, fail_safe_mode, parse_record_line, parse_records_lossy,
    };
    use std::string::String;
    use std::vec::Vec;

    struct MockBackend {
        calls: Vec<&'static str>,
        fail_generate: bool,
        decoded: Vec<&'static [u8]>,
        decode_idx: usize,
    }

    impl Default for MockBackend {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                fail_generate: false,
                decoded: vec![b"arthur: model: ok"],
                decode_idx: 0,
            }
        }
    }

    #[cfg(not(feature = "agentic-actions"))]
    impl ConversationBackend for MockBackend {
        fn tokenize(&mut self, input: &str, out: &mut Vec<u32>) -> Result<(), ConversationError> {
            self.calls.push("tokenize");
            if input.contains("write_file") {
                self.calls.push("saw_write_tool");
            }
            out.extend(input.bytes().map(|byte| u32::from(byte % 8) + 1));
            Ok(())
        }

        fn generate(
            &mut self,
            context: &[u32],
            out: &mut Vec<u32>,
        ) -> Result<(), ConversationError> {
            self.calls.push("generate");
            if self.fail_generate {
                return Err(ConversationError::Generate);
            }
            assert!(!context.is_empty());
            out.extend_from_slice(&[42, 43]);
            Ok(())
        }

        fn decode(&mut self, tokens: &[u32], out: &mut Vec<u8>) -> Result<(), ConversationError> {
            self.calls.push("decode");
            assert_eq!(tokens, &[42, 43]);
            let idx = core::cmp::min(self.decode_idx, self.decoded.len() - 1);
            out.extend_from_slice(self.decoded[idx]);
            self.decode_idx += 1;
            Ok(())
        }
    }

    impl AgenticActionDecoder for MockBackend {
        fn decode_agentic_action(
            &mut self,
            _state: &mut ConversationState,
            _user_text: &str,
            _available_actions: &[AgenticActionKind],
            out: &mut Vec<u8>,
        ) -> Result<(), ConversationError> {
            self.calls.push("agentic_decode");
            let idx = core::cmp::min(self.decode_idx, self.decoded.len() - 1);
            out.extend_from_slice(self.decoded[idx]);
            self.decode_idx += 1;
            Ok(())
        }
    }

    struct MockActions {
        tools: &'static [ToolKind],
        denied: bool,
        seen: Vec<ToolKind>,
    }

    #[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
    impl ToolActionExecutor for MockActions {
        fn available_tools(&self) -> &[ToolKind] {
            self.tools
        }

        fn execute_tool_call(
            &mut self,
            call: ToolCall<'_>,
            reply: &mut Vec<u8>,
        ) -> Result<(), ConversationError> {
            self.seen.push(call.kind());
            if self.denied {
                reply.extend_from_slice(b"arthur: Denied: write_file requires file /scratch");
            } else {
                match call {
                    ToolCall::ReadFile { path } => {
                        reply.extend_from_slice(b"arthur: read_file: ");
                        reply.extend_from_slice(path.as_bytes());
                    }
                    _ => reply.extend_from_slice(b"arthur: action ok"),
                }
            }
            Ok(())
        }
    }

    impl AgenticActionExecutor for MockActions {
        fn available_agentic_actions(&self) -> &[AgenticActionKind] {
            &[
                AgenticActionKind::ReadFile,
                AgenticActionKind::WriteFile,
                AgenticActionKind::ListDir,
                AgenticActionKind::ChangeDir,
                AgenticActionKind::Status,
            ]
        }

        fn execute_agentic_action(
            &mut self,
            action: AgenticAction<'_>,
            reply: &mut Vec<u8>,
        ) -> Result<(), ConversationError> {
            match action {
                AgenticAction::ReadFile { path } => {
                    self.seen.push(ToolKind::ReadFile);
                    if self.denied {
                        reply.extend_from_slice(b"arthur: Denied: read_file requires file ");
                        reply.extend_from_slice(path.as_bytes());
                    } else {
                        reply.extend_from_slice(b"arthur: read_file: ");
                        reply.extend_from_slice(path.as_bytes());
                    }
                }
                AgenticAction::ListDir { path } => {
                    self.seen.push(ToolKind::ListDir);
                    reply.extend_from_slice(b"arthur: list_dir: ");
                    reply.extend_from_slice(path.as_bytes());
                }
                AgenticAction::ChangeDir { path } => {
                    self.seen.push(ToolKind::ListDir);
                    reply.extend_from_slice(b"arthur: cd: ");
                    reply.extend_from_slice(path.as_bytes());
                }
                AgenticAction::Status => {
                    reply.extend_from_slice(b"arthur: status: ready");
                }
                AgenticAction::WriteFile { content, .. } => {
                    self.seen.push(ToolKind::WriteFile);
                    if self.denied {
                        reply.extend_from_slice(
                            b"arthur: Denied: write_file requires file /scratch",
                        );
                    } else {
                        reply.extend_from_slice(b"arthur: write_file: bytes=");
                        reply.extend_from_slice(content.len().to_string().as_bytes());
                        reply.extend_from_slice(b" undo=snapshot path=/scratch");
                    }
                }
                AgenticAction::Undo => {
                    self.seen.push(ToolKind::WriteFile);
                    reply.extend_from_slice(
                        b"arthur: undo: write_file restored bytes=24 path=/scratch",
                    );
                }
                AgenticAction::TrashFile { path } => {
                    self.seen.push(ToolKind::WriteFile);
                    reply.extend_from_slice(b"arthur: trash_file: moved_to_trash path=");
                    reply.extend_from_slice(path.as_bytes());
                    reply.extend_from_slice(b" undo=A2-snapshot");
                }
                AgenticAction::MoveFile { path, destination } => {
                    self.seen.push(ToolKind::WriteFile);
                    reply.extend_from_slice(b"arthur: move_file: moved_to_trash path=");
                    reply.extend_from_slice(path.as_bytes());
                    reply.extend_from_slice(b" destination=");
                    reply.extend_from_slice(destination.as_bytes());
                    reply.extend_from_slice(b" undo=A2-snapshot");
                }
                AgenticAction::KillProcess { handle } => {
                    self.seen.push(ToolKind::KillProcess);
                    if self.denied {
                        reply.extend_from_slice(b"arthur: Denied: kill_process requires process ");
                        reply.extend_from_slice(handle.to_string().as_bytes());
                        reply.extend_from_slice(b" rechecked-after-confirm");
                    } else {
                        reply.extend_from_slice(b"arthur: kill_process: confirmed handle=");
                        reply.extend_from_slice(handle.to_string().as_bytes());
                    }
                }
                AgenticAction::SpawnProcess { inherit_handle, .. } => {
                    self.seen.push(ToolKind::SpawnProcess);
                    if inherit_handle != 0 || self.denied {
                        reply.extend_from_slice(
                            b"arthur: Denied: spawn_process requires inherit handle ",
                        );
                        reply.extend_from_slice(inherit_handle.to_string().as_bytes());
                        reply.extend_from_slice(b" rechecked-after-confirm");
                    } else {
                        reply.extend_from_slice(b"arthur: spawn_process: pid=7 handle=7007");
                    }
                }
                AgenticAction::ExternalSend { bytes, .. } => {
                    self.seen.push(ToolKind::NetSend);
                    if self.denied {
                        reply.extend_from_slice(
                            b"arthur: Denied: net_send requires tcp 8008 rechecked-after-confirm",
                        );
                    } else {
                        reply.extend_from_slice(b"arthur: external_send: bytes=");
                        reply.extend_from_slice(bytes.to_string().as_bytes());
                        reply.extend_from_slice(b" policy=per-send typed-confirm");
                    }
                }
                AgenticAction::LoadModel { .. } => {
                    self.seen.push(ToolKind::LoadModel);
                    reply.extend_from_slice(b"arthur: load_model: handle=9009");
                }
                AgenticAction::Clarify { .. }
                | AgenticAction::Restricted { .. }
                | AgenticAction::Confirm
                | AgenticAction::DryRun { .. }
                | AgenticAction::ConfirmTyped { .. }
                | AgenticAction::Cancel
                | AgenticAction::Refuse { .. }
                | AgenticAction::Done { .. }
                | AgenticAction::NoOp => {}
            }
            Ok(())
        }
    }

    struct ConfirmRequiredActions {
        probes: usize,
        confirmed_writes: usize,
    }

    impl AgenticActionExecutor for ConfirmRequiredActions {
        fn available_agentic_actions(&self) -> &[AgenticActionKind] {
            &[
                AgenticActionKind::ReadFile,
                AgenticActionKind::WriteFile,
                AgenticActionKind::ListDir,
                AgenticActionKind::ChangeDir,
                AgenticActionKind::Status,
            ]
        }

        fn execute_agentic_action(
            &mut self,
            action: AgenticAction<'_>,
            reply: &mut Vec<u8>,
        ) -> Result<(), ConversationError> {
            match action {
                AgenticAction::WriteFile { path, .. } => {
                    self.probes += 1;
                    reply.extend_from_slice(b"arthur: confirm required: write_file ");
                    reply.extend_from_slice(path.as_bytes());
                    reply.extend_from_slice(b" reason=key-file-not-backed-up");
                }
                AgenticAction::Undo => {
                    reply.extend_from_slice(b"arthur: undo: no snapshot");
                }
                _ => reply.extend_from_slice(b"arthur: action ok"),
            }
            Ok(())
        }

        fn execute_confirmed_agentic_action(
            &mut self,
            action: AgenticAction<'_>,
            reply: &mut Vec<u8>,
        ) -> Result<(), ConversationError> {
            match action {
                AgenticAction::WriteFile { path, content } => {
                    self.confirmed_writes += 1;
                    reply.extend_from_slice(b"arthur: write_file: bytes=");
                    reply.extend_from_slice(content.len().to_string().as_bytes());
                    reply.extend_from_slice(b" undo=unavailable confirmed=destructive path=");
                    reply.extend_from_slice(path.as_bytes());
                    Ok(())
                }
                other => self.execute_agentic_action(other, reply),
            }
        }
    }

    struct MultiStepActions {
        deny_read_after: Option<usize>,
        reads: usize,
        write_probes: usize,
        confirmed_writes: usize,
    }

    impl AgenticActionExecutor for MultiStepActions {
        fn available_agentic_actions(&self) -> &[AgenticActionKind] {
            &[
                AgenticActionKind::ReadFile,
                AgenticActionKind::WriteFile,
                AgenticActionKind::ListDir,
                AgenticActionKind::ChangeDir,
                AgenticActionKind::Status,
            ]
        }

        fn execute_agentic_action(
            &mut self,
            action: AgenticAction<'_>,
            reply: &mut Vec<u8>,
        ) -> Result<(), ConversationError> {
            match action {
                AgenticAction::ReadFile { path } => {
                    self.reads += 1;
                    if self.deny_read_after.is_some_and(|limit| self.reads > limit) {
                        reply.extend_from_slice(b"arthur: Denied: read_file requires file ");
                        reply.extend_from_slice(path.as_bytes());
                    } else {
                        reply.extend_from_slice(b"arthur: read_file: NeoPlutarch scratch base");
                    }
                }
                AgenticAction::WriteFile { path, .. } => {
                    self.write_probes += 1;
                    reply.extend_from_slice(b"arthur: confirm required: write_file ");
                    reply.extend_from_slice(path.as_bytes());
                    reply.extend_from_slice(b" reason=stakes-policy");
                }
                AgenticAction::Undo => {
                    reply.extend_from_slice(
                        b"arthur: undo: write_file restored bytes=24 path=/scratch",
                    );
                }
                _ => reply.extend_from_slice(b"arthur: action ok"),
            }
            Ok(())
        }

        fn execute_confirmed_agentic_action(
            &mut self,
            action: AgenticAction<'_>,
            reply: &mut Vec<u8>,
        ) -> Result<(), ConversationError> {
            match action {
                AgenticAction::WriteFile { path, content } => {
                    self.confirmed_writes += 1;
                    reply.extend_from_slice(b"arthur: write_file: bytes=");
                    reply.extend_from_slice(content.len().to_string().as_bytes());
                    reply.extend_from_slice(b" undo=prewrite.bin path=");
                    reply.extend_from_slice(path.as_bytes());
                    Ok(())
                }
                other => self.execute_agentic_action(other, reply),
            }
        }
    }

    #[cfg(all(not(feature = "arthur-act"), not(feature = "agentic-actions")))]
    #[test]
    fn dummy_echo_does_not_touch_model_backend() {
        let mut state = ConversationState::new(8);
        let mut backend = MockBackend::default();
        let mut reply = Vec::new();
        respond_to_turn(
            ReplyMode::DummyEcho,
            &mut state,
            &mut backend,
            b"hello arthur\n",
            &mut reply,
        )
        .unwrap();

        assert_eq!(reply, b"arthur: echo: hello arthur");
        assert!(state.context().is_empty());
        assert!(backend.calls.is_empty());
    }

    #[test]
    fn memory_record_format_kat_is_byte_exact() {
        let mut encoded = Vec::new();
        encode_record(
            MemoryRecord {
                kind: MemoryType::Fact,
                timestamp: 1_781_433_600,
                content: "LogansOS M1 persists",
            },
            &mut encoded,
        )
        .unwrap();

        assert_eq!(encoded, b"1:fact:1781433600:LogansOS M1 persists\n");
    }

    #[test]
    fn memory_parser_rejects_malformed_records() {
        assert_eq!(
            parse_record_line(b"2:fact:1781433600:x"),
            Err(MemoryError::BadVersion)
        );
        assert_eq!(
            parse_record_line(b"1:secret:1781433600:x"),
            Err(MemoryError::BadType)
        );
        assert_eq!(
            parse_record_line(b"1:fact:not-a-timestamp:x"),
            Err(MemoryError::BadTimestamp)
        );
        assert_eq!(
            parse_record_line(b"1:fact:1781433600"),
            Err(MemoryError::BadFieldCount)
        );
        assert_eq!(
            parse_record_line(b"1:fact:1781433600:"),
            Err(MemoryError::BadContent)
        );
        assert_eq!(
            parse_record_line(b"1:fact:1781433600:x\n"),
            Err(MemoryError::BadContent)
        );
        assert_eq!(
            parse_record_line(&[b'1', b':', b'f', b'a', b'c', b't', b':', b'1', b':', 0xff,]),
            Err(MemoryError::BadUtf8)
        );
    }

    #[test]
    fn memory_lossy_parser_skips_corrupt_lines_without_leaking() {
        let bytes = b"0:relationship:not-a-timestamp:SHOULD_NOT_LEAK\n1:relationship:1781433601:Alice knows Arthur\n";
        let mut records = Vec::new();

        let malformed = parse_records_lossy(bytes, &mut records);

        assert_eq!(malformed, 1);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, MemoryType::Relationship);
        assert_eq!(records[0].timestamp, 1_781_433_601);
        assert_eq!(records[0].content, "Alice knows Arthur");
        assert!(
            !records
                .iter()
                .any(|record| record.content.contains("SHOULD_NOT_LEAK"))
        );
    }

    #[test]
    fn memory_category_paths_are_fixed() {
        let cases = [
            (MemoryType::Fact, "/neofs/.arthur/memory/fact.mem"),
            (
                MemoryType::Preference,
                "/neofs/.arthur/memory/preference.mem",
            ),
            (MemoryType::Event, "/neofs/.arthur/memory/event.mem"),
            (
                MemoryType::Relationship,
                "/neofs/.arthur/memory/relationship.mem",
            ),
        ];

        let mut path = Vec::new();
        for (kind, expected) in cases {
            category_path(kind, &mut path);
            assert_eq!(core::str::from_utf8(&path).unwrap(), expected);
        }
    }

    #[test]
    fn memory_fail_safe_is_exhaustive() {
        assert_eq!(fail_safe_mode(Ok(())), MemoryMode::Durable);

        for failure in [
            StoreFailure::MissingDirectory,
            StoreFailure::MissingCategory,
            StoreFailure::OpenDenied,
            StoreFailure::ReadError,
            StoreFailure::WriteError,
            StoreFailure::NoSpace,
            StoreFailure::CorruptRecord,
            StoreFailure::InvalidCategory,
        ] {
            assert_eq!(fail_safe_mode(Err(failure)), MemoryMode::Ephemeral);
        }
    }

    #[cfg(all(not(feature = "arthur-act"), not(feature = "agentic-actions")))]
    #[test]
    fn model_mode_tokenizes_generates_decodes_and_records_context() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend::default();
        let mut reply = Vec::new();
        respond_to_turn(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            b"hello",
            &mut reply,
        )
        .unwrap();

        assert_eq!(backend.calls, ["tokenize", "generate", "decode"]);
        assert_eq!(reply, b"arthur: model: ok");
        assert!(state.context().ends_with(&[42, 43]));
    }

    #[test]
    fn context_is_a_sliding_userspace_token_window() {
        let mut state = ConversationState::new(5);
        state.push_tokens(&[1, 2, 3]);
        state.push_tokens(&[4, 5, 6]);
        assert_eq!(state.context(), &[2, 3, 4, 5, 6]);
    }

    #[cfg(all(not(feature = "arthur-act"), not(feature = "agentic-actions")))]
    #[test]
    fn model_generate_errors_are_structured() {
        let mut state = ConversationState::new(8);
        let mut backend = MockBackend {
            fail_generate: true,
            ..MockBackend::default()
        };
        let mut reply = Vec::new();
        let err = respond_to_turn(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            b"hello",
            &mut reply,
        )
        .unwrap_err();

        assert_eq!(err, ConversationError::Generate);
        assert!(reply.is_empty());
    }

    #[test]
    fn renderer_only_emits_cap_scoped_tool_defs() {
        let mut prompt = String::new();
        render_hermes_prompt(&[ToolKind::ReadFile], "read /hello.txt", &mut prompt);

        assert!(prompt.contains("<tool_call>"));
        assert!(prompt.contains("\"name\":\"read_file\""));
        assert!(!prompt.contains("\"name\":\"write_file\""));
        assert!(prompt.contains("read /hello.txt"));
    }

    #[test]
    fn parser_extracts_valid_hermes_tool_call() {
        let parsed = extract_tool_call(
            "ok<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/hello.txt\"}}</tool_call>",
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed, ToolCall::ReadFile { path: "/hello.txt" });
    }

    #[test]
    fn parser_treats_pure_text_as_no_action() {
        assert_eq!(extract_tool_call("arthur: model: hello").unwrap(), None);
    }

    #[test]
    fn parser_reports_malformed_tool_call_for_retry() {
        let err = extract_tool_call("<tool_call>{\"name\":\"read_file\"}</tool_call>").unwrap_err();
        assert_eq!(err, ToolParseError::MissingArguments);
    }

    #[test]
    fn parser_accepts_all_v1_action_shapes() {
        let cases = [
            (
                "{\"name\":\"read_file\",\"arguments\":{\"path\":\"/hello.txt\"}}",
                ToolKind::ReadFile,
            ),
            (
                "{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"x\"}}",
                ToolKind::WriteFile,
            ),
            (
                "{\"name\":\"list_dir\",\"arguments\":{\"path\":\"/\"}}",
                ToolKind::ListDir,
            ),
            (
                "{\"name\":\"spawn_process\",\"arguments\":{\"name\":\"worker\"}}",
                ToolKind::SpawnProcess,
            ),
            (
                "{\"name\":\"kill_process\",\"arguments\":{\"handle\":7}}",
                ToolKind::KillProcess,
            ),
            (
                "{\"name\":\"inspect_process\",\"arguments\":{\"handle\":7}}",
                ToolKind::InspectProcess,
            ),
            (
                "{\"name\":\"wait_process\",\"arguments\":{\"handle\":7}}",
                ToolKind::WaitProcess,
            ),
            (
                "{\"name\":\"net_dial\",\"arguments\":{\"host\":\"example.test\",\"port\":443}}",
                ToolKind::NetDial,
            ),
            (
                "{\"name\":\"net_send\",\"arguments\":{\"handle\":9,\"data\":\"hi\"}}",
                ToolKind::NetSend,
            ),
            (
                "{\"name\":\"net_recv\",\"arguments\":{\"handle\":9,\"max_bytes\":32}}",
                ToolKind::NetRecv,
            ),
            (
                "{\"name\":\"infer\",\"arguments\":{\"model_handle\":2,\"input\":\"hi\"}}",
                ToolKind::Infer,
            ),
            (
                "{\"name\":\"load_model\",\"arguments\":{\"path\":\"/neofs/model.neom\"}}",
                ToolKind::LoadModel,
            ),
        ];

        for (json, kind) in cases {
            let wrapped = format!("<tool_call>{json}</tool_call>");
            assert_eq!(extract_tool_call(&wrapped).unwrap().unwrap().kind(), kind);
        }
    }

    #[test]
    fn agentic_ast_byte_oracle_binds_read_action_encoding() {
        let ast = "ReadFile(\"/hello.txt\")";
        let parsed = parse_agentic_ast(ast).unwrap();

        assert_eq!(parsed, AgenticAction::ReadFile { path: "/hello.txt" });

        let mut rendered = String::new();
        render_agentic_ast(parsed, &mut rendered).unwrap();
        assert_eq!(rendered.as_bytes(), ast.as_bytes());
    }

    #[test]
    fn agentic_ast_byte_oracle_binds_write_action_encoding() {
        let ast = "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>";
        let parsed = parse_agentic_ast(ast).unwrap();

        assert_eq!(
            parsed,
            AgenticAction::WriteFile {
                path: "/scratch",
                content: "NeoPlutarch scratch A2ok"
            }
        );

        let mut rendered = String::new();
        render_agentic_ast(parsed, &mut rendered).unwrap();
        assert_eq!(rendered.as_bytes(), ast.as_bytes());
    }

    #[test]
    fn agentic_ast_accepts_a2_nav_mutate_confirm_undo_and_noop() {
        assert_eq!(
            parse_agentic_ast("ListDir(\"/\")").unwrap(),
            AgenticAction::ListDir { path: "/" }
        );
        assert_eq!(
            parse_agentic_ast("cd(\"/\")").unwrap(),
            AgenticAction::ChangeDir { path: "/" }
        );
        assert_eq!(parse_agentic_ast("status").unwrap(), AgenticAction::Status);
        assert_eq!(
            parse_agentic_ast("clarify(\"Which path should I read?\")").unwrap(),
            AgenticAction::Clarify {
                question: "Which path should I read?"
            }
        );
        assert_eq!(
            parse_agentic_ast("<tool_call>{\"name\":\"no_op\"}</tool_call>").unwrap(),
            AgenticAction::NoOp
        );
        assert_eq!(
            parse_agentic_ast("<tool_call>{\"name\":\"confirm\"}</tool_call>").unwrap(),
            AgenticAction::Confirm
        );
        assert_eq!(
            parse_agentic_ast("<tool_call>{\"name\":\"undo\"}</tool_call>").unwrap(),
            AgenticAction::Undo
        );
    }

    #[test]
    fn agentic_a3_ast_byte_oracles_bind_taxonomy_and_typed_confirm() {
        let trash_ast = "<tool_call>{\"name\":\"trash_file\",\"arguments\":{\"path\":\"/scratch\"}}</tool_call>";
        let parsed = parse_agentic_ast(trash_ast).unwrap();
        assert_eq!(parsed, AgenticAction::TrashFile { path: "/scratch" });
        assert_eq!(
            agentic_effect_tier(parsed),
            AgenticEffectTier::TrashRecoverable
        );
        let mut rendered = String::new();
        render_agentic_ast(parsed, &mut rendered).unwrap();
        assert_eq!(rendered, trash_ast);
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&rendered),
            Some(AGENTIC_FIXTURE_TRASH_SCRATCH_TOKEN)
        );

        let kill = AgenticAction::KillProcess { handle: 4242 };
        render_agentic_ast(kill, &mut rendered).unwrap();
        assert_eq!(
            parse_agentic_ast(&rendered).unwrap(),
            AgenticAction::KillProcess { handle: 4242 }
        );
        assert_eq!(
            agentic_effect_tier(kill),
            AgenticEffectTier::LocalDestructive
        );
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&rendered),
            Some(AGENTIC_FIXTURE_KILL_4242_TOKEN)
        );

        let send = AgenticAction::ExternalSend {
            host: "api.example.com",
            port: 443,
            bytes: 16,
        };
        render_agentic_ast(send, &mut rendered).unwrap();
        assert_eq!(parse_agentic_ast(&rendered).unwrap(), send);
        assert_eq!(agentic_effect_tier(send), AgenticEffectTier::ExternalSend);
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&rendered),
            Some(AGENTIC_FIXTURE_EXTERNAL_SEND_TOKEN)
        );

        let confirm = AgenticAction::ConfirmTyped {
            phrase: "confirm kill 4242",
        };
        render_agentic_ast(confirm, &mut rendered).unwrap();
        assert_eq!(parse_agentic_ast(&rendered).unwrap(), confirm);
        assert_eq!(agentic_effect_tier(confirm), AgenticEffectTier::ReadOnly);
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&rendered),
            Some(AGENTIC_FIXTURE_CONFIRM_KILL_4242_TOKEN)
        );

        let confirm_trash = AgenticAction::ConfirmTyped {
            phrase: "confirm trash /scratch",
        };
        render_agentic_ast(confirm_trash, &mut rendered).unwrap();
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&rendered),
            Some(AGENTIC_FIXTURE_CONFIRM_TRASH_TOKEN)
        );

        let dry_run = AgenticAction::DryRun {
            plan: "kill_process 4242",
        };
        render_agentic_ast(dry_run, &mut rendered).unwrap();
        assert_eq!(parse_agentic_ast(&rendered).unwrap(), dry_run);
        assert_eq!(agentic_effect_tier(dry_run), AgenticEffectTier::ReadOnly);

        let refuse = AgenticAction::Refuse {
            reason: "A3 refuses broad protected or supervising-session actions",
        };
        render_agentic_ast(refuse, &mut rendered).unwrap();
        assert_eq!(parse_agentic_ast(&rendered).unwrap(), refuse);
        assert_eq!(
            agentic_effect_tier(refuse),
            AgenticEffectTier::Unpreviewable
        );

        let done = AgenticAction::Done {
            summary: "multi-step complete",
        };
        render_agentic_ast(done, &mut rendered).unwrap();
        assert_eq!(parse_agentic_ast(&rendered).unwrap(), done);
        assert_eq!(agentic_effect_tier(done), AgenticEffectTier::ReadOnly);
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&rendered),
            Some(AGENTIC_FIXTURE_DONE_TOKEN)
        );
    }

    #[test]
    fn agentic_a3_off_preserves_a2_restricted_ast_byte_for_byte() {
        let a2 = [
            AgenticActionKind::ReadFile,
            AgenticActionKind::WriteFile,
            AgenticActionKind::ListDir,
        ];
        let a3 = [
            AgenticActionKind::ReadFile,
            AgenticActionKind::WriteFile,
            AgenticActionKind::ListDir,
            AgenticActionKind::TrashFile,
            AgenticActionKind::MoveFile,
            AgenticActionKind::KillProcess,
            AgenticActionKind::SpawnProcess,
            AgenticActionKind::ExternalSend,
            AgenticActionKind::LoadModel,
        ];

        let mut rendered = String::new();
        let a2_action = fallback_agentic_action("delete /scratch", &a2);
        render_agentic_ast(a2_action, &mut rendered).unwrap();
        assert_eq!(
            rendered,
            "clarify(\"A2 restricts delete move kill spawn and external actions\")"
        );

        assert_eq!(
            fallback_agentic_action("delete /scratch", &a3),
            AgenticAction::TrashFile { path: "/scratch" }
        );
        assert_eq!(
            fallback_agentic_action("move /scratch /trash/scratch", &a3),
            AgenticAction::MoveFile {
                path: "/scratch",
                destination: "/trash/scratch"
            }
        );
    }

    #[test]
    fn agentic_a1_kill_switch_never_emits_mutating_candidates() {
        let a1 = [
            AgenticActionKind::ReadFile,
            AgenticActionKind::ListDir,
            AgenticActionKind::ChangeDir,
            AgenticActionKind::Status,
        ];

        assert_eq!(
            fallback_agentic_action("write /scratch", &a1),
            AgenticAction::Clarify {
                question: "Write file is not available"
            }
        );

        let mut candidates = Vec::new();
        push_agentic_candidate_asts("write /scratch", &a1, &mut candidates).unwrap();
        assert!(
            candidates
                .iter()
                .all(|candidate| !candidate.contains("\"name\":\"write_file\"")),
            "A1 candidates must not include mutating write_file ASTs: {candidates:?}"
        );
    }

    #[test]
    fn agentic_ast_rejects_malformed_or_unsafe_outputs() {
        assert_eq!(
            parse_agentic_ast(
                "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/tmp/../secret\",\"content\":\"x\"}}</tool_call>"
            ),
            Err(AgenticAstError::InvalidPath)
        );
        assert_eq!(
            parse_agentic_ast("<tool_call>{\"name\":\"read_file\"}</tool_call>"),
            Err(AgenticAstError::UnknownProduction)
        );
        assert_eq!(
            parse_agentic_ast(
                "<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/tmp/../secret\"}}</tool_call>"
            ),
            Err(AgenticAstError::InvalidPath)
        );
        assert_eq!(
            parse_agentic_ast(
                "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"bad\\\"quote\"}}</tool_call>"
            ),
            Err(AgenticAstError::InvalidContent)
        );
    }

    #[test]
    fn agentic_candidates_cover_valid_productions_not_just_fallback() {
        let available = [
            AgenticActionKind::ReadFile,
            AgenticActionKind::WriteFile,
            AgenticActionKind::ListDir,
        ];
        assert_eq!(
            fallback_agentic_action("read the hello.txt file", &available),
            AgenticAction::ReadFile { path: "/hello.txt" }
        );
        assert_eq!(
            fallback_agentic_action("read the missing.txt file", &available),
            AgenticAction::ReadFile { path: "/hello.txt" }
        );

        let mut asts = Vec::new();
        push_agentic_candidate_asts("read the missing.txt file", &available, &mut asts).unwrap();

        let mut hello_ast = String::new();
        render_agentic_ast(
            AgenticAction::ReadFile { path: "/hello.txt" },
            &mut hello_ast,
        )
        .unwrap();
        let mut missing_ast = String::new();
        render_agentic_ast(
            AgenticAction::ReadFile {
                path: "/missing.txt",
            },
            &mut missing_ast,
        )
        .unwrap();
        assert!(asts.contains(&hello_ast));
        assert!(asts.contains(&missing_ast));
        let mut write_ast = String::new();
        render_agentic_ast(
            AgenticAction::WriteFile {
                path: "/scratch",
                content: "NeoPlutarch scratch A2ok",
            },
            &mut write_ast,
        )
        .unwrap();
        assert!(asts.contains(&write_ast));
        assert!(asts.len() > 1);
    }

    #[test]
    fn agentic_fixture_oracle_tokens_bind_distinct_read_asts() {
        let mut hello_ast = String::new();
        render_agentic_ast(
            AgenticAction::ReadFile { path: "/hello.txt" },
            &mut hello_ast,
        )
        .unwrap();
        let mut missing_ast = String::new();
        render_agentic_ast(
            AgenticAction::ReadFile {
                path: "/missing.txt",
            },
            &mut missing_ast,
        )
        .unwrap();

        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&hello_ast),
            Some(AGENTIC_FIXTURE_READ_HELLO_TOKEN)
        );
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&missing_ast),
            Some(AGENTIC_FIXTURE_READ_MISSING_TOKEN)
        );
        assert_ne!(
            agentic_fixture_oracle_token_for_ast(&hello_ast),
            agentic_fixture_oracle_token_for_ast(&missing_ast)
        );
    }

    #[test]
    fn agentic_fixture_oracle_tokens_bind_a2_asts() {
        let mut write_ast = String::new();
        render_agentic_ast(
            AgenticAction::WriteFile {
                path: "/scratch",
                content: "NeoPlutarch scratch A2ok",
            },
            &mut write_ast,
        )
        .unwrap();
        let mut danger_key_ast = String::new();
        render_agentic_ast(
            AgenticAction::WriteFile {
                path: "/neofs/danger.key",
                content: "NeoPlutarch scratch A2ok",
            },
            &mut danger_key_ast,
        )
        .unwrap();
        let mut confirm_ast = String::new();
        render_agentic_ast(AgenticAction::Confirm, &mut confirm_ast).unwrap();
        let mut undo_ast = String::new();
        render_agentic_ast(AgenticAction::Undo, &mut undo_ast).unwrap();

        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&write_ast),
            Some(AGENTIC_FIXTURE_WRITE_SCRATCH_TOKEN)
        );
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&danger_key_ast),
            Some(AGENTIC_FIXTURE_WRITE_DANGER_KEY_TOKEN)
        );
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&confirm_ast),
            Some(AGENTIC_FIXTURE_CONFIRM_TOKEN)
        );
        assert_eq!(
            agentic_fixture_oracle_token_for_ast(&undo_ast),
            Some(AGENTIC_FIXTURE_UNDO_TOKEN)
        );
    }

    #[test]
    fn agentic_action_path_executes_a1_read_and_list() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/hello.txt\"}}</tool_call>",
                b"<tool_call>{\"name\":\"list_dir\",\"arguments\":{\"path\":\"/\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"read /hello.txt",
            &mut reply,
        )
        .unwrap();
        assert_eq!(reply, b"arthur: read_file: /hello.txt");

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"list /",
            &mut reply,
        )
        .unwrap();

        assert_eq!(actions.seen, [ToolKind::ReadFile, ToolKind::ListDir]);
        assert_eq!(reply, b"arthur: list_dir: /");
    }

    #[test]
    fn agentic_action_path_surfaces_cap_denial_for_ungranted_read() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/missing.txt\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: true,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"read /missing.txt",
            &mut reply,
        )
        .unwrap();

        assert_eq!(actions.seen, [ToolKind::ReadFile]);
        assert_eq!(
            reply,
            b"arthur: Denied: read_file requires file /missing.txt"
        );
    }

    #[test]
    fn agentic_action_path_confirms_before_mutating() {
        let mut state = ConversationState::new(16);
        state.set_posture(AgenticPosture::Cautious); // pins the pre-yolo A2 flow
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"clarify\",\"arguments\":{\"question\":\"Which path should I read?\"}}</tool_call>",
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
                b"<tool_call>{\"name\":\"undo\"}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"what should happen?",
            &mut reply,
        )
        .unwrap();
        assert_eq!(reply, b"arthur: clarify: Which path should I read?");

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"write /scratch",
            &mut reply,
        )
        .unwrap();

        assert!(actions.seen.is_empty());
        assert_eq!(
            reply,
            b"arthur: confirm required: write_file /scratch bytes=24 reversible=snapshot-before-write"
        );

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        )
        .unwrap();
        assert_eq!(actions.seen, [ToolKind::WriteFile]);
        assert_eq!(
            reply,
            b"arthur: write_file: bytes=24 undo=snapshot path=/scratch"
        );

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"undo now",
            &mut reply,
        )
        .unwrap();
        assert_eq!(actions.seen, [ToolKind::WriteFile, ToolKind::WriteFile]);
        assert_eq!(
            reply,
            b"arthur: undo: write_file restored bytes=24 path=/scratch"
        );
    }

    #[test]
    fn agentic_action_path_denies_confirmed_ungranted_write() {
        let mut state = ConversationState::new(16);
        state.set_posture(AgenticPosture::Cautious); // pins the pre-yolo A2 flow
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: true,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"write /scratch",
            &mut reply,
        )
        .unwrap();
        assert!(actions.seen.is_empty());

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        )
        .unwrap();

        assert_eq!(actions.seen, [ToolKind::WriteFile]);
        assert_eq!(reply, b"arthur: Denied: write_file requires file /scratch");
    }

    #[test]
    fn agentic_a3_typed_confirm_rejects_wrong_and_stale_before_execution() {
        let mut state = ConversationState::new(16);
        state.set_posture(AgenticPosture::Cautious); // pins the pre-yolo A3 flow
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"trash_file\",\"arguments\":{\"path\":\"/scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm wrong phrase\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm trash /scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"trash_file\",\"arguments\":{\"path\":\"/scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"kill_process\",\"arguments\":{\"handle\":4242}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm trash /scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm kill 4242\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"delete /scratch",
            &mut reply,
        )
        .unwrap();
        let text = std::str::from_utf8(&reply).unwrap();
        assert!(text.contains("arthur: dry-run required: tier=T"));
        assert!(text.contains("phrase=confirm trash /scratch"));
        assert!(!text.contains("posture=yolo")); // cautious previews carry no yolo marker
        assert!(actions.seen.is_empty());

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm typed wrong",
            &mut reply,
        )
        .unwrap();
        assert_eq!(reply, b"arthur: typed-confirm: wrong phrase no-op");
        assert!(actions.seen.is_empty());

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm typed trash",
            &mut reply,
        )
        .unwrap();
        assert_eq!(
            reply,
            b"arthur: trash_file: moved_to_trash path=/scratch undo=A2-snapshot"
        );
        assert_eq!(actions.seen, [ToolKind::WriteFile]);

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"delete /scratch",
            &mut reply,
        )
        .unwrap();
        assert!(
            std::str::from_utf8(&reply)
                .unwrap()
                .contains("phrase=confirm trash /scratch")
        );

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"kill 4242",
            &mut reply,
        )
        .unwrap();
        assert!(
            std::str::from_utf8(&reply)
                .unwrap()
                .contains("phrase=confirm kill 4242")
        );

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm stale trash",
            &mut reply,
        )
        .unwrap();
        assert_eq!(reply, b"arthur: typed-confirm: stale-preview no-op");
        assert_eq!(actions.seen, [ToolKind::WriteFile]);

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm kill",
            &mut reply,
        )
        .unwrap();
        assert_eq!(reply, b"arthur: kill_process: confirmed handle=4242");
        assert_eq!(actions.seen, [ToolKind::WriteFile, ToolKind::KillProcess]);
    }

    #[test]
    fn agentic_a3_untyped_confirm_is_noop_for_external_send() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"external_send\",\"arguments\":{\"host\":\"api.example.com\",\"port\":443,\"bytes\":16}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm send api.example.com:443\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"send external",
            &mut reply,
        )
        .unwrap();
        let text = std::str::from_utf8(&reply).unwrap();
        assert!(text.contains("tier=X"));
        assert!(text.contains("policy=per-send"));
        assert!(actions.seen.is_empty());

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        )
        .unwrap();
        assert_eq!(reply, b"arthur: confirm: typed confirmation required no-op");
        assert!(actions.seen.is_empty());

        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm typed send",
            &mut reply,
        )
        .unwrap();
        assert_eq!(
            reply,
            b"arthur: external_send: bytes=16 policy=per-send typed-confirm"
        );
        assert_eq!(actions.seen, [ToolKind::NetSend]);
    }

    #[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
    #[test]
    fn action_path_executes_one_tool_call_and_reports_result() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![b"<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/hello.txt\"}}</tool_call>"],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"read /hello.txt",
            &mut reply,
        )
        .unwrap();

        assert_eq!(actions.seen, [ToolKind::ReadFile]);
        assert_eq!(reply, b"arthur: read_file: /hello.txt");
        assert!(!backend.calls.contains(&"saw_write_tool"));
    }

    #[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
    #[test]
    fn action_path_reports_denied_from_disposition_layer() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"x\"}}</tool_call>"],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: true,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"write /scratch",
            &mut reply,
        )
        .unwrap();

        assert_eq!(actions.seen, [ToolKind::WriteFile]);
        assert_eq!(reply, b"arthur: Denied: write_file requires file /scratch");
    }

    #[cfg(all(feature = "arthur-act", not(feature = "agentic-actions")))]
    #[test]
    fn malformed_tool_call_retries_then_executes_valid_call() {
        let mut state = ConversationState::new(32);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"read_file\"}</tool_call>",
                b"<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/hello.txt\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        respond_to_turn_with_actions(
            ReplyMode::Model,
            &mut state,
            &mut backend,
            &mut actions,
            b"read /hello.txt",
            &mut reply,
        )
        .unwrap();

        assert_eq!(backend.decode_idx, 2);
        assert_eq!(actions.seen, [ToolKind::ReadFile]);
        assert_eq!(reply, b"arthur: read_file: /hello.txt");
    }

    // === Yolo-within-guardrails posture (the ISC-5 safety-contract matrix) ===
    //
    // The posture is LAYER-1 (confirmation friction) only. Every row below
    // pins one cell of the failure matrix:
    //   yolo_reversible_granted   = executes_silently (undo armed)
    //   yolo_missing_cap          = DENIED (the 29.3 floor with confirm skipped)
    //   yolo_irreversible_local   = still_gated (L keeps typed-confirm)
    //   yolo_irreversible_external= still_gated (X keeps per-send typed-confirm)
    //   yolo_wrong_reversible     = undo_recovers
    //   posture_cautious          = a3_equiv (byte-for-byte)
    //   posture_off               = inert (no decode, no execute)

    fn drive_agentic<A: AgenticActionExecutor>(
        state: &mut ConversationState,
        backend: &mut MockBackend,
        actions: &mut A,
        input: &[u8],
        reply: &mut Vec<u8>,
    ) {
        respond_to_turn_with_agentic_actions(
            ReplyMode::Model,
            state,
            backend,
            actions,
            input,
            reply,
        )
        .unwrap();
    }

    #[test]
    fn agentic_multi_step_pauses_for_per_step_stakes_confirm_and_resumes() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"ReadFile(\"/scratch\")",
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
                b"ReadFile(\"/scratch\")",
                b"<tool_call>{\"name\":\"done\",\"arguments\":{\"summary\":\"multi-step complete\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MultiStepActions {
            deny_read_after: None,
            reads: 0,
            write_probes: 0,
            confirmed_writes: 0,
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"multi clean scratch",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: multi-step paused step=2 reason=stakes-policy ledger=1:read_file undo=no,2:write_file pending-confirm chain_undo=session per_step_confirm=ok"
        );
        assert_eq!(actions.reads, 1);
        assert_eq!(actions.write_probes, 1);
        assert_eq!(actions.confirmed_writes, 0);

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: multi-step done steps=3 budget=4 ledger=1:read_file undo=no,2:write_file undo=session,3:read_file undo=no chain_undo=session cap_check=per_step multi_step=ok"
        );
        assert_eq!(actions.reads, 2);
        assert_eq!(actions.confirmed_writes, 1);
    }

    #[cfg(feature = "arthur-reversibility-stakes")]
    #[test]
    fn agentic_a3_stakes_undo_is_deterministic_not_model_decoded() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![b"not a valid undo ast"],
            ..MockBackend::default()
        };
        let mut actions = MultiStepActions {
            deny_read_after: None,
            reads: 0,
            write_probes: 0,
            confirmed_writes: 0,
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"undo now",
            &mut reply,
        );

        assert_eq!(
            reply,
            b"arthur: undo: write_file restored bytes=24 path=/scratch"
        );
        assert!(backend.calls.is_empty());
    }

    #[test]
    fn agentic_multi_step_budget_stops_with_honest_ledger() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"ReadFile(\"/scratch\")",
                b"ReadFile(\"/scratch\")",
                b"ReadFile(\"/scratch\")",
                b"ReadFile(\"/scratch\")",
            ],
            ..MockBackend::default()
        };
        let mut actions = MultiStepActions {
            deny_read_after: None,
            reads: 0,
            write_probes: 0,
            confirmed_writes: 0,
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"multi runaway",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: multi-step stopped reason=budget steps=4 budget=4 ledger=1:read_file undo=no,2:read_file undo=no,3:read_file undo=no,4:read_file undo=no"
        );
        assert_eq!(actions.reads, 4);
    }

    #[test]
    fn agentic_multi_step_partial_failure_reports_completed_prefix() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![b"ReadFile(\"/scratch\")", b"ReadFile(\"/scratch\")"],
            ..MockBackend::default()
        };
        let mut actions = MultiStepActions {
            deny_read_after: Some(1),
            reads: 0,
            write_probes: 0,
            confirmed_writes: 0,
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"multi runaway",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: multi-step stopped step=2 action=read_file reason=denied ledger=1:read_file undo=no chain_undo=session"
        );
        assert_eq!(actions.reads, 2);
    }

    #[test]
    fn agentic_multi_step_runtime_off_is_inert() {
        let mut state = ConversationState::new(16);
        state.set_posture(AgenticPosture::Off);
        let mut backend = MockBackend {
            decoded: vec![b"ReadFile(\"/scratch\")"],
            ..MockBackend::default()
        };
        let mut actions = MultiStepActions {
            deny_read_after: None,
            reads: 0,
            write_probes: 0,
            confirmed_writes: 0,
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"multi clean scratch",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: posture: off - agentic actions disabled (/posture yolo to enable)"
        );
        assert!(backend.calls.is_empty());
        assert_eq!(actions.reads, 0);
        assert_eq!(actions.write_probes, 0);
    }

    #[test]
    fn posture_default_is_yolo_and_query_reports_it() {
        let mut state = ConversationState::new(16);
        assert_eq!(state.posture(), AgenticPosture::Yolo);
        let mut backend = MockBackend::default();
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"/posture",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: posture: yolo");
        // The query is a deterministic intercept: the model was never invoked.
        assert!(backend.calls.is_empty());
        assert!(actions.seen.is_empty());
    }

    #[test]
    fn yolo_reversible_write_executes_silently_with_undo_armed() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
                b"<tool_call>{\"name\":\"undo\"}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        // The write executes on the spot — one executor call, no pending.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"write /scratch",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: write_file: bytes=24 undo=snapshot path=/scratch posture=yolo".as_slice()
        );
        assert_eq!(actions.seen, [ToolKind::WriteFile]);

        // No pending mutation was staged: a follow-up confirm is a no-op.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: confirm: no pending action");
        assert_eq!(actions.seen, [ToolKind::WriteFile]);

        // The undo net stays armed after a yolo write (wrong-but-reversible
        // recovers).
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"undo now",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: undo: write_file restored bytes=24 path=/scratch"
        );
    }

    #[test]
    fn yolo_missing_cap_is_denied_by_the_unchanged_cap_check() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: true,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        // ISC-2 (Article 29.3): with ALL confirmation skipped, the action
        // still reaches the cap-check and is DENIED — a hijacked yolo-Arthur
        // cannot exceed its held caps. The denial is loud, never silent.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"write /scratch",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: Denied: write_file requires file /scratch posture=yolo".as_slice()
        );
        assert_eq!(actions.seen, [ToolKind::WriteFile]);
    }

    #[test]
    fn yolo_undo_unavailable_stages_confirm_before_destructive_write() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/neofs/danger.key\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = ConfirmRequiredActions {
            probes: 0,
            confirmed_writes: 0,
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"write /neofs/danger.key",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: confirm required: write_file /neofs/danger.key reason=key-file-not-backed-up posture=yolo"
                .as_slice()
        );
        assert_eq!(actions.probes, 1);
        assert_eq!(actions.confirmed_writes, 0);

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: write_file: bytes=24 undo=unavailable confirmed=destructive path=/neofs/danger.key"
                .as_slice()
        );
        assert_eq!(actions.probes, 1);
        assert_eq!(actions.confirmed_writes, 1);
    }

    #[test]
    fn yolo_trash_executes_silently_and_undo_recovers() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"trash_file\",\"arguments\":{\"path\":\"/scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"undo\"}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        // T-tier trash flows without a dry-run under yolo (recoverable), with
        // the A2 snapshot armed.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"delete /scratch",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: trash_file: moved_to_trash path=/scratch undo=A2-snapshot posture=yolo"
                .as_slice()
        );
        assert_eq!(actions.seen, [ToolKind::WriteFile]);

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"undo now",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: undo: write_file restored bytes=24 path=/scratch"
        );
    }

    #[test]
    fn yolo_irreversible_local_still_requires_typed_confirm() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"kill_process\",\"arguments\":{\"handle\":4242}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm kill 4242\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        // Kill is irreversible-LOCAL (L tier): yolo drops friction only for
        // reversible actions, so the dry-run + typed confirm stays.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"kill 4242",
            &mut reply,
        );
        let text = std::str::from_utf8(&reply).unwrap();
        assert!(text.contains("arthur: dry-run required: tier=L"));
        assert!(text.contains("phrase=confirm kill 4242 posture=yolo"));
        assert!(actions.seen.is_empty());

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm kill",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: kill_process: confirmed handle=4242");
        assert_eq!(actions.seen, [ToolKind::KillProcess]);
    }

    #[test]
    fn yolo_external_send_still_gated_per_send() {
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"external_send\",\"arguments\":{\"host\":\"api.example.com\",\"port\":443,\"bytes\":16}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm send api.example.com:443\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        // X tier: an external send NEVER flows silently, yolo or not.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"send external",
            &mut reply,
        );
        let text = std::str::from_utf8(&reply).unwrap();
        assert!(text.contains("arthur: dry-run required: tier=X"));
        assert!(text.contains(" posture=yolo"));
        assert!(actions.seen.is_empty());

        // An untyped confirm is not send authority.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: confirm: typed confirmation required no-op");
        assert!(actions.seen.is_empty());

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm send",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: external_send: bytes=16 policy=per-send typed-confirm"
        );
        assert_eq!(actions.seen, [ToolKind::NetSend]);
    }

    #[test]
    fn posture_cautious_equals_pre_yolo_a3_byte_for_byte() {
        // The cautious corpus: every action class, with the expected bytes
        // copied literally from the pre-yolo A2/A3 tests — cautious IS
        // today's shipped behavior, bit-for-bit.
        let mut state = ConversationState::new(16);
        state.set_posture(AgenticPosture::Cautious);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm\"}</tool_call>",
                b"<tool_call>{\"name\":\"trash_file\",\"arguments\":{\"path\":\"/scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm trash /scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"/hello.txt\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"write /scratch",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: confirm required: write_file /scratch bytes=24 reversible=snapshot-before-write"
                .as_slice()
        );
        assert!(actions.seen.is_empty());

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: write_file: bytes=24 undo=snapshot path=/scratch"
        );
        assert_eq!(actions.seen, [ToolKind::WriteFile]);

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"delete /scratch",
            &mut reply,
        );
        let text = std::str::from_utf8(&reply).unwrap();
        assert!(text.contains("arthur: dry-run required: tier=T"));
        assert!(text.contains("phrase=confirm trash /scratch"));

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm trash",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: trash_file: moved_to_trash path=/scratch undo=A2-snapshot".as_slice()
        );

        // Read-only actions execute directly in every posture.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"read /hello.txt",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: read_file: /hello.txt");
    }

    #[test]
    fn posture_off_is_inert_no_decode_no_execute() {
        let mut state = ConversationState::new(16);
        state.set_posture(AgenticPosture::Off);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"/scratch\",\"content\":\"NeoPlutarch scratch A2ok\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        for input in [
            b"write /scratch".as_slice(),
            b"delete /scratch",
            b"kill 4242",
        ] {
            drive_agentic(&mut state, &mut backend, &mut actions, input, &mut reply);
            assert_eq!(
                reply,
                b"arthur: posture: off - agentic actions disabled (/posture yolo to enable)"
                    .as_slice()
            );
        }
        // Structural inertness: the model decoder and the executor were never
        // invoked (the property the pre-agentic layer had by construction).
        assert!(backend.calls.is_empty());
        assert!(actions.seen.is_empty());

        // Recovery: switching back re-enables the layer.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"/posture yolo",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: posture: set yolo");
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"write /scratch",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: write_file: bytes=24 undo=snapshot path=/scratch posture=yolo".as_slice()
        );
        assert_eq!(actions.seen, [ToolKind::WriteFile]);
    }

    #[test]
    fn posture_is_not_model_emittable_and_bad_args_never_reach_the_model() {
        // No posture production exists in the constrained grammar: an AST
        // that tries to name a posture tool is an unknown production.
        assert_eq!(
            parse_agentic_ast(
                "<tool_call>{\"name\":\"posture\",\"arguments\":{\"mode\":\"yolo\"}}</tool_call>"
            ),
            Err(AgenticAstError::UnknownProduction)
        );

        // A malformed /posture line yields usage deterministically — it never
        // falls through to the model as a prompt.
        let mut state = ConversationState::new(16);
        let mut backend = MockBackend::default();
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"/posture bogus",
            &mut reply,
        );
        assert_eq!(
            reply,
            b"arthur: posture: usage /posture [yolo|cautious|off]"
        );
        assert!(backend.calls.is_empty());
        assert_eq!(state.posture(), AgenticPosture::Yolo);
    }

    #[test]
    fn posture_switch_stales_pending_typed_confirm() {
        let mut state = ConversationState::new(16);
        state.set_posture(AgenticPosture::Cautious);
        let mut backend = MockBackend {
            decoded: vec![
                b"<tool_call>{\"name\":\"trash_file\",\"arguments\":{\"path\":\"/scratch\"}}</tool_call>",
                b"<tool_call>{\"name\":\"confirm_typed\",\"arguments\":{\"phrase\":\"confirm trash /scratch\"}}</tool_call>",
            ],
            ..MockBackend::default()
        };
        let mut actions = MockActions {
            tools: &[ToolKind::ReadFile],
            denied: false,
            seen: Vec::new(),
        };
        let mut reply = Vec::new();

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"delete /scratch",
            &mut reply,
        );
        assert!(
            std::str::from_utf8(&reply)
                .unwrap()
                .contains("phrase=confirm trash /scratch")
        );

        // The posture switch invalidates the staged preview: the previously
        // correct phrase is now stale and must not execute.
        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"/posture yolo",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: posture: set yolo");

        drive_agentic(
            &mut state,
            &mut backend,
            &mut actions,
            b"confirm trash",
            &mut reply,
        );
        assert_eq!(reply, b"arthur: typed-confirm: stale-preview no-op");
        assert!(actions.seen.is_empty());
    }
}

/// B4 fix — a *direct-function* byte-oracle that binds M2's OWN
/// `encrypt_plaintext` / `decrypt_file` / `nonce_for`, not the raw crate.
///
/// The replaced KAT (in `neo-arthur/src/arthur_lib.rs`, with `[lib] test=false`)
/// (a) never ran in the gate and (b) hand-rolled the format with the raw
/// `chacha20poly1305` crate while asserting a nonce layout (`[4..12]`) that M2's
/// real `nonce_for` (`to_le_bytes()[..12]`) does not use. These KATs run under
/// the standard `cargo test --workspace` and would fail if M2's wrapper drifted.
#[cfg(test)]
mod m2_crypto_kats {
    use std::vec::Vec;

    use chacha20poly1305::aead::{AeadInPlace, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Nonce};

    use super::memory_crypto::{
        CategoryKey, ENCRYPTED_FORMAT_VERSION, MemoryEncryptError, NONCE_LEN, TAG_LEN,
        decrypt_file, encrypt_plaintext, extract_nonce_from_file,
    };

    /// Independently compute the expected M2 blob with the RAW crate, laying the
    /// nonce out the way `nonce_for` is *documented* to (low 12 bytes of the
    /// counter, little-endian). This is the oracle — NOT produced by M2's
    /// `encrypt_plaintext`, so a divergence in M2's wrapper makes the bytes differ.
    fn oracle_blob(key: &[u8; 32], counter: u128, pt: &[u8]) -> Vec<u8> {
        let nonce_bytes: [u8; 12] = counter.to_le_bytes()[..12].try_into().unwrap();
        let cipher = ChaCha20Poly1305::new_from_slice(key).unwrap();
        let mut ct = pt.to_vec();
        let tag = cipher
            .encrypt_in_place_detached(Nonce::from_slice(&nonce_bytes), b"", &mut ct)
            .unwrap();
        let mut blob = Vec::new();
        blob.push(0x01u8); // version
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ct);
        blob.extend_from_slice(tag.as_slice());
        blob
    }

    #[test]
    fn encrypt_plaintext_matches_byte_oracle() {
        let key = [0x42u8; 32];
        let counter = 0x0102_0304_0506_0708_u128; // arbitrary, < 2^96
        let pt = b"fact: the cat is on the mat\n";

        let ck = CategoryKey::from_raw(key, counter);
        let mut got = Vec::new();
        encrypt_plaintext(&ck, counter, pt, &mut got).expect("encrypt");

        assert_eq!(
            got,
            oracle_blob(&key, counter, pt),
            "encrypt_plaintext must match the independent byte oracle"
        );
        // Structural pins, independent of the oracle helper.
        assert_eq!(got[0], ENCRYPTED_FORMAT_VERSION);
        assert_eq!(&got[1..1 + NONCE_LEN], &counter.to_le_bytes()[..NONCE_LEN]);
        assert_eq!(got.len(), 1 + NONCE_LEN + pt.len() + TAG_LEN);
    }

    #[test]
    fn decrypt_file_round_trips_real_blob() {
        let key = [0x99u8; 32];
        let counter = 7u128;
        let pt = b"preference: dark mode\n";
        let ck = CategoryKey::from_raw(key, counter);

        let mut blob = Vec::new();
        encrypt_plaintext(&ck, counter, pt, &mut blob).expect("encrypt");

        let mut out = Vec::new();
        decrypt_file(&blob, &ck, &mut out).expect("decrypt");
        assert_eq!(out.as_slice(), pt);
    }

    #[test]
    fn decrypt_file_rejects_tamper_and_clears_out() {
        let key = [0x11u8; 32];
        let counter = 3u128;
        let pt = b"event: booted\n";
        let ck = CategoryKey::from_raw(key, counter);

        let mut blob = Vec::new();
        encrypt_plaintext(&ck, counter, pt, &mut blob).expect("encrypt");
        blob[13] ^= 0xFF; // flip a ciphertext byte just after the 13-byte header

        let mut out = Vec::new();
        let res = decrypt_file(&blob, &ck, &mut out);
        assert_eq!(res, Err(MemoryEncryptError::AuthFail));
        assert!(
            out.is_empty(),
            "unverified plaintext must never be surfaced"
        );
    }

    #[test]
    fn decrypt_file_rejects_wrong_key() {
        let counter = 5u128;
        let pt = b"relationship: alice\n";
        let ck = CategoryKey::from_raw([0x01u8; 32], counter);
        let wrong = CategoryKey::from_raw([0x02u8; 32], counter);

        let mut blob = Vec::new();
        encrypt_plaintext(&ck, counter, pt, &mut blob).expect("encrypt");

        let mut out = Vec::new();
        assert_eq!(
            decrypt_file(&blob, &wrong, &mut out),
            Err(MemoryEncryptError::AuthFail)
        );
        assert!(out.is_empty());
    }

    /// `nonce_for` layout pin: low 12 LE bytes of the counter, AND byte-identical
    /// to the on-disk header `extract_nonce_from_file` reads back. Catches the
    /// exact `[4..12]` confusion the deleted KAT shipped.
    #[test]
    fn nonce_for_is_low_12_le_and_matches_header() {
        let ck = CategoryKey::from_raw([0u8; 32], 0);
        let counter = 0x0011_2233_4455_6677_8899_u128;

        let expected: [u8; 12] = counter.to_le_bytes()[..12].try_into().unwrap();
        assert_eq!(ck.nonce_for(counter), expected);

        let mut blob = Vec::new();
        encrypt_plaintext(&ck, counter, b"x", &mut blob).unwrap();
        assert_eq!(extract_nonce_from_file(&blob), Some(counter));
    }

    /// Distinct counters under one key -> distinct nonces AND distinct blobs (the
    /// whole reason the write-ahead counter exists: never reuse a nonce).
    #[test]
    fn distinct_counters_distinct_blobs() {
        let key = [0x7Au8; 32];
        let pt = b"same plaintext\n";
        let ck = CategoryKey::from_raw(key, 0);

        let mut blob_a = Vec::new();
        encrypt_plaintext(&ck, 1, pt, &mut blob_a).unwrap();
        let mut blob_b = Vec::new();
        encrypt_plaintext(&ck, 2, pt, &mut blob_b).unwrap();

        assert_ne!(blob_a, blob_b);
        assert_ne!(
            extract_nonce_from_file(&blob_a),
            extract_nonce_from_file(&blob_b)
        );
    }
}

#[cfg(test)]
mod ot19_reversibility_kats {
    use std::vec::Vec;

    use super::memory_crypto::{CategoryKey, MemoryEncryptError, decrypt_file, encrypt_plaintext};
    use super::reversibility_logic::{
        artifact_hash_dir, path_hash, path_is_key_file, prewrite_artifact_path,
        reversible_backup_allowed, stakes_policy_requires_confirm,
    };

    #[test]
    fn prewrite_artifact_path_is_stable_for_scratch() {
        assert_eq!(path_hash("/scratch"), 0xcd2d_d734_db24_ea74);
        assert_eq!(artifact_hash_dir("/scratch"), "cd2dd734db24ea74");
        assert_eq!(
            prewrite_artifact_path("/scratch"),
            "/neofs/.arthur/undo/actions/cd2dd734db24ea74/prewrite.bin"
        );
    }

    #[test]
    fn key_files_are_never_backup_candidates() {
        assert!(path_is_key_file("/neofs/.arthur/memory/fact.key"));
        assert!(!reversible_backup_allowed("/neofs/.arthur/memory/fact.key"));
        assert!(reversible_backup_allowed("/scratch"));
        assert!(reversible_backup_allowed("/neofs/.arthur/memory/fact.mem"));
    }

    #[test]
    fn stakes_policy_is_mechanical_not_model_judged() {
        let policy = r#"{
            "confirm_destructive": true,
            "paths": ["/scratch", "/important"],
            "write_file": ["/also-write-gated"]
        }"#;
        assert!(stakes_policy_requires_confirm(
            policy,
            "write_file",
            "/scratch"
        ));
        assert!(stakes_policy_requires_confirm(
            policy,
            "write_file",
            "/also-write-gated"
        ));
        assert!(!stakes_policy_requires_confirm(
            policy,
            "write_file",
            "/casual"
        ));
        assert!(!stakes_policy_requires_confirm(
            r#"{"paths":["/scratch"]}"#,
            "write_file",
            "/scratch"
        ));
    }

    #[test]
    fn undo_artifact_auth_failure_is_detectable() {
        let ck = CategoryKey::from_raw([0x5a; 32], 9);
        let mut artifact = Vec::new();
        encrypt_plaintext(&ck, 9, b"NeoPlutarch scratch base", &mut artifact).unwrap();
        let last = artifact.len() - 1;
        artifact[last] ^= 0x80;

        let mut out = Vec::new();
        assert_eq!(
            decrypt_file(&artifact, &ck, &mut out),
            Err(MemoryEncryptError::AuthFail)
        );
        assert!(out.is_empty());
    }
}

/// B1c (F8): every row of the vocab x model composition, and the guarantee
/// that a bad vocab never runs the model load.
#[cfg(test)]
mod vocab_compose_tests {
    use super::model_state_logic::{
        EBADMSG, ENOENT, ModelState, VocabLoad, compose, vocab_load_for_error,
    };
    use std::cell::Cell;

    #[test]
    fn open_errors_map_to_vocab_outcomes() {
        assert_eq!(vocab_load_for_error(ENOENT), VocabLoad::Absent);
        assert_eq!(vocab_load_for_error(EBADMSG), VocabLoad::Damaged);
        assert_eq!(vocab_load_for_error(-5), VocabLoad::Unreadable(-5));
    }

    #[test]
    fn a_bad_vocab_never_takes_the_model() {
        for (vocab, expected) in [
            (VocabLoad::Absent, ModelState::Absent),
            (VocabLoad::Damaged, ModelState::Damaged),
            (VocabLoad::Unreadable(-5), ModelState::LoadFailed),
            (VocabLoad::Unparseable, ModelState::LoadFailed),
        ] {
            let calls = Cell::new(0);
            let state = compose(vocab, || {
                calls.set(calls.get() + 1);
                Ok(())
            });
            assert_eq!(state, expected, "{vocab:?}");
            assert_eq!(calls.get(), 0, "{vocab:?} must not take the model");
        }
    }

    #[test]
    fn a_loaded_vocab_reports_the_model_outcome() {
        for (result, expected) in [
            (Ok(()), ModelState::Ready),
            (Err(ENOENT), ModelState::Absent),
            (Err(EBADMSG), ModelState::Damaged),
            (Err(-12), ModelState::LoadFailed),
        ] {
            let calls = Cell::new(0);
            let state = compose(VocabLoad::Loaded, || {
                calls.set(calls.get() + 1);
                result
            });
            assert_eq!(state, expected, "{result:?}");
            assert_eq!(calls.get(), 1);
        }
    }
}
