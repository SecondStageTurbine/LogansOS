// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.2: building and pinning the install bundle
//! (`docs/THINK-2026-10-27-ot87-installer.md` §4.3; the format is
//! `kernel/src/install_bundle_logic.rs`).
//!
//! `cargo xtask install-bundle` writes `target/install/bundle.neoinst` and
//! prints its BLAKE3. The bundle is built **before** the kernel: R4.3 compiles
//! that hash into an `installer` kernel through [`PIN_ENV`], and an image is
//! assembled only from a staged bundle that hashes to it ([`check_staged`]).

use std::fs;
use std::path::{Path, PathBuf};

use crate::esp_template;
use crate::install_bundle_logic::{
    self as logic, ESP_512, ESP_4096, KIND_SLOT_MAP, KIND_SPARSE, NEOFS_SEED, SLOTS_512, SLOTS_4096,
};

/// The environment variable an `installer` kernel build reads the pin from.
pub const PIN_ENV: &str = "LOGANSOS_INSTALL_BUNDLE_BLAKE3";
pub const BUNDLE_REL: &str = "target/install/bundle.neoinst";
/// Where the image carries the bundle: the kernel's `installer::BUNDLE_MODULE`
/// (`/install/bundle`), relative to the ISO root.
pub const MODULE_REL: &str = "install/bundle";
/// Sparse images are stored as their non-zero 4 KiB blocks.
const SPARSE_BLOCK: usize = 4096;

/// The bundle's bytes from its inputs: Limine's `BOOTX64.EFI` and the NeoFS
/// seed image. Deterministic: the same inputs give the same bytes. One
/// 512 MiB template is held at a time.
pub fn assemble(bootx64: &[u8], neofs_seed: &[u8]) -> Result<Vec<u8>, String> {
    let mut templates = Vec::new();
    for sector in [512, 4096] {
        let (image, map) = esp_template::build(sector, bootx64)?;
        let sparse = logic::encode_sparse(&image, SPARSE_BLOCK);
        expands_to(&sparse, &image).map_err(|e| format!("esp-{sector}: {e}"))?;
        let slots = logic::encode_slot_map(&map).ok_or("slot map: bad path")?;
        templates.push((image.len() as u64, sparse, map, slots));
    }
    let seed = logic::encode_sparse(neofs_seed, SPARSE_BLOCK);
    expands_to(&seed, neofs_seed).map_err(|e| format!("{NEOFS_SEED}: {e}"))?;
    let [
        (len512, sparse512, map512, slots512),
        (len4096, sparse4096, map4096, slots4096),
    ] = <[_; 2]>::try_from(templates).map_err(|_| "two templates")?;
    let parts: [(&str, u32, u64, &[u8]); 5] = [
        (ESP_512, KIND_SPARSE, len512, &sparse512),
        (ESP_4096, KIND_SPARSE, len4096, &sparse4096),
        (SLOTS_512, KIND_SLOT_MAP, slots512.len() as u64, &slots512),
        (
            SLOTS_4096,
            KIND_SLOT_MAP,
            slots4096.len() as u64,
            &slots4096,
        ),
        (NEOFS_SEED, KIND_SPARSE, neofs_seed.len() as u64, &seed),
    ];
    let bundle = logic::encode_bundle(&parts).ok_or("bundle: bad part name")?;
    // Read it back the way the installer will: the manifest, every part's
    // hash and bytes, and the slot maps.
    let manifest =
        logic::installer_manifest(&bundle).map_err(|e| format!("bundle manifest: {e:?}"))?;
    for (part, (_, _, expanded, stored)) in manifest.iter().zip(parts) {
        let bytes =
            logic::check_part(&bundle, part).map_err(|e| format!("{}: {e:?}", part.name))?;
        if bytes != stored || part.expanded != expanded {
            return Err(format!("{}: reads back different", part.name));
        }
    }
    for (stored, map) in [(&slots512, &map512), (&slots4096, &map4096)] {
        if logic::slot_map(stored).as_ref() != Ok(map) {
            return Err(format!(
                "slot map S={}: reads back different",
                map.sector_size
            ));
        }
    }
    Ok(bundle)
}

