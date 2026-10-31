// SPDX-License-Identifier: MPL-2.0
//! Vocab-sidecar parser and real-tokenizer holder for the `arthur-real-vocab` feature.
//!
//! Reads the L1-emitted `.vocab` file from NeoFS at Arthur startup and produces
//! owned tables backed by `Box<[u8]>`. The parser matches L1's byte format exactly
//! (see `neomc/src/gguf_import.rs::write_tokenizer_export`).
//!
//! Format:
//! ```text
//! NEOM_TOKENIZER\t1\n
//! bos\t{id}\n          (optional)
//! eos\t{id}\n          (optional)
//! pad\t{id}\n          (optional)
//! tokens\t{count}\n
//! {id}\t{hex_bytes}\n  (repeated `count` times, id = 0..count-1)
//! merges\t{count}\n
//! {rank}\t{hex_bytes}\n (repeated `count` times, rank = 0..count-1; bytes are "left right")
//! ```

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::tokenizer_logic::{
    BpeMergeEntry, BpeTables, BpeTokenizer, BpeVocabEntry, TokenizerError,
};
use crate::vocab_bin_logic::{ConvertError, VocabBin, VocabBinError};

/// The tokenizer's tables: built from the text sidecar, or the prebuilt
/// `NEOVOCB1` file used in place (`arthur-vocab-binary`). Both run
/// `tokenizer_logic`'s one encode/decode.
pub enum VocabTokenizer {
    Text(BpeTokenizer),
    Binary(VocabBin<Vec<u8>>),
}

impl VocabTokenizer {
    pub fn encode(&self, input: &str) -> Result<Vec<u32>, TokenizerError> {
        match self {
            Self::Text(t) => t.encode(input),
            Self::Binary(b) => b.encode(input),
        }
    }

    pub fn decode(&self, ids: &[u32]) -> Result<String, TokenizerError> {
        match self {
            Self::Text(t) => t.decode(ids),
            Self::Binary(b) => b.decode(ids),
        }
    }
}

/// Owned real-vocab state: tokenizer + special-token IDs.
/// Built once at startup from the sidecar file; immutable thereafter.
pub struct RealVocab {
    pub tokenizer: VocabTokenizer,
    #[allow(dead_code)]
    pub token_count: usize,
    // OT-37: the Qwen2.5 chat template starts at <|im_start|> and adds no BOS, so
    // bos is parsed for completeness but not used by the boot prompt path. `allow`
    // (not `expect`): these are dead in the kernel/lib build but READ by this
    // module's own unit tests when it is host-mounted into a test crate, so the
    // expectation would be unfulfilled there.
    #[allow(dead_code)]
    pub bos_token_id: Option<u32>,
    pub eos_token_id: Option<u32>,
    #[allow(dead_code)]
    pub pad_token_id: Option<u32>,
    /// Control-token IDs, sorted: every vocab entry spelled `<|...|>` (Qwen's
    /// `<|im_end|>`, `<|endoftext|>`, ...) plus the sidecar's EOS. A reply ends
    /// at the first of these and never renders it. Unread under
    /// `agentic-actions`, whose replies are not free text.
    #[allow(dead_code)]
    control_ids: Vec<u32>,
}

impl RealVocab {
    /// The part of `tokens` to render as reply text: everything before the
    /// first control token. The kernel's generate loop returns the stop token
    /// it ended on (`<|im_end|>`), so the reply is cut at its ID.
    #[allow(dead_code)]
    pub fn reply_tokens<'a>(&self, tokens: &'a [u32]) -> &'a [u32] {
        let end = tokens
            .iter()
            .position(|id| self.control_ids.binary_search(id).is_ok())
            .unwrap_or(tokens.len());
        &tokens[..end]
    }

    /// The model was stopped by the `max_new` cap, not by a stop token: the
    /// reply is not the whole answer.
    #[allow(dead_code)]
    pub fn reply_was_cut(&self, tokens: &[u32], max_new: usize) -> bool {
        tokens.len() >= max_new && self.reply_tokens(tokens).len() == tokens.len()
    }
}

