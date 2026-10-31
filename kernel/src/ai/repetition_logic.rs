// SPDX-License-Identifier: MPL-2.0
//! A9-B: keep greedy decoding out of degenerate loops, deterministically.
//!
//! Two independent rules, both pure (the kernel's generation core calls them,
//! `tools/ai-ops-tests` mounts this file):
//!
//! - **Repetition penalty** over the last `window` generated tokens: each
//!   distinct token there has its logit pushed down, `l / p` when `l > 0` and
//!   `l * p` otherwise (the rule of Keskar et al. 2019, CTRL §4.1, which the
//!   common inference stacks implement). `p == 1.0` returns before touching
//!   any logit, so every bit is kept, a signalling NaN's too (`NaN * 1.0`
//!   would quiet it: Jarvis, #97 RETURN item 1).
//! - **Loop stop**: the reply is cut only once it has repeated exactly, with
//!   some period `p`, for at least `ngram` tokens beyond the first copy of
//!   the repeating phrase: the last `ngram + p` tokens repeat with period
//!   `p`. The cut keeps one copy. So a phrase shorter than `ngram` said
//!   twice is never cut (its second copy is fewer than `ngram` tokens of
//!   repetition), while said often enough it is (an 8-token phrase three
//!   times: 24 tokens, cut to the first 8). A phrase of `ngram` tokens or
//!   more said twice is a loop by the same rule, and is cut to one copy. The
//!   contract is worded this way, rather than "a phrase shorter than `ngram`
//!   is never cut", because that rule would let a one-token loop run to 32
//!   tokens before the stop and keep 16 copies (PM call on Jarvis, #97
//!   RETURN item 2).
//!
//! Neither rule draws a random number, so the decode stays deterministic.

/// The penalty's window: how many of the most recent generated tokens it
/// considers.
pub const REPETITION_WINDOW: usize = 64;

/// The loop stop's run length, in tokens (about a dozen words of English).
pub const LOOP_NGRAM: usize = 16;

/// The penalty the product decode applies. 1.0 (no change) until Qwen2.5's
/// published `generation_config.json` value is read from the raw file; the
/// container that built this could not reach it, and the value is not
/// guessed. The loop stop is live either way.
pub const DECODE_PENALTY: f32 = 1.0;

/// Apply the repetition penalty `penalty` (>= 1.0) to `logits` for every
/// distinct token among the last `window` of `generated`. Tokens outside the
/// vocabulary are ignored. Each token is penalised once, however often it
/// occurs.
pub fn penalize(logits: &mut [f32], generated: &[u32], window: usize, penalty: f32) {
    // 1.0 is the identity: leave every logit's bits as they are.
    if penalty == 1.0 {
        return;
    }
    let recent = &generated[generated.len().saturating_sub(window)..];
    for (i, &token) in recent.iter().enumerate() {
        // Once per distinct token: skip one already seen in this window.
        if recent[..i].contains(&token) {
            continue;
        }
        if let Some(logit) = logits.get_mut(token as usize) {
            *logit = if *logit > 0.0 {
                *logit / penalty
            } else {
                *logit * penalty
            };
        }
    }
}