/// Whether the sparse image `stored` expands to `image`, checked 1 MiB at
/// a time.
fn expands_to(stored: &[u8], image: &[u8]) -> Result<(), String> {
    let runs = logic::sparse_runs(stored, image.len() as u64).map_err(|e| format!("{e:?}"))?;
    let mut window = vec![0u8; 1 << 20];
    for (i, chunk) in image.chunks(window.len()).enumerate() {
        let out = &mut window[..chunk.len()];
        logic::expand_into(&runs, (i << 20) as u64, out);
        if out != chunk {
            return Err(format!("expands to different bytes in MiB {i}"));
        }
    }
    Ok(())
}

/// Refuse a staged bundle that doesn't hash to `pin_hex` (§4.3 item 2, the
/// same discipline as the model pin check).
pub fn check_staged(bundle: &[u8], pin_hex: &str) -> Result<(), String> {
    let actual = crate::hex_string(blake3::hash(bundle).as_bytes());
    if !actual.eq_ignore_ascii_case(pin_hex.trim()) {
        return Err(format!(
            "install bundle BLAKE3 {actual} does not match the kernel's pin {pin_hex}; \
             rebuild the bundle and the kernel together"
        ));
    }
    logic::installer_manifest(bundle)
        .map(|_| ())
        .map_err(|e| format!("install bundle manifest: {e:?}"))
}

/// Build the bundle into `target/install/`. Returns its path and BLAKE3.
pub fn build_to_target() -> Result<(PathBuf, String), String> {
    let limine = crate::resolve_limine_dir()?;
    let bootx64_path = limine.join("BOOTX64.EFI");
    let bootx64 =
        fs::read(&bootx64_path).map_err(|e| format!("read {}: {e}", bootx64_path.display()))?;
    let dir = crate::workspace_root().join("target/install");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let seed_path = dir.join("neofs-seed.img");
    crate::prepare_neofs_storage_disk_with_payload_files_sized(
        &seed_path,
        "arthur-memory",
        None,
        &[],
        &[],
        None,
    )?;
    let seed = fs::read(&seed_path).map_err(|e| format!("read {}: {e}", seed_path.display()))?;
    let bundle = assemble(&bootx64, &seed)?;
    let path = crate::workspace_root().join(BUNDLE_REL);
    write_atomically(&path, &bundle)?;
    Ok((path, crate::hex_string(blake3::hash(&bundle).as_bytes())))
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
}

/// `cargo xtask install-bundle [--check <blake3>]`.
pub fn cmd(args: &[String]) -> Result<(), String> {
    match args {
        [] => {
            let (path, hash) = build_to_target()?;
            let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!(
                "INSTALL_BUNDLE/1 status=ok path={} bytes={len} blake3={hash} pin_env={PIN_ENV}",
                path.display()
            );
            Ok(())
        }
        [flag, pin] if flag == "--check" => {
            let path = crate::workspace_root().join(BUNDLE_REL);
            let bundle = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            check_staged(&bundle, pin)?;
            println!(
                "INSTALL_BUNDLE/1 status=pinned path={} blake3={pin}",
                path.display()
            );
            Ok(())
        }
        _ => Err("usage: cargo xtask install-bundle [--check <blake3>]".into()),
    }
}

/// Each deployment file against its slot file (THINK §7.2: kernel 72 MiB,
/// each module 44 MiB). An image whose installer would have to truncate one
/// is not built.
pub fn check_slot_sizes(files: &[(&str, &Path)]) -> Result<(), String> {
    for (name, path) in files {
        let bytes = fs::metadata(path)
            .map_err(|e| format!("stat {}: {e}", path.display()))?
            .len();
        slot_fits(name, bytes)?;
    }
    Ok(())
}

/// Whether a deployment file of `bytes` fits slot file `name`.
pub fn slot_fits(name: &str, bytes: u64) -> Result<(), String> {
    let (_, slot) = esp_template::SLOT_FILES
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| format!("{name}: no slot file of that name"))?;
    if bytes > *slot {
        return Err(format!(
            "{name} is {bytes} bytes, more than its {slot}-byte slot file (THINK §7.2); \
             the installer would truncate it, so the image is not built"
        ));
    }
    Ok(())
}