/// Appended to a reply that reached the new-token cap without a stop token.
/// ASCII: Arthur's surface draws only printable ASCII. Unread under
/// `agentic-actions`, whose replies are not free text.
#[allow(dead_code)]
pub const REPLY_CUT_MARKER: &str = " [answer cut short]";

/// A control token's spelling: `<|`, a non-empty name, `|>`.
fn is_control_spelling(token: &[u8]) -> bool {
    token.len() > 4 && token.starts_with(b"<|") && token.ends_with(b"|>")
}

/// Parsed but not yet boxed — intermediate result before building the tokenizer.
struct RawSidecar {
    vocab_tokens: Vec<(u32, Box<[u8]>)>,
    #[allow(clippy::type_complexity)]
    merge_pairs: Vec<(Box<[u8]>, Box<[u8]>, u32)>,
    bos_token_id: Option<u32>,
    eos_token_id: Option<u32>,
    pad_token_id: Option<u32>,
}

/// Try to parse a sidecar byte buffer and build a `RealVocab`.
///
/// Returns `Some(RealVocab)` on success, `None` if the buffer is not a valid
/// sidecar or the tokenizer fails to build (caller falls back to fixture).
pub fn try_build_real_vocab(bytes: &[u8]) -> Option<RealVocab> {
    let raw = parse_raw_sidecar(bytes)?;
    let vocab_entries: Vec<BpeVocabEntry<'_>> = raw
        .vocab_tokens
        .iter()
        .map(|(id, token)| BpeVocabEntry {
            token: token.as_ref(),
            id: *id,
        })
        .collect();

    let merge_entries: Vec<BpeMergeEntry<'_>> = raw
        .merge_pairs
        .iter()
        .map(|(left, right, rank)| BpeMergeEntry {
            left: left.as_ref(),
            right: right.as_ref(),
            rank: *rank,
        })
        .collect();

    let tokenizer = BpeTokenizer::from_slices(&vocab_entries, &merge_entries).ok()?;

    let control_ids = control_ids(
        raw.vocab_tokens
            .iter()
            .map(|(id, token)| (*id, token.as_ref())),
        raw.eos_token_id,
    );

    Some(RealVocab {
        tokenizer: VocabTokenizer::Text(tokenizer),
        token_count: raw.vocab_tokens.len(),
        bos_token_id: raw.bos_token_id,
        eos_token_id: raw.eos_token_id,
        pad_token_id: raw.pad_token_id,
        control_ids,
    })
}

