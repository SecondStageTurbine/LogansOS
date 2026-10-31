// SPDX-License-Identifier: MPL-2.0
//! B1b: which boot-module models the kernel will load, and the digest each one
//! must have (`Plans/DECISION-2026-10-19-arthur-first-boot-model.md`, D1/D4;
//! OT-73).
//!
//! Pure and allocation-free: host-tested by `tools/module-reclaim-logic-tests`
//! through `#[path]`, so keep it free of `crate::` paths and feature cfgs. The
//! kernel chooses which pin table applies.
//!
//! A model is loaded from a boot module only through a path under
//! [`BOOT_MODEL_DIR`], and only if that module has a pin here. The pin is the
//! BLAKE3 of the whole `.neom` file, which is the NeoFS `content_hash` of the
//! same bytes. Every byte is hashed and compared before any byte is used; a
//! mismatch loads nothing (fail closed).

/// The namespace `sys_model_load` treats as "a boot module, not a file".
pub const BOOT_MODEL_DIR: &str = "/boot/models/";

/// A boot-module model and the digest it must have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelPin {
    /// The module's path suffix, as Limine reports it (`/models/<file>`).
    pub module: &'static str,
    /// BLAKE3 of the whole file.
    pub blake3: [u8; 32],
}

/// Qwen2.5-0.5B-Instruct (Q4_K_M), imported to `.neom`: 485,489,152 bytes.
/// Recorded from the real file's NeoFS `content_hash`, re-verified offline
/// over all bytes (`docs/OPEN-THREADS.md` OT-73). Upstream reproducibility (the
/// raw GGUF imported twice, byte-identical) is still pending: B1-1.
pub const QWEN25_05B: ModelPin = ModelPin {
    module: "/models/model-qwen2.5-0.5b.neom",
    blake3: hex32(b"a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe"),
};

/// The models a product kernel will load from a boot module.
pub const PRODUCT_PINS: &[ModelPin] = &[QWEN25_05B];

/// The hermetic CI fixture (`neomc fixtures` → `valid_embedding.neom`, 4,144
/// bytes, deterministic), staged under the 0.5B module name by the
/// `arthur_boot_module_*` scenarios. Only test kernels accept it, and they
/// still accept the real 0.5B digest, so a real model staged on a test run is
/// held to the real pin.
pub const FIXTURE_05B: ModelPin = ModelPin {
    module: "/models/model-qwen2.5-0.5b.neom",
    blake3: hex32(b"7be90f8ddc8472a5da8588230fbdee3750dabcd6f40ad0860588c237e894931e"),
};

/// B1c: the tiny inventory LM the kv-persist KAT uses
/// (`kernel/src/ai/fixtures/kv_persist_kat.neom`, 382,464 bytes), staged by
/// the `real_lm_generate_boot` scenario to drive the product generate core.
pub const KAT_INVENTORY: ModelPin = ModelPin {
    module: "/models/kv-persist-kat.neom",
    blake3: hex32(b"6f14b933789cf24683579d030f31d737360ba890b7f3a65899f63759c1f1029c"),
};

/// The models a test kernel will load from a boot module.
pub const TEST_PINS: &[ModelPin] = &[QWEN25_05B, FIXTURE_05B, KAT_INVENTORY];

/// B1c: the 0.5B tokenizer vocabulary, imported from the same pinned GGUF:
/// 7,990,862 bytes. Reproduced byte-for-byte by two independent imports
/// (`Plans/RETURN-2026-10-20-b1-1-model-import-janus.md`).
pub const QWEN25_05B_VOCAB: ModelPin = ModelPin {
    module: "/vocab/model-qwen2.5-0.5b.vocab",
    blake3: hex32(b"e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265"),
};

/// The hermetic fixture vocabulary xtask writes beside the fixture model
/// (`write_stage_b_fixture_vocab_sidecar`): 5,435,213 bytes, deterministic.
pub const FIXTURE_05B_VOCAB: ModelPin = ModelPin {
    module: "/vocab/model-qwen2.5-0.5b.vocab",
    blake3: hex32(b"5ac8a1c3b6281f41fa7ec68ca24dd7d1016da0c5b38fd57c6e053f8dba25b73a"),
};

/// The same vocabulary prebuilt as `NEOVOCB1` (`xtask vocab-bin`, from the
/// pinned sidecar above; `Plans/NOTE-2026-10-26-arthur-binary-vocab-design-s1.md`):
/// 5,010,512 bytes. The converter is deterministic, so this follows from the
/// sidecar's pin.
pub const QWEN25_05B_VOCB: ModelPin = ModelPin {
    module: "/vocab/model-qwen2.5-0.5b.vocb",
    blake3: hex32(b"7973d7392444d3faea8d558105c9722c9106483a5a44410fa27fc1c4e6cee015"),
};

/// The fixture vocabulary's `NEOVOCB1`: 3,380,944 bytes.
pub const FIXTURE_05B_VOCB: ModelPin = ModelPin {
    module: "/vocab/model-qwen2.5-0.5b.vocb",
    blake3: hex32(b"98f942bf134c25c9effb3db9535cf87ad739677280cf1797dec3e5b3bc5fd9ce"),
};

/// The vocabularies a product kernel serves from `/boot/vocab/`.
pub const PRODUCT_VOCAB_PINS: &[ModelPin] = &[QWEN25_05B_VOCAB, QWEN25_05B_VOCB];

