// SPDX-License-Identifier: CC0-1.0
// Independent llama.cpp golden for the Qwen3.5-0.8B `Hello` greedy decode.
//
// Raw-source provenance (captured 2026-07-18; never transcribed through an LLM):
// - GGUF: `Qwen3.5-0.8B-Q4_K_M.gguf`
// - GGUF SHA-256: `f5b14da98939b60bbe1019a964eba656407e1e0b64f1fe3003ff6d650e93bfec`
// - llama.cpp: `5f04dc7ac3f40271f60105574be0f617887195bb` (version 9638)
// - server: `llama-server -m Qwen3.5-0.8B-Q4_K_M.gguf --host 127.0.0.1
//   --port 18080 -c 2048 -ngl 0`
// - request: `POST /completion {"prompt":"Hello","n_predict":32,
//   "temperature":0,"seed":0,"n_probs":5,"stream":false,
//   "cache_prompt":false}`
// - extraction: the `id` field of each of the 32 raw JSON
//   `completion_probabilities` entries, in response order.

#[allow(dead_code)]
pub const LLAMA_CPP_QWEN35_HELLO_GREEDY_32: [u32; 32] = [
    11, 353, 1044, 264, 5286, 506, 264, 11759, 13, 353, 1044, 4237, 264, 5392, 383, 279, 8282, 314,
    328, 760, 14955, 314, 279, 3130, 303, 279, 10499, 314, 279, 36317, 3158, 353,
];

#[allow(dead_code)]
pub const LLAMA_CPP_QWEN35_HELLO_GENERATED_IDS_LINE: &str =
    "generated_ids=32 [11,353,1044,264,5286,506,264,11759,13,353,1044,4237,264,5392,383,279,8282,314,328,760,14955,314,279,3130,303,279,10499,314,279,36317,3158,353]";