/// Where the reply should end if its tail repeats: `Some(len)` when the last
/// `ngram` tokens of `generated` also occur earlier in it (starting before
/// the tail does; the two runs may overlap), meaning "keep
/// `generated[..len]`", everything before the repeated copy. `None` while
/// nothing repeats, or when `ngram` is 0. Two equal runs starting `p` apart
/// are exactly `ngram + p` tokens repeating with period `p`; the first time
/// that holds, `generated[..len]` ends after one copy of the phrase.
pub fn loop_end(generated: &[u32], ngram: usize) -> Option<usize> {
    if ngram == 0 || generated.len() <= ngram {
        return None;
    }
    let tail_start = generated.len() - ngram;
    let tail = &generated[tail_start..];
    (0..tail_start)
        .any(|start| &generated[start..start + ngram] == tail)
        .then_some(tail_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-computed: tokens 1 (positive logit) and 3 (negative) are in the
    /// window, token 0 is not; 1 occurs twice and is penalised once.
    #[test]
    fn penalty_divides_positive_and_multiplies_negative_logits_once() {
        let mut logits = [2.0f32, 3.0, 0.5, -1.0, 4.0];
        penalize(&mut logits, &[0, 9, 1, 3, 1], 4, 1.5);
        assert_eq!(logits, [2.0, 2.0, 0.5, -1.5, 4.0]);
        // 0 sits outside the 4-token window; 9 is outside the vocabulary.
    }

    /// The kill-switch value: a penalty of 1.0 leaves every logit bit-for-bit
    /// as it was, so greedy picks the same token.
    #[test]
    fn penalty_one_changes_nothing() {
        let original = [1.25f32, -0.0, 0.0, -3.5, 7.0e-30, -7.0e-30, 123.456];
        let mut logits = original;
        penalize(&mut logits, &[0, 1, 2, 3, 4, 5, 6], 64, 1.0);
        for (a, b) in logits.iter().zip(&original) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    /// Jarvis, #97 RETURN item 1: at 1.0 a signalling NaN keeps its bits
    /// (multiplying it by 1.0 would quiet it to 0x7fc00001), as do the
    /// other logits.
    #[test]
    fn penalty_one_keeps_a_signalling_nan() {
        let original = [
            f32::from_bits(0x7f80_0001),
            f32::from_bits(0xff80_0001),
            2.0,
        ];
        let mut logits = original;
        penalize(&mut logits, &[0, 1, 2], 64, 1.0);
        for (a, b) in logits.iter().zip(&original) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        assert_eq!(logits[0].to_bits(), 0x7f80_0001);
    }

    /// Jarvis, #97 RETURN item 2, under the contract as reworded: an 8-token
    /// phrase said three times has repeated for 16 tokens beyond its first
    /// copy, so it is cut, keeping one copy; said twice it is never cut.
    #[test]
    fn a_phrase_is_cut_after_ngram_tokens_of_repetition_keeping_one_copy() {
        let thrice: Vec<u32> = (0..24).map(|i| i % 8).collect();
        assert_eq!(loop_end(&thrice, 16), Some(8));
        let twice: Vec<u32> = (0..16).map(|i| i % 8).collect();
        assert_eq!(loop_end(&twice, 16), None);
        // For every period below `ngram`, fed one token at a time: no cut
        // until `ngram + p` tokens, then a cut that keeps one copy.
        for p in 1..16usize {
            let text: Vec<u32> = (0..64).map(|i| (i % p) as u32 + 100).collect();
            let fired = (1..=text.len()).find_map(|n| loop_end(&text[..n], 16).map(|k| (n, k)));
            assert_eq!(fired, Some((16 + p, p)), "period {p}");
        }
    }

    /// The penalty can move the argmax off a token that keeps winning.
    #[test]
    fn penalty_breaks_a_one_token_loop() {
        let logits = [1.0f32, 1.2, 0.3];
        let mut penalised = logits;
        penalize(&mut penalised, &[1], 64, 1.3);
        let best = |l: &[f32]| {
            l.iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0
        };
        assert_eq!(best(&logits), 1);
        assert_eq!(best(&penalised), 0);
    }

    #[test]
    fn loop_end_cuts_the_repeated_copy() {
        // "a b c d" then "a b c d" again: with n = 4 the reply keeps one copy.
        let g = [10, 11, 12, 13, 10, 11, 12, 13];
        assert_eq!(loop_end(&g, 4), Some(4));
        assert_eq!(loop_end(&g[..7], 4), None);
        // An overlapping repeat counts: "x x x x x" with n = 3.
        assert_eq!(loop_end(&[7, 7, 7, 7], 3), Some(1));
        // A shorter phrase repeating does not trip a longer n.
        assert_eq!(loop_end(&[1, 2, 3, 9, 1, 2, 3], 4), None);
        assert_eq!(loop_end(&[1, 2, 3, 9, 1, 2, 3], 3), Some(4));
        // Degenerate arguments.
        assert_eq!(loop_end(&g, 0), None);
        assert_eq!(loop_end(&[], 4), None);
        assert_eq!(loop_end(&[1, 2, 3, 4], 4), None);
    }

    /// The loop stop fires exactly when the first full repeated copy lands,
    /// not before: fed one token at a time, a sentence of `LOOP_NGRAM` tokens
    /// repeated forever ends with one copy kept.
    #[test]
    fn loop_end_fires_on_the_first_full_repeat() {
        let sentence: [u32; LOOP_NGRAM] = core::array::from_fn(|i| 100 + i as u32);
        let mut generated = [0u32; 3 * LOOP_NGRAM];
        for (i, t) in generated.iter_mut().enumerate() {
            *t = sentence[i % LOOP_NGRAM];
        }
        let fired = (1..=generated.len())
            .find_map(|len| loop_end(&generated[..len], LOOP_NGRAM).map(|keep| (len, keep)));
        assert_eq!(fired, Some((2 * LOOP_NGRAM, LOOP_NGRAM)));
    }

    /// A deterministic stand-in for the model: next-token logits from a hash
    /// of the context, over a 50-token vocabulary, with no repetition built in.
    fn toy_logits(context: &[u32]) -> [f32; 50] {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &t in context {
            h = (h ^ u64::from(t)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        core::array::from_fn(|i| {
            let x = (h ^ (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
                .wrapping_mul(0xff51_afd7_ed55_8ccd);
            ((x >> 40) as f32 / (1u64 << 24) as f32) * 8.0 - 4.0
        })
    }

    fn argmax(logits: &[f32]) -> u32 {
        let mut best = 0;
        for (i, &l) in logits.iter().enumerate() {
            if l > logits[best] {
                best = i;
            }
        }
        best as u32
    }

    /// Greedy decode with the guard as the kernel core applies it (`guard`:
    /// penalty before the pick, loop stop after it), or without it.
    fn decode(prompt: &[u32], max_new: usize, guard: Option<f32>) -> Vec<u32> {
        let mut context = prompt.to_vec();
        let mut generated = Vec::new();
        for _ in 0..max_new {
            let mut logits = toy_logits(&context);
            if let Some(p) = guard {
                penalize(&mut logits, &generated, REPETITION_WINDOW, p);
            }
            let next = argmax(&logits);
            generated.push(next);
            context.push(next);
            if guard.is_some()
                && let Some(keep) = loop_end(&generated, LOOP_NGRAM)
            {
                generated.truncate(keep);
                break;
            }
        }
        generated
    }

    /// The kill-switch: the guard at the product penalty (1.0) picks exactly
    /// the tokens the unguarded decode picks, as long as nothing loops; this
    /// toy model's 200 tokens never repeat a 16-token run.
    #[test]
    fn guard_at_the_product_penalty_picks_today_s_tokens() {
        assert_eq!(DECODE_PENALTY, 1.0);
        for prompt in [&[1u32, 2, 3][..], &[7], &[42, 0, 42, 0]] {
            let today = decode(prompt, 200, None);
            assert!(loop_end(&today, LOOP_NGRAM).is_none());
            assert_eq!(decode(prompt, 200, Some(DECODE_PENALTY)), today);
        }
    }

    /// The A9 failure shape ("…the moon is a natural satellite. The moon is
    /// a natural satellite. The moon…" until max_new): a varied opening, then
    /// one sentence repeated. Fed a token at a time, the loop stop ends the
    /// reply with the opening and one copy of the sentence, as soon as the
    /// second copy's first 16 tokens are out.
    #[test]
    fn a_looping_answer_ends_after_one_copy() {
        let opening: Vec<u32> = (500..519).collect();
        let sentence: Vec<u32> = (900..923).collect(); // 23 tokens, > LOOP_NGRAM
        let mut stream = opening.clone();
        for _ in 0..10 {
            stream.extend_from_slice(&sentence);
        }
        let mut generated = Vec::new();
        let mut kept = None;
        for &token in &stream {
            generated.push(token);
            if let Some(keep) = loop_end(&generated, LOOP_NGRAM) {
                kept = Some((generated.len(), keep));
                break;
            }
        }
        let (stopped_at, keep) = kept.expect("the loop stop fires");
        assert_eq!(stopped_at, opening.len() + sentence.len() + LOOP_NGRAM);
        assert_eq!(keep, opening.len() + sentence.len());
        let mut want = opening;
        want.extend_from_slice(&sentence);
        assert_eq!(&generated[..keep], &want[..]);
    }
}
