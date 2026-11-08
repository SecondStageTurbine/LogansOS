// SPDX-License-Identifier: MPL-2.0
//! The fixed eval set: the prompts Arthur is actually given.
//!
//! Sources (all exact typed text):
//! - `hello world`: the 21-token boot prompt (`QwenTemplate`, `arthur_real_brain_boot`).
//! - `hello desktop`, `again`, `open probe app`, `list /`, `help`,
//!   `what time is it?`, `what's the date?`, `what time is it in tokyo?`:
//!   `tools/xtask/src/main.rs` desktop scenarios (`DESKTOP_*_HID_STEPS`).
//! - `hi`: `login_arthur_permission` (`tools/xtask/src/login_scenarios.rs`).
//! - `hello arthur`, `/model 0.5b`, `read /hello.txt`, `write /scratch`,
//!   `/posture`, `write /scratch!`, `read /scratch`, `undo now`,
//!   `write /forbidden`, `delete /scratch #`, `kill 4242 @`,
//!   `send external ?`, `write /neofs/danger.key k`, `confirm`,
//!   `/posture off`, `/posture yolo`, `multi clean scratch`,
//!   `multi runaway`: `tools/xtask/src/main.rs` Arthur console/agentic
//!   scenarios (`ARTHUR_*_SERIAL_STEPS`).
//! - `what's 2+2?`, `what time is it`: J.c.'s preview questions
//!   (`Plans/FEEDBACK-2026-11-02-preview.md`).
//!
//! Each is a user turn framed by `QwenTemplate` (the chat template Arthur's
//! boot/desktop path uses: atomic `<|im_start|>`, system "You are a helpful
//! assistant.", `<|im_end|>`), so all three models receive identical token IDs.

#[derive(Clone, Copy, Debug)]
pub struct Prompt {
    pub id: &'static str,
    pub source: &'static str,
    pub text: &'static str,
}

pub const PROMPTS: &[Prompt] = &[
    Prompt {
        id: "boot_hello_world",
        source: "arthur_real_brain_boot (#1)",
        text: "hello world",
    },
    Prompt {
        id: "desktop_session",
        source: "desktop_session_boot",
        text: "hello desktop",
    },
    Prompt {
        id: "desktop_again",
        source: "desktop_home_boot",
        text: "again",
    },
    Prompt {
        id: "desktop_apps",
        source: "desktop_apps_boot",
        text: "open probe app",
    },
    Prompt {
        id: "desktop_list",
        source: "desktop_lens_boot",
        text: "list /",
    },
    Prompt {
        id: "desktop_help",
        source: "desktop_help_boot",
        text: "help",
    },
    Prompt {
        id: "desktop_time",
        source: "desktop_clock_boot",
        text: "what time is it?",
    },
    Prompt {
        id: "desktop_date",
        source: "desktop_clock_boot",
        text: "what's the date?",
    },
    Prompt {
        id: "desktop_tokyo",
        source: "desktop_clock_boot",
        text: "what time is it in tokyo?",
    },
    Prompt {
        id: "login_hi",
        source: "login_arthur_permission",
        text: "hi",
    },
    Prompt {
        id: "preview_calc",
        source: "J.c. preview F8",
        text: "what's 2+2?",
    },
    Prompt {
        id: "preview_time",
        source: "J.c. preview F8",
        text: "what time is it",
    },
    Prompt {
        id: "console_hello",
        source: "arthur_converse",
        text: "hello arthur",
    },
    Prompt {
        id: "console_model",
        source: "arthur_boots_thinking",
        text: "/model 0.5b",
    },
    Prompt {
        id: "act_read",
        source: "arthur_act",
        text: "read /hello.txt",
    },
    Prompt {
        id: "act_write",
        source: "arthur_act",
        text: "write /scratch",
    },
    Prompt {
        id: "posture",
        source: "arthur_agentic_actions",
        text: "/posture",
    },
    Prompt {
        id: "write_reversible",
        source: "arthur_agentic_actions",
        text: "write /scratch!",
    },
    Prompt {
        id: "read_scratch",
        source: "arthur_agentic_actions",
        text: "read /scratch",
    },
    Prompt {
        id: "undo",
        source: "arthur_agentic_actions",
        text: "undo now",
    },
    Prompt {
        id: "write_forbidden",
        source: "arthur_agentic_actions",
        text: "write /forbidden",
    },
    Prompt {
        id: "delete",
        source: "arthur_agentic_actions",
        text: "delete /scratch #",
    },
    Prompt {
        id: "kill",
        source: "arthur_agentic_actions",
        text: "kill 4242 @",
    },
    Prompt {
        id: "send_external",
        source: "arthur_agentic_actions",
        text: "send external ?",
    },
    Prompt {
        id: "write_danger",
        source: "arthur_reversibility",
        text: "write /neofs/danger.key k",
    },
    Prompt {
        id: "confirm",
        source: "arthur_reversibility",
        text: "confirm",
    },
    Prompt {
        id: "posture_off",
        source: "arthur_reversibility",
        text: "/posture off",
    },
    Prompt {
        id: "posture_yolo",
        source: "arthur_reversibility",
        text: "/posture yolo",
    },
    Prompt {
        id: "multi_clean",
        source: "arthur_multistep_agentic",
        text: "multi clean scratch",
    },
    Prompt {
        id: "multi_runaway",
        source: "arthur_multistep_agentic",
        text: "multi runaway",
    },
];

/// WikiText-2 (raw) test source, pinned before the run.
pub const CORPUS_SOURCE: &str = "https://huggingface.co/datasets/Salesforce/wikitext wikitext-2-raw-v1/test-00000-of-00001.parquet";
pub const CORPUS_REVISION: &str = "b08601e04326c79dfdd32d625aee71d232d685c3";
pub const CORPUS_PARQUET_SHA256: &str =
    "5f1bea067869d04849c0f975a2b29c4ff47d867f484f5010ea5e861eab246d91";
/// SHA-256 of the extracted `test.txt` text (the parquet's `text` column joined).
pub const CORPUS_TEXT_SHA256: &str =
    "bbf94c53a05abe9ee670d3b6343608095822c85e26de37c70b24fc571964574a";
/// Tokens used: the first 8,192 tokens of the test split.
pub const CORPUS_TOKEN_START: usize = 0;
pub const CORPUS_TOKENS: usize = 8192;
/// NLL is summed over positions `eval_start .. len-1` predicting
/// `eval_start+1 .. len`: 4,095 targets, the same last-half window
/// `llama-perplexity` evaluates at `-c 8192`.
pub const CORPUS_EVAL_START: usize = 4096;
