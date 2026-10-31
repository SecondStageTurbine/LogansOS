// SPDX-License-Identifier: MPL-2.0
//! Byte-oracle for the OT-28 fast≡slow KV-persist KAT fixture.
//!
//! The kernel `sys_generate_kv_persist_equivalence` scenario `include_bytes!`s a
//! committed tiny inventory `.neom` (`kernel/src/ai/fixtures/kv_persist_kat.neom`)
//! and runs the persistent-KV vs reprefill-baseline generation paths on it. This
//! test is the independent oracle for that committed binary: it regenerates the
//! bytes from the authoritative `neomc` writer and asserts the committed file is
//! byte-identical, so the fixture can never silently drift from the generator.
//!
//! Regenerate the committed fixture (e.g. after changing the geometry/weights):
//!   NEO_WRITE_KV_PERSIST_FIXTURE=1 cargo test -p neomc kv_persist_kat_fixture

use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    // tools/neomc → repo root → the kernel fixture the KAT include_bytes!s.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../kernel/src/ai/fixtures/kv_persist_kat.neom")
}

#[test]
fn kv_persist_kat_fixture_is_valid_and_matches_committed_bytes() {
    let built = neomc::fixtures::kv_persist_kat_inventory_bytes()
        .expect("neomc must build the KV-persist KAT inventory .neom");

    // The generator output must itself be a structurally valid `.neom`.
    neomc::validator::validate(&built)
        .expect("generated KV-persist KAT fixture must pass the neomc validator");

    let path = fixture_path();

    if std::env::var_os("NEO_WRITE_KV_PERSIST_FIXTURE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &built).unwrap();
        eprintln!("wrote {} ({} bytes)", path.display(), built.len());
        return;
    }

    let committed = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "committed KAT fixture {} is missing ({e}); regenerate with \
             NEO_WRITE_KV_PERSIST_FIXTURE=1 cargo test -p neomc kv_persist_kat_fixture",
            path.display()
        )
    });

    assert_eq!(
        committed.len(),
        built.len(),
        "committed KAT fixture length differs from the neomc generator (byte-oracle)"
    );
    assert_eq!(
        committed, built,
        "committed KAT fixture is stale vs the neomc generator (byte-oracle); \
         regenerate with NEO_WRITE_KV_PERSIST_FIXTURE=1"
    );
}