/// The ids spelled `<|...|>`, plus the EOS: sorted and unique.
fn control_ids<'a>(tokens: impl Iterator<Item = (u32, &'a [u8])>, eos: Option<u32>) -> Vec<u32> {
    let mut ids: Vec<u32> = tokens
        .filter(|(_, token)| is_control_spelling(token))
        .map(|(id, _)| id)
        .chain(eos)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// `arthur-vocab-binary`: take a `NEOVOCB1` file and use it in place. The
/// one pass is `VocabBin::new`'s validation; there is no parse.
#[allow(dead_code)] // the text-vocab Arthur builds do not call it.
pub fn try_build_binary_vocab(bytes: Vec<u8>) -> Result<RealVocab, VocabBinError> {
    let bin = VocabBin::new(bytes)?;
    let control_ids = control_ids(
        (0..bin.token_count()).filter_map(|id| bin.token_bytes(id).map(|t| (id, t))),
        bin.eos(),
    );
    Ok(RealVocab {
        token_count: bin.token_count() as usize,
        bos_token_id: bin.bos(),
        eos_token_id: bin.eos(),
        pad_token_id: bin.pad(),
        control_ids,
        tokenizer: VocabTokenizer::Binary(bin),
    })
}

/// The image-build converter (`xtask`): the text sidecar to `NEOVOCB1`,
/// refusing a source that breaks the format's preconditions (dense ids, every
/// merge's parts and result tokens; see `vocab_bin_logic::write`).
#[allow(dead_code)] // xtask and the host tests call it; Arthur does not.
pub fn convert_text_to_binary(text: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let raw = parse_raw_sidecar(text).ok_or(ConvertError::Unparseable)?;
    let tokens: Vec<(u32, &[u8])> = raw
        .vocab_tokens
        .iter()
        .map(|(id, t)| (*id, t.as_ref()))
        .collect();
    let merges: Vec<(&[u8], &[u8], u32)> = raw
        .merge_pairs
        .iter()
        .map(|(l, r, rank)| (l.as_ref(), r.as_ref(), *rank))
        .collect();
    crate::vocab_bin_logic::write(
        &tokens,
        &merges,
        raw.bos_token_id,
        raw.eos_token_id,
        raw.pad_token_id,
    )
}

fn parse_raw_sidecar(bytes: &[u8]) -> Option<RawSidecar> {
    let text = core::str::from_utf8(bytes).ok()?;

    let mut lines = text.lines();

    // Magic line: "NEOM_TOKENIZER\t1"
    let magic = lines.next()?;
    if magic != "NEOM_TOKENIZER\t1" {
        return None;
    }

    let mut bos_id: Option<u32> = None;
    let mut eos_id: Option<u32> = None;
    let mut pad_id: Option<u32> = None;

    // Scan for special-token IDs until we hit the `tokens\t` line.
    let mut token_count: Option<usize> = None;
    for line in lines.by_ref() {
        if let Some(count_str) = line.strip_prefix("tokens\t") {
            token_count = parse_u32(count_str).map(|n| n as usize);
            break;
        } else if let Some(id_str) = line.strip_prefix("bos\t") {
            bos_id = parse_u32(id_str);
        } else if let Some(id_str) = line.strip_prefix("eos\t") {
            eos_id = parse_u32(id_str);
        } else if let Some(id_str) = line.strip_prefix("pad\t") {
            pad_id = parse_u32(id_str);
        } else if line.starts_with("merges\t") {
            // Hit merges before tokens — malformed.
            return None;
        }
    }

    let token_count = token_count?;
    let mut vocab_tokens = Vec::with_capacity(token_count);
    for _ in 0..token_count {
        let line = lines.next()?;
        let (id_str, hex) = line.split_once('\t')?;
        let id = parse_u32(id_str)?;
        let token_bytes = hex_decode(hex)?;
        vocab_tokens.push((id, token_bytes.into_boxed_slice()));
    }

    // Read merges count.
    let merges_line = lines.next()?;
    let merges_count_str = merges_line.strip_prefix("merges\t")?;
    let merges_count = parse_u32(merges_count_str)? as usize;

    let mut merge_pairs = Vec::with_capacity(merges_count);
    for rank in 0..merges_count {
        let line = lines.next()?;
        let (_rank_str, hex) = line.split_once('\t')?;
        let merge_bytes = hex_decode(hex)?;
        // Merge bytes encode "left right" (space-separated raw token bytes).
        let space_pos = merge_bytes.iter().position(|&b| b == b' ')?;
        let left: Box<[u8]> = merge_bytes[..space_pos].into();
        let right: Box<[u8]> = merge_bytes[space_pos + 1..].into();
        if left.is_empty() || right.is_empty() {
            return None;
        }
        merge_pairs.push((left, right, rank as u32));
    }

    Some(RawSidecar {
        vocab_tokens,
        merge_pairs,
        bos_token_id: bos_id,
        eos_token_id: eos_id,
        pad_token_id: pad_id,
    })
}

fn parse_u32(s: &str) -> Option<u32> {
    s.trim().parse().ok()
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_digit(bytes[i])?;
        let lo = hex_digit(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Qwen2.5 chat-template control tokens. These are ATOMIC tokens in the real
/// vocab — confirmed from the `.vocab` sidecar (raw source):
///   151644 -> 3c7c696d5f73746172747c3e = `<|im_start|>`
///   151645 -> 3c7c696d5f656e647c3e     = `<|im_end|>`
/// The boot's byte-level BPE encoder does NOT recognize the literal
/// `<|im_start|>` string as a single token (it splits it into byte pieces), so
/// the template MUST emit these IDs directly. Emitting them as text (the OT-37
/// bug) produced a prompt with no atomic control tokens, so the model never saw
/// a well-formed assistant turn and replied with immediate EOS.
pub const QWEN_IM_START_ID: u32 = 151644;
pub const QWEN_IM_END_ID: u32 = 151645;
/// Default system turn — keeps the model inside its instruction-tuned
/// distribution so it answers as an assistant rather than free-completing.
const QWEN_SYSTEM_PROMPT: &str = "You are a helpful assistant.";

pub struct QwenTemplate {
    im_start_id: u32,
    im_end_id: u32,
}

impl QwenTemplate {
    pub const fn new() -> Self {
        Self {
            im_start_id: QWEN_IM_START_ID,
            im_end_id: QWEN_IM_END_ID,
        }
    }

    /// Frame a single user turn as Qwen2.5 chat-template TOKEN IDs:
    /// ```text
    /// <|im_start|>system\n{SYSTEM}<|im_end|>\n
    /// <|im_start|>user\n{user}<|im_end|>\n
    /// <|im_start|>assistant\n
    /// ```
    /// Control tokens are emitted as their atomic vocab IDs; text segments are
    /// BPE-encoded via `encode` (each segment encoded whole so BPE merges match
    /// the reference tokenization). No BOS: the Qwen chat template begins at
    /// `<|im_start|>`. Returns `None` if any segment fails to encode.
    pub fn frame_turn_ids<F>(
        &self,
        user_input: &str,
        encode: F,
        out: &mut alloc::vec::Vec<u32>,
    ) -> Option<()>
    where
        F: Fn(&str) -> Option<alloc::vec::Vec<u32>>,
    {
        use alloc::string::String;

        out.push(self.im_start_id);
        let mut system = String::from("system\n");
        system.push_str(QWEN_SYSTEM_PROMPT);
        out.extend_from_slice(&encode(&system)?);
        out.push(self.im_end_id);
        out.extend_from_slice(&encode("\n")?);

        out.push(self.im_start_id);
        let mut user = String::from("user\n");
        user.push_str(user_input);
        out.extend_from_slice(&encode(&user)?);
        out.push(self.im_end_id);
        out.extend_from_slice(&encode("\n")?);

        out.push(self.im_start_id);
        out.extend_from_slice(&encode("assistant\n")?);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;

    /// Byte-oracle: known Qwen token <-> id pairs through the parser + BpeTokenizer.
    ///
    /// These ids are representative of the real Qwen2.5 vocab structure. The sidecar
    /// fixture below is a minimal subset that exercises the parser path end-to-end.
    #[test]
    fn byte_oracle_parser_round_trip_known_tokens() {
        let sidecar_text = build_test_sidecar();

        let real_vocab = try_build_real_vocab(sidecar_text.as_bytes()).expect("build real vocab");

        assert_eq!(real_vocab.bos_token_id, Some(151644));
        assert_eq!(real_vocab.eos_token_id, Some(151644));
        assert_eq!(real_vocab.pad_token_id, Some(151644));

        // Encode "hi" should produce token id 4 (the merged form).
        let encoded = real_vocab.tokenizer.encode("hi").expect("encode hi");
        assert_eq!(encoded, vec![4]);

        // Decode token 4 should produce "hi".
        let decoded = real_vocab.tokenizer.decode(&[4]).expect("decode 4");
        assert_eq!(decoded, "hi");

        // Encode "h i" (h + space + i) — the space is a separate token.
        let encoded_space = real_vocab.tokenizer.encode("h i").expect("encode h i");
        assert_eq!(encoded_space, vec![2, 1, 3]);

        // Decode round-trip: encode -> decode == original.
        let roundtrip = real_vocab
            .tokenizer
            .decode(&encoded_space)
            .expect("roundtrip");
        assert_eq!(roundtrip, "h i");
    }

    #[test]
    fn parser_rejects_bad_magic() {
        let bad = "BAD_MAGIC\t1\ntokens\t0\nmerges\t0\n";
        assert!(try_build_real_vocab(bad.as_bytes()).is_none());
    }

    #[test]
    fn parser_rejects_odd_hex() {
        let sidecar = [
            "NEOM_TOKENIZER\t1",
            "tokens\t1",
            "0\tabc", // odd-length hex
            "merges\t0",
        ]
        .join("\n")
            + "\n";
        assert!(try_build_real_vocab(sidecar.as_bytes()).is_none());
    }

    /// Byte-oracle for the OT-37 chat template: the framed token sequence is
    /// reconstructed INDEPENDENTLY here (not via `frame_turn_ids`) and asserted
    /// equal, so a writer bug can't agree with itself. The mock `encode` maps each
    /// UTF-8 byte to its value as a token id, which makes the control-token
    /// PLACEMENT verifiable without the real 151936-entry vocab. The key
    /// properties: the atomic `<|im_start|>`/`<|im_end|>` IDs appear (never
    /// byte-split text), three im_start + two im_end, and the user text is present.
    #[test]
    fn qwen_template_frames_turn_ids_with_atomic_control_tokens() {
        let template = QwenTemplate::new();
        let encode = |s: &str| Some(s.bytes().map(u32::from).collect::<alloc::vec::Vec<u32>>());

        let mut out = alloc::vec::Vec::new();
        template
            .frame_turn_ids("hello world", encode, &mut out)
            .expect("frame");

        // Independent reconstruction of the expected sequence.
        let mut expected = alloc::vec::Vec::new();
        let push_bytes = |s: &str, e: &mut alloc::vec::Vec<u32>| e.extend(s.bytes().map(u32::from));
        expected.push(QWEN_IM_START_ID);
        push_bytes("system\nYou are a helpful assistant.", &mut expected);
        expected.push(QWEN_IM_END_ID);
        push_bytes("\n", &mut expected);
        expected.push(QWEN_IM_START_ID);
        push_bytes("user\nhello world", &mut expected);
        expected.push(QWEN_IM_END_ID);
        push_bytes("\n", &mut expected);
        expected.push(QWEN_IM_START_ID);
        push_bytes("assistant\n", &mut expected);

        assert_eq!(out, expected);
        assert_eq!(out[0], QWEN_IM_START_ID);
        assert_eq!(
            out.iter().filter(|&&t| t == QWEN_IM_START_ID).count(),
            3,
            "three <|im_start|> control tokens (system, user, assistant)"
        );
        assert_eq!(
            out.iter().filter(|&&t| t == QWEN_IM_END_ID).count(),
            2,
            "two <|im_end|> control tokens (system, user)"
        );
    }

    /// A sidecar with Qwen2.5's three chat control tokens at their real IDs,
    /// spelled in the hand-laid hex of the real `.vocab` (151643
    /// `<|endoftext|>`, 151644 `<|im_start|>`, 151645 `<|im_end|>`), EOS =
    /// `<|im_end|>`, and two near-misses that are ordinary text.
    fn build_control_sidecar() -> String {
        let mut s = String::new();
        s.push_str("NEOM_TOKENIZER\t1\n");
        s.push_str("eos\t151645\n");
        s.push_str("tokens\t9\n");
        s.push_str(&format!("1\t{}\n", hex_encode(&[0xC4, 0xA0])));
        s.push_str(&format!("2\t{}\n", hex_encode(b"h")));
        s.push_str(&format!("3\t{}\n", hex_encode(b"i")));
        s.push_str(&format!("4\t{}\n", hex_encode(b"hi")));
        // `<|>` and `<||>`: no name between the bars, so not control tokens.
        s.push_str("5\t3c7c3e\n");
        s.push_str("6\t3c7c7c3e\n");
        s.push_str("151643\t3c7c656e646f66746578747c3e\n");
        s.push_str("151644\t3c7c696d5f73746172747c3e\n");
        s.push_str("151645\t3c7c696d5f656e647c3e\n");
        s.push_str("merges\t1\n");
        s.push_str(&format!("0\t{}\n", hex_encode(b"h i")));
        s
    }

    /// Zenbook acceptance: the reply rendered "Paris.<|im_end|>". The kernel's
    /// generate loop returns the stop token it ended on; the reply is cut at
    /// its ID, so the text never carries it.
    #[test]
    fn reply_ends_at_im_end_without_rendering_it() {
        let vocab = try_build_real_vocab(build_control_sidecar().as_bytes()).expect("vocab");
        let generated = [4, 1, 2, QWEN_IM_END_ID];

        // The defect being fixed: decoding the whole stream renders the token.
        let whole = vocab.tokenizer.decode(&generated).expect("decode whole");
        assert_eq!(whole, "hi h<|im_end|>");

        assert_eq!(vocab.reply_tokens(&generated), &[4, 1, 2]);
        let reply = vocab
            .tokenizer
            .decode(vocab.reply_tokens(&generated))
            .expect("decode reply");
        assert_eq!(reply, "hi h");
    }

    #[test]
    fn reply_ends_at_any_control_token() {
        let vocab = try_build_real_vocab(build_control_sidecar().as_bytes()).expect("vocab");
        // `<|endoftext|>` is not the EOS here; its spelling makes it control.
        assert_eq!(vocab.reply_tokens(&[4, 151643, 2]), &[4]);
        assert_eq!(vocab.reply_tokens(&[3, QWEN_IM_START_ID, 4]), &[3]);
        // Only a control token: an empty reply.
        assert_eq!(vocab.reply_tokens(&[QWEN_IM_END_ID]), &[] as &[u32]);
        // No control token (a max-length reply): unchanged.
        assert_eq!(vocab.reply_tokens(&[4, 1, 4]), &[4, 1, 4]);
        // `<|>` and `<||>` are text.
        assert_eq!(vocab.reply_tokens(&[5, 6, 4]), &[5, 6, 4]);
    }

    /// A reply that reached the cap without a stop token is marked; one that
    /// ended on `<|im_end|>`, before or at the cap, is whole.
    #[test]
    fn reply_cut_only_at_the_cap_without_a_stop_token() {
        let vocab = try_build_real_vocab(build_control_sidecar().as_bytes()).expect("vocab");
        // At the cap, no stop token: cut.
        assert!(vocab.reply_was_cut(&[4, 1, 2, 3], 4));
        // Ended on <|im_end|> before the cap: whole.
        assert!(!vocab.reply_was_cut(&[4, 1, QWEN_IM_END_ID], 4));
        // The cap's last token was the stop token: whole.
        assert!(!vocab.reply_was_cut(&[4, 1, 2, QWEN_IM_END_ID], 4));
        // Short of the cap: the model chose to stop.
        assert!(!vocab.reply_was_cut(&[4, 1], 4));
        assert_eq!(REPLY_CUT_MARKER, " [answer cut short]");
        assert!(REPLY_CUT_MARKER.bytes().all(|b| (0x20..=0x7e).contains(&b)));
    }

    /// The sidecar's EOS ends the reply even when it is not spelled `<|...|>`.
    #[test]
    fn reply_ends_at_eos_by_id() {
        let vocab = try_build_real_vocab(build_test_sidecar().as_bytes()).expect("vocab");
        assert_eq!(vocab.eos_token_id, Some(151644));
        assert_eq!(vocab.reply_tokens(&[4, 151644, 2]), &[4]);
        assert_eq!(vocab.reply_tokens(&[4, 2]), &[4, 2]);
    }

    fn build_test_sidecar() -> String {
        let mut s = String::new();
        s.push_str("NEOM_TOKENIZER\t1\n");
        s.push_str("bos\t151644\n");
        s.push_str("eos\t151644\n");
        s.push_str("pad\t151644\n");
        s.push_str("tokens\t5\n");
        // id 0: padding token (newline convention)
        s.push_str(&format!("0\t{}\n", hex_encode(b"\n")));
        // id 1: space — stored in the GPT-2 byte-level-MAPPED form the encoder
        // looks up (space 0x20 -> `Ġ` = bytes C4 A0 via `bytelevel_encode`), not the
        // raw byte. (This fixture only began running once host-mounted into a test
        // crate; the raw-0x20 form silently never matched the mapped encoder input.)
        s.push_str(&format!("1\t{}\n", hex_encode(&[0xC4, 0xA0])));
        // id 2: "h"
        s.push_str(&format!("2\t{}\n", hex_encode(b"h")));
        // id 3: "i"
        s.push_str(&format!("3\t{}\n", hex_encode(b"i")));
        // id 4: "hi" (merged token)
        s.push_str(&format!("4\t{}\n", hex_encode(b"hi")));
        s.push_str("merges\t1\n");
        // rank 0: merge "h" + "i" -> "hi"
        s.push_str(&format!("0\t{}\n", hex_encode(b"h i")));
        s
    }

    fn hex_encode(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }
}