/// The vocabularies a test kernel serves (the real ones still held to their
/// pins).
pub const TEST_VOCAB_PINS: &[ModelPin] = &[
    QWEN25_05B_VOCAB,
    FIXTURE_05B_VOCAB,
    QWEN25_05B_VOCB,
    FIXTURE_05B_VOCB,
];

/// Ordinary modules under this directory are served only if pinned.
pub const VOCAB_DIR: &str = "/vocab/";

/// `/boot/models/<file>` → the module suffix `/models/<file>`. `None` for any
/// other path, an empty file name, or a name with a further `/`.
pub fn module_suffix(path: &str) -> Option<&str> {
    let name = path.strip_prefix(BOOT_MODEL_DIR)?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    // "/boot/models/x" → "/models/x": drop the leading "/boot".
    Some(&path["/boot".len()..])
}

/// Whether `vocab` (`/vocab/<name>.vocab`, or its `NEOVOCB1` form
/// `/vocab/<name>.vocb`) is the vocabulary of `model` (`/models/<name>.neom`):
/// the same `<name>`.
pub fn vocab_pairs_model(model: &str, vocab: &str) -> bool {
    let Some(name) = model
        .strip_prefix(CONSUMABLE_MODEL_DIR)
        .and_then(|m| m.strip_suffix(".neom"))
    else {
        return false;
    };
    vocab
        .strip_prefix(VOCAB_DIR)
        .and_then(|v| v.strip_suffix(".vocab").or_else(|| v.strip_suffix(".vocb")))
        .is_some_and(|v| !name.is_empty() && v == name)
}

/// The module directory of consumable boot models.
pub const CONSUMABLE_MODEL_DIR: &str = "/models/";

/// OT-87 R2: where an installed system's NeoFS volume is mounted.
pub const NEOFS_DIR: &str = "/neofs/";

/// OT-87 R2: the pin a file on the installed NeoFS volume is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeofsPin {
    /// A model: the pin's module suffix (`/models/<file>`).
    Model(&'static str),
    /// A vocabulary: the pin's module suffix (`/vocab/<file>`).
    Vocab(&'static str),
}

/// OT-87 R2: the file name a pin's module suffix has on NeoFS (`/models/x` or
/// `/vocab/x` → `x`). `None` for any other suffix.
pub fn neofs_name(module: &str) -> Option<&str> {
    module
        .strip_prefix(CONSUMABLE_MODEL_DIR)
        .or_else(|| module.strip_prefix(VOCAB_DIR))
        .filter(|name| !name.is_empty() && !name.contains('/'))
}

/// OT-87 R2: `/neofs/<file>` → the pin it is held to, when a model or vocab
/// pin has that file name. `path` must already be normalized (no `.`, `..`
/// or doubled `/`). `None` for an unpinned name, a nested path, or a path
/// outside [`NEOFS_DIR`]: those load as before, with no pin.
pub fn neofs_pin(
    models: &'static [ModelPin],
    vocabs: &'static [ModelPin],
    path: &str,
) -> Option<NeofsPin> {
    let name = path.strip_prefix(NEOFS_DIR)?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let named = |p: &&ModelPin| neofs_name(p.module) == Some(name);
    if let Some(pin) = models.iter().find(named) {
        return Some(NeofsPin::Model(pin.module));
    }
    vocabs.iter().find(named).map(|p| NeofsPin::Vocab(p.module))
}

/// What a module's bytes are, measured against the pins for its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// No pin names this module: it is never loaded, and never consumed.
    Unpinned,
    /// The digest equals one of the module's pins.
    Match,
    /// The module is pinned and the digest equals none of its pins.
    Mismatch,
}

/// Is `module` pinned at all? Checked before the module is taken, so an
/// unpinned module is never consumed.
pub fn is_pinned(pins: &[ModelPin], module: &str) -> bool {
    pins.iter().any(|p| p.module == module)
}

/// Judge `digest` against every pin for `module`.
pub fn verdict(pins: &[ModelPin], module: &str, digest: &[u8; 32]) -> Verdict {
    let mut pinned = false;
    for pin in pins.iter().filter(|p| p.module == module) {
        pinned = true;
        if &pin.blake3 == digest {
            return Verdict::Match;
        }
    }
    if pinned {
        Verdict::Mismatch
    } else {
        Verdict::Unpinned
    }
}

/// The first pin for `module`: the digest a mismatch is reported against.
pub fn expected(pins: &[ModelPin], module: &str) -> Option<[u8; 32]> {
    pins.iter().find(|p| p.module == module).map(|p| p.blake3)
}

const fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        // Compile-time pins only: a bad digit fails the build.
        _ => panic!("pin digest must be lowercase hex"),
    }
}

/// 64 lowercase hex digits → 32 bytes, at compile time.
pub const fn hex32(hex: &[u8; 64]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = (nibble(hex[2 * i]) << 4) | nibble(hex[2 * i + 1]);
        i += 1;
    }
    out
}

/// 32 bytes → 64 lowercase hex digits, for serial lines.
pub fn hex_digest(bytes: &[u8; 32]) -> [u8; 64] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 64];
    for (i, b) in bytes.iter().enumerate() {
        out[2 * i] = DIGITS[(b >> 4) as usize];
        out[2 * i + 1] = DIGITS[(b & 0xf) as usize];
    }
    out
}
