// SPDX-License-Identifier: MPL-2.0
mod format;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use format::{
    ExtraFile, ImageVariant, MkfsOptions, ModelFile, create_image_variant, hex_hash, image_hash,
    verify_image, verify_rejection_paths,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("neofs-mkfs: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        print_help();
        return Ok(());
    };
    let rest: Vec<String> = args.collect();
    match cmd.as_str() {
        "create" => cmd_create(&rest),
        "verify" => cmd_verify(&rest),
        "sanity" => cmd_sanity(&rest),
        "hash" => cmd_hash(&rest),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn cmd_create(args: &[String]) -> Result<(), String> {
    let output = required_path(args, "--output")?;
    let options = options_from_args(args)?;
    let variant = variant_from_args(args)?;
    let summary = create_image_variant(&output, &options, variant)?;
    println!(
        "NEOFS_MKFS/1 status=ok path={} variant={} image_size={} hash={} superblock_copies={} root_inode_block={} next_free_block={} free_map_block={}",
        output.display(),
        variant.name(),
        summary.image_size_bytes,
        hex_hash(&summary.image_hash),
        summary.superblock_copies,
        summary.root_inode_block,
        summary.next_free_block,
        summary.free_map_block
    );
    Ok(())
}

fn cmd_verify(args: &[String]) -> Result<(), String> {
    let image = positional_path(args, 0).ok_or("verify requires an image path")?;
    let report = verify_image(&image)?;
    let hash = image_hash(&image)?;
    println!(
        "NEOFS_MKFS_VERIFY/1 status=ok path={} image_size={} hash={} superblock_copies={} selected_generation={} root_inode_block={} entries={} model_kind=Model indirect_zero={} reserved_zero={} free_map_present={}",
        image.display(),
        report.image_size_bytes,
        hex_hash(&hash),
        report.superblock_copies,
        report.selected_generation,
        report.root_inode_block,
        report.entries.join(","),
        report.indirect_zero,
        report.reserved_zero,
        report.free_map_present
    );
    Ok(())
}

/// BLAKE3 of a plain file, for checking a staging source against the
/// `content_hash` an existing NeoFS inode records.
///
/// Exists because `content_hash` is defined as BLAKE3(logical file bytes), so a
/// recorded hash doubles as a fingerprint for the file a rebuild stages FROM.
/// Verifying that independently matters: a corrupted extract or transfer would
/// otherwise be re-hashed during mkfs into a value that agrees with itself all
/// the way into the new image -- the closed-loop failure the byte-oracle rule
/// exists to prevent. Reuses the same blake3 the format uses.
fn cmd_hash(args: &[String]) -> Result<(), String> {
    let path = positional_path(args, 0).ok_or("hash requires a file path")?;
    let hash = image_hash(&path)?;
    let hex = hex_hash(&hash);
    if let Some(expected) = optional_value(args, "--expect") {
        let expected = expected.trim().to_ascii_lowercase();
        if expected != hex {
            return Err(format!(
                "NEOFS_MKFS_HASH/1 status=MISMATCH path={} actual={} expected={}",
                path.display(),
                hex,
                expected
            ));
        }
        println!(
            "NEOFS_MKFS_HASH/1 status=match path={} hash={}",
            path.display(),
            hex
        );
        return Ok(());
    }
    println!(
        "NEOFS_MKFS_HASH/1 status=ok path={} hash={}",
        path.display(),
        hex
    );
    Ok(())
}

fn cmd_sanity(args: &[String]) -> Result<(), String> {
    let output_dir = optional_path(args, "--output-dir")
        .unwrap_or_else(|| PathBuf::from("target/neofs-mkfs-sanity"));
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("create {}: {e}", output_dir.display()))?;

    let options = options_from_args(args)?;
    verify_rejection_paths()?;

    let image_a = output_dir.join("neofs-a.img");
    let image_b = output_dir.join("neofs-b.img");
    let a = create_image_variant(&image_a, &options, ImageVariant::Normal)?;
    let b = create_image_variant(&image_b, &options, ImageVariant::Normal)?;
    let report_a = verify_image(&image_a)?;
    let report_b = verify_image(&image_b)?;

    if a.image_hash != b.image_hash {
        return Err(format!(
            "deterministic hash mismatch: {} vs {}",
            hex_hash(&a.image_hash),
            hex_hash(&b.image_hash)
        ));
    }
    if report_a != report_b {
        return Err("deterministic verification reports differ".to_string());
    }

    println!(
        "NEOFS_MKFS_SANITY/1 status=PASS output_dir={} image_size={} hash={} deterministic=true superblock_copies={} root_kind=Directory entries={} model_kind=Model indirect_zero={} reserved_zero={} free_map_present={} rejects=ok",
        output_dir.display(),
        a.image_size_bytes,
        hex_hash(&a.image_hash),
        report_a.superblock_copies,
        report_a.entries.join(","),
        report_a.indirect_zero,
        report_a.reserved_zero,
        report_a.free_map_present
    );
    Ok(())
}

fn options_from_args(args: &[String]) -> Result<MkfsOptions, String> {
    let mut options = MkfsOptions::default();
    if let Some(size) = optional_value(args, "--size-mib") {
        let size_mib = size
            .parse::<u64>()
            .map_err(|_| format!("invalid --size-mib `{size}`"))?;
        options.image_size_bytes = size_mib
            .checked_mul(1024 * 1024)
            .ok_or("image size overflow")?;
    }
    if let Some(block_size) = optional_value(args, "--block-size") {
        options.block_size = block_size
            .parse::<u32>()
            .map_err(|_| format!("invalid --block-size `{block_size}`"))?;
    }
    if let Some(kind) = optional_value(args, "--model-kind-id") {
        options.model_kind_id = kind
            .parse::<u8>()
            .map_err(|_| format!("invalid --model-kind-id `{kind}`"))?;
    }
    if let Some(path) = optional_path(args, "--model-path") {
        options.model_bytes =
            Some(std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?);
    }
    for spec in repeated_values(args, "--extra-model") {
        let (name, path) = spec
            .split_once('=')
            .ok_or_else(|| format!("invalid --extra-model `{spec}`, expected name=path"))?;
        let path = PathBuf::from(path);
        options.extra_model_files.push(ModelFile {
            name: name.to_string(),
            bytes: std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?,
        });
    }
    for spec in repeated_values(args, "--extra-file") {
        let (name, path) = spec
            .split_once('=')
            .ok_or_else(|| format!("invalid --extra-file `{spec}`, expected name=path"))?;
        let path = PathBuf::from(path);
        options.extra_regular_files.push(ExtraFile {
            name: name.to_string(),
            bytes: std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?,
        });
    }
    Ok(options)
}

fn variant_from_args(args: &[String]) -> Result<ImageVariant, String> {
    optional_value(args, "--variant")
        .as_deref()
        .map(ImageVariant::from_name)
        .unwrap_or(Ok(ImageVariant::Normal))
}

fn required_path(args: &[String], flag: &str) -> Result<PathBuf, String> {
    optional_path(args, flag).ok_or_else(|| format!("missing required {flag}"))
}

fn optional_path(args: &[String], flag: &str) -> Option<PathBuf> {
    optional_value(args, flag).map(PathBuf::from)
}

fn optional_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|pair| (pair[0] == flag).then(|| pair[1].clone()))
}

fn repeated_values(args: &[String], flag: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
        .collect()
}

fn positional_path(args: &[String], index: usize) -> Option<PathBuf> {
    args.iter()
        .filter(|arg| !arg.starts_with("--"))
        .nth(index)
        .map(PathBuf::from)
}

fn print_help() {
    println!("NeoFS v1 mkfs tool");
    println!();
    println!("USAGE:");
    println!(
        "    neofs-mkfs create --output <path> [--variant normal|arthur-memory|arthur-memory-forgetting|arthur-reversibility|arthur-reversibility-stakes|corrupt-primary|highest-generation|reserved-type|corrupt-free-map] [--size-mib 128] [--block-size 4096] [--model-path <neom>] [--extra-model <name=neom> ...] [--extra-file <name=path> ...]"
    );
    println!("    neofs-mkfs verify <path>");
    println!("    neofs-mkfs sanity [--output-dir <dir>] [--size-mib 128]");
    println!("    neofs-mkfs hash <path> [--expect <hex>]");
    println!();
    println!(
        "    `hash` is BLAKE3 over a plain file -- use it to check a staging source against the"
    );
    println!("    content_hash an existing inode records. Exits non-zero on --expect mismatch.");
}
