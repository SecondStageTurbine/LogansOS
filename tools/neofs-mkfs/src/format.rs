// SPDX-License-Identifier: MPL-2.0
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const FS_BLOCK_SIZE: u32 = 4096;
pub const FS_BLOCK_SIZE_LOG2: u16 = 12;
pub const SUPERBLOCK_SIZE: usize = 512;
pub const INODE_SIZE: usize = 256;
pub const DIRENT_SIZE: usize = 64;
pub const DIRECT_BLOCKS: usize = 8;
pub const INDIRECT_POINTERS_PER_BLOCK: usize = FS_BLOCK_SIZE as usize / std::mem::size_of::<u64>();
pub const SINGLE_INDIRECT_BLOCKS: usize = INDIRECT_POINTERS_PER_BLOCK;
pub const DOUBLE_INDIRECT_BLOCKS: usize = INDIRECT_POINTERS_PER_BLOCK * INDIRECT_POINTERS_PER_BLOCK;
pub const TRIPLE_INDIRECT_BLOCKS: usize =
    INDIRECT_POINTERS_PER_BLOCK * INDIRECT_POINTERS_PER_BLOCK * INDIRECT_POINTERS_PER_BLOCK;
pub const MAX_FILE_SIZE: usize = 2 * 1024 * 1024 * 1024;
pub const MAX_FILE_BLOCKS: u64 = (MAX_FILE_SIZE / FS_BLOCK_SIZE as usize) as u64;
pub const DEFAULT_IMAGE_SIZE_BYTES: u64 = 128 * 1024 * 1024;
pub const FORMAT_VERSION: u16 = 1;
pub const TYPE_VERSION: u8 = 1;
pub const FREE_MAP_BITS_PER_BLOCK: u64 = FS_BLOCK_SIZE as u64 * 8;

const SUPERBLOCK_MAGIC: &[u8; 8] = b"NEOFSv1\0";
const INODE_MAGIC: &[u8; 8] = b"NEOFSINO";
const SUPERBLOCK_COPY_OFFSETS: [u64; 3] = [0, 1024 * 1024, 64 * 1024 * 1024];

const READ: u32 = 1 << 0;
const WRITE: u32 = 1 << 1;
const LOOKUP_RESERVED: u32 = 1 << 2;
const READDIR_RESERVED: u32 = 1 << 3;
const EXECUTE_RESERVED: u32 = 1 << 4;
const MMAP_RESERVED: u32 = 1 << 5;
const GRANT_RESERVED: u32 = 1 << 24;
const DERIVE_RESERVED: u32 = 1 << 25;

const HELLO_CONTENT: &[u8] = b"Hello from NeoFS v1 BUILD-2 2A mkfs.\n";

#[derive(Clone, Debug)]
pub struct ModelFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ExtraFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct MkfsOptions {
    pub image_size_bytes: u64,
    pub block_size: u32,
    pub model_kind_id: u8,
    pub model_bytes: Option<Vec<u8>>,
    pub extra_model_files: Vec<ModelFile>,
    pub extra_regular_files: Vec<ExtraFile>,
    pub include_arthur_memory: bool,
    pub include_arthur_memory_forgetting: bool,
    pub include_arthur_reversibility: bool,
    pub include_arthur_stakes_policy: bool,
    /// OT-66 L1: an empty `.system/` at the root, where sign-in keeps the
    /// account's credential record (userland makes no directories).
    pub include_system_dir: bool,
}

impl Default for MkfsOptions {
    fn default() -> Self {
        Self {
            image_size_bytes: DEFAULT_IMAGE_SIZE_BYTES,
            block_size: FS_BLOCK_SIZE,
            model_kind_id: InodeKind::Model as u8,
            model_bytes: None,
            extra_model_files: Vec::new(),
            extra_regular_files: Vec::new(),
            include_arthur_memory: false,
            include_arthur_memory_forgetting: false,
            include_arthur_reversibility: false,
            include_arthur_stakes_policy: false,
            include_system_dir: false,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ImageVariant {
    Normal,
    ArthurMemory,
    /// The VM image's data disk (OT-87 R1): `arthur-memory` and an empty
    /// `.system/` (OT-66 L1).
    ArthurMemorySystem,
    ArthurMemoryForgetting,
    ArthurReversibility,
    ArthurReversibilityStakes,
    CorruptPrimary,
    HighestGeneration,
    ReservedType,
    CorruptFreeMap,
    /// A newer generation in superblock copy 2 whose root has a second
    /// entry naming `hello.txt`'s inode. Mount must reject that candidate
    /// (#206 AAG, C1: freeing one entry's version would free the other's).
    SharedInode,
    /// A newer generation in copy 2 with a file whose inode names one data
    /// block twice (`direct_blocks = [D, D]`, hash over D‖D). Mount must
    /// reject it.
    DoubledBlock,
}

impl ImageVariant {
    pub fn from_name(name: &str) -> Result<Self, String> {
        match name {
            "normal" => Ok(Self::Normal),
            "arthur-memory" => Ok(Self::ArthurMemory),
            "arthur-memory-system" => Ok(Self::ArthurMemorySystem),
            "arthur-memory-forgetting" => Ok(Self::ArthurMemoryForgetting),
            "arthur-reversibility" => Ok(Self::ArthurReversibility),
            "arthur-reversibility-stakes" => Ok(Self::ArthurReversibilityStakes),
            "corrupt-primary" => Ok(Self::CorruptPrimary),
            "highest-generation" => Ok(Self::HighestGeneration),
            "reserved-type" => Ok(Self::ReservedType),
            "corrupt-free-map" => Ok(Self::CorruptFreeMap),
            "shared-inode" => Ok(Self::SharedInode),
            "doubled-block" => Ok(Self::DoubledBlock),
            _ => Err(format!("unknown image variant `{name}`")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::ArthurMemory => "arthur-memory",
            Self::ArthurMemorySystem => "arthur-memory-system",
            Self::ArthurMemoryForgetting => "arthur-memory-forgetting",
            Self::ArthurReversibility => "arthur-reversibility",
            Self::ArthurReversibilityStakes => "arthur-reversibility-stakes",
            Self::CorruptPrimary => "corrupt-primary",
            Self::HighestGeneration => "highest-generation",
            Self::ReservedType => "reserved-type",
            Self::CorruptFreeMap => "corrupt-free-map",
            Self::SharedInode => "shared-inode",
            Self::DoubledBlock => "doubled-block",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImageSummary {
    pub image_size_bytes: u64,
    pub image_hash: [u8; 32],
    pub superblock_copies: usize,
    pub root_inode_block: u64,
    pub next_free_block: u64,
    pub free_map_block: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifyReport {
    pub image_size_bytes: u64,
    pub superblock_copies: usize,
    pub selected_generation: u64,
    pub root_inode_block: u64,
    pub entries: Vec<String>,
    pub indirect_zero: bool,
    pub reserved_zero: bool,
    pub free_map_present: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
enum InodeKind {
    Regular = 0,
    Directory = 1,
    Symlink = 2,
    Model = 3,
}

impl TryFrom<u8> for InodeKind {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Regular),
            1 => Ok(Self::Directory),
            2 => Ok(Self::Symlink),
            3 => Ok(Self::Model),
            _ => Err(format!("unsupported inode type id {value}")),
        }
    }
}

impl InodeKind {
    fn name(self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::Directory => "Directory",
            Self::Symlink => "Symlink",
            Self::Model => "Model",
        }
    }

    fn rights(self) -> u32 {
        match self {
            Self::Regular => {
                READ | WRITE | EXECUTE_RESERVED | MMAP_RESERVED | GRANT_RESERVED | DERIVE_RESERVED
            }
            Self::Directory => {
                LOOKUP_RESERVED | READDIR_RESERVED | GRANT_RESERVED | DERIVE_RESERVED
            }
            Self::Symlink => READ,
            Self::Model => READ | MMAP_RESERVED | GRANT_RESERVED | DERIVE_RESERVED,
        }
    }
}

#[derive(Clone, Debug)]
struct Superblock {
    total_blocks: u64,
    next_free_block: u64,
    generation: u64,
    root_inode_block: u64,
    root_inode_hash: [u8; 32],
    inode_size_bytes: u32,
    next_generation: u32,
    free_map: Option<FreeMapRef>,
}

#[derive(Clone, Debug)]
struct FreeMapRef {
    start_block: u64,
    block_count: u64,
    hash: [u8; 32],
}

#[derive(Clone, Debug)]
struct Inode {
    kind: InodeKind,
    generation: u32,
    file_size: u64,
    block_count: u64,
    direct_blocks: [u64; DIRECT_BLOCKS],
    single_indirect_block: u64,
    double_indirect_block: u64,
    triple_indirect_block: u64,
    content_hash: [u8; 32],
    directory_entry_count: u32,
    fs_block_count: u64,
}

#[derive(Clone, Debug)]
struct DirEntry {
    name: String,
    child_block: u64,
    kind: InodeKind,
}

struct RootFileSpec {
    name: String,
    kind: InodeKind,
    content: Vec<u8>,
}

struct FileBlockLayout {
    direct_blocks: [u64; DIRECT_BLOCKS],
    single_indirect_block: u64,
    double_indirect_block: u64,
    triple_indirect_block: u64,
    data_blocks: Vec<u64>,
    indirect_blocks: Vec<(u64, [u8; FS_BLOCK_SIZE as usize])>,
}

struct AllocatedFile {
    name: String,
    content: Vec<u8>,
    inode_block: u64,
    kind: InodeKind,
    generation: u32,
    layout: FileBlockLayout,
}

const ARTHUR_M4_FACT_MEM: &[u8] = b"m4 opaque fact memory bytes before forget 0001";
const ARTHUR_M4_PREFERENCE_MEM: &[u8] = b"m4 opaque preference memory bytes before wipe 0002";
const ARTHUR_M4_EVENT_MEM: &[u8] = b"m4 opaque event memory bytes before wipe 0003";
const ARTHUR_M4_RELATIONSHIP_MEM: &[u8] = b"m4 opaque relationship memory bytes before wipe 0004";
const ARTHUR_M4_KEY_FILE_LEN: usize = 48;
const ARTHUR_STAKES_POLICY: &[u8] =
    br#"{"confirm_destructive":true,"paths":["/scratch"],"write_file":["/scratch"]}"#;

fn arthur_m4_seed_key(seed: u8) -> Vec<u8> {
    let mut key = Vec::with_capacity(ARTHUR_M4_KEY_FILE_LEN);
    for idx in 0..ARTHUR_M4_KEY_FILE_LEN {
        key.push(seed.wrapping_add(idx as u8));
    }
    key
}

fn arthur_memory_forgetting_files() -> Vec<RootFileSpec> {
    vec![
        RootFileSpec {
            name: "fact.mem".to_string(),
            kind: InodeKind::Regular,
            content: ARTHUR_M4_FACT_MEM.to_vec(),
        },
        RootFileSpec {
            name: "preference.mem".to_string(),
            kind: InodeKind::Regular,
            content: ARTHUR_M4_PREFERENCE_MEM.to_vec(),
        },
        RootFileSpec {
            name: "event.mem".to_string(),
            kind: InodeKind::Regular,
            content: ARTHUR_M4_EVENT_MEM.to_vec(),
        },
        RootFileSpec {
            name: "relationship.mem".to_string(),
            kind: InodeKind::Regular,
            content: ARTHUR_M4_RELATIONSHIP_MEM.to_vec(),
        },
        RootFileSpec {
            name: "fact.key".to_string(),
            kind: InodeKind::Regular,
            content: arthur_m4_seed_key(0x11),
        },
        RootFileSpec {
            name: "preference.key".to_string(),
            kind: InodeKind::Regular,
            content: arthur_m4_seed_key(0x22),
        },
        RootFileSpec {
            name: "event.key".to_string(),
            kind: InodeKind::Regular,
            content: arthur_m4_seed_key(0x33),
        },
        RootFileSpec {
            name: "relationship.key".to_string(),
            kind: InodeKind::Regular,
            content: arthur_m4_seed_key(0x44),
        },
    ]
}

pub fn create_image(path: &Path, options: &MkfsOptions) -> Result<ImageSummary, String> {
    validate_options(options)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }

    let total_blocks = options.image_size_bytes / FS_BLOCK_SIZE as u64;
    let copy_blocks = superblock_copy_blocks(options.image_size_bytes)?;
    let first_data_block = copy_blocks
        .iter()
        .max()
        .copied()
        .ok_or("no superblock copies")?
        + 1;

    let free_map_block_count = free_map_block_count_for_total(total_blocks)?;
    let free_map_block = first_data_block;
    let root_inode_block = free_map_block + free_map_block_count;
    let root_dir_block = root_inode_block + 1;
    let hello_inode_block = root_dir_block + 1;
    let hello_data_block = hello_inode_block + 1;

    let mut root_files =
        Vec::with_capacity(1 + options.extra_model_files.len() + options.extra_regular_files.len());
    root_files.push(RootFileSpec {
        name: "model.neom".to_string(),
        kind: InodeKind::Model,
        content: options.model_bytes.clone().unwrap_or_else(model_content),
    });
    root_files.extend(
        options
            .extra_model_files
            .iter()
            .cloned()
            .map(|model| RootFileSpec {
                name: model.name,
                kind: InodeKind::Model,
                content: model.bytes,
            }),
    );
    root_files.extend(
        options
            .extra_regular_files
            .iter()
            .cloned()
            .map(|file| RootFileSpec {
                name: file.name,
                kind: InodeKind::Regular,
                content: file.bytes,
            }),
    );

    let mut next_block = hello_data_block + 1;
    let mut file_layouts = Vec::with_capacity(root_files.len());
    let root_file_generation_base = 3u32;
    for (index, spec) in root_files.into_iter().enumerate() {
        let block_count = data_block_count(spec.content.len())?;
        let inode_block = next_block;
        next_block += 1;
        let layout = allocate_file_block_layout(&mut next_block, block_count)?;
        file_layouts.push(AllocatedFile {
            content: spec.content,
            inode_block,
            kind: spec.kind,
            generation: root_file_generation_base + index as u32,
            layout,
            name: spec.name,
        });
    }

    let include_arthur_home = options.include_arthur_memory
        || options.include_arthur_reversibility
        || options.include_arthur_stakes_policy;
    let memory_file_specs = if options.include_arthur_memory_forgetting {
        arthur_memory_forgetting_files()
    } else {
        Vec::new()
    };
    let subdir_generation = root_file_generation_base + file_layouts.len() as u32;
    let mut next_generation_id = subdir_generation + 1;
    let arthur_generation = if include_arthur_home {
        let generation = next_generation_id;
        next_generation_id += 1;
        Some(generation)
    } else {
        None
    };
    let arthur_memory_generation = if options.include_arthur_memory {
        let generation = next_generation_id;
        next_generation_id += 1;
        Some(generation)
    } else {
        None
    };
    let arthur_undo_generation = if options.include_arthur_reversibility {
        let generation = next_generation_id;
        next_generation_id += 1;
        Some(generation)
    } else {
        None
    };
    let arthur_undo_actions_generation = if options.include_arthur_reversibility {
        let generation = next_generation_id;
        next_generation_id += 1;
        Some(generation)
    } else {
        None
    };
    let arthur_undo_scratch_generation = if options.include_arthur_reversibility {
        let generation = next_generation_id;
        next_generation_id += 1;
        Some(generation)
    } else {
        None
    };
    let arthur_stakes_generation = if options.include_arthur_stakes_policy {
        let generation = next_generation_id;
        next_generation_id += 1;
        Some(generation)
    } else {
        None
    };
    let memory_file_generation_base = next_generation_id;
    // `.system` numbers after the memory files and lays out after every
    // other block, so an image without it is byte-identical to before.
    let system_generation = memory_file_generation_base + memory_file_specs.len() as u32;

    let subdir_inode_block = next_block;
    let mut next_block_after_dirs = subdir_inode_block + 1;
    let arthur_inode_block = if include_arthur_home {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_dir_block = if include_arthur_home {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_memory_inode_block = if options.include_arthur_memory {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_memory_dir_block = if options.include_arthur_memory && !memory_file_specs.is_empty()
    {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_undo_inode_block = if options.include_arthur_reversibility {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_undo_dir_block = if options.include_arthur_reversibility {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_undo_actions_inode_block = if options.include_arthur_reversibility {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_undo_actions_dir_block = if options.include_arthur_reversibility {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_undo_scratch_inode_block = if options.include_arthur_reversibility {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_stakes_inode_block = if options.include_arthur_stakes_policy {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let arthur_stakes_layout = if options.include_arthur_stakes_policy {
        let layout = allocate_file_block_layout(
            &mut next_block_after_dirs,
            data_block_count(ARTHUR_STAKES_POLICY.len())?,
        )?;
        Some(layout)
    } else {
        None
    };
    let mut memory_file_layouts = Vec::with_capacity(memory_file_specs.len());
    for (index, spec) in memory_file_specs.into_iter().enumerate() {
        let block_count = data_block_count(spec.content.len())?;
        let inode_block = next_block_after_dirs;
        next_block_after_dirs += 1;
        let layout = allocate_file_block_layout(&mut next_block_after_dirs, block_count)?;
        memory_file_layouts.push(AllocatedFile {
            content: spec.content,
            inode_block,
            kind: spec.kind,
            generation: memory_file_generation_base + index as u32,
            layout,
            name: spec.name,
        });
    }
    let system_inode_block = if options.include_system_dir {
        let block = next_block_after_dirs;
        next_block_after_dirs += 1;
        Some(block)
    } else {
        None
    };
    let next_free_block = next_block_after_dirs;
    if next_free_block > total_blocks {
        return Err("image too small for deterministic mkfs payload".to_string());
    }
    let free_map_bytes = vec![0u8; free_map_byte_len(free_map_block_count)?];

    let mut root_dir_data = [0u8; FS_BLOCK_SIZE as usize];
    let root_entry_count = 1
        + file_layouts.len()
        + 1
        + usize::from(include_arthur_home)
        + usize::from(options.include_system_dir);
    if root_entry_count > FS_BLOCK_SIZE as usize / DIRENT_SIZE {
        return Err("too many deterministic root entries for one direct block".to_string());
    }
    let mut entry_index = 0usize;
    let mut write_root_entry = |data: &mut [u8; FS_BLOCK_SIZE as usize],
                                name: &str,
                                child_block: u64,
                                kind: InodeKind|
     -> Result<(), String> {
        let start = entry_index * DIRENT_SIZE;
        write_dir_entry(
            &mut data[start..start + DIRENT_SIZE],
            name,
            child_block,
            kind,
        )?;
        entry_index += 1;
        Ok(())
    };
    write_root_entry(
        &mut root_dir_data,
        "hello.txt",
        hello_inode_block,
        InodeKind::Regular,
    )?;
    for file in &file_layouts {
        write_root_entry(&mut root_dir_data, &file.name, file.inode_block, file.kind)?;
    }
    write_root_entry(
        &mut root_dir_data,
        "subdir",
        subdir_inode_block,
        InodeKind::Directory,
    )?;
    if include_arthur_home {
        write_root_entry(
            &mut root_dir_data,
            ".arthur",
            arthur_inode_block.expect("Arthur inode block exists"),
            InodeKind::Directory,
        )?;
    }
    if let Some(block) = system_inode_block {
        write_root_entry(&mut root_dir_data, ".system", block, InodeKind::Directory)?;
    }

    let mut arthur_dir_data = [0u8; FS_BLOCK_SIZE as usize];
    let mut arthur_entry_count = 0usize;
    if options.include_arthur_memory {
        let start = arthur_entry_count * DIRENT_SIZE;
        write_dir_entry(
            &mut arthur_dir_data[start..start + DIRENT_SIZE],
            "memory",
            arthur_memory_inode_block.expect("Arthur memory inode block exists"),
            InodeKind::Directory,
        )?;
        arthur_entry_count += 1;
    }
    if options.include_arthur_reversibility {
        let start = arthur_entry_count * DIRENT_SIZE;
        write_dir_entry(
            &mut arthur_dir_data[start..start + DIRENT_SIZE],
            "undo",
            arthur_undo_inode_block.expect("Arthur undo inode block exists"),
            InodeKind::Directory,
        )?;
        arthur_entry_count += 1;
    }
    if options.include_arthur_stakes_policy {
        let start = arthur_entry_count * DIRENT_SIZE;
        write_dir_entry(
            &mut arthur_dir_data[start..start + DIRENT_SIZE],
            "stakes.json",
            arthur_stakes_inode_block.expect("Arthur stakes inode block exists"),
            InodeKind::Regular,
        )?;
        arthur_entry_count += 1;
    }
    let arthur_memory_entry_count = memory_file_layouts.len();
    if arthur_memory_entry_count > FS_BLOCK_SIZE as usize / DIRENT_SIZE {
        return Err("too many deterministic Arthur memory entries".to_string());
    }
    let mut arthur_memory_dir_data = [0u8; FS_BLOCK_SIZE as usize];
    for (idx, file) in memory_file_layouts.iter().enumerate() {
        let start = idx * DIRENT_SIZE;
        write_dir_entry(
            &mut arthur_memory_dir_data[start..start + DIRENT_SIZE],
            &file.name,
            file.inode_block,
            file.kind,
        )?;
    }

    let hello_data = data_block(HELLO_CONTENT)?;
    let next_generation = memory_file_generation_base + memory_file_layouts.len() as u32;

    let root_inode = write_inode(InodeWrite {
        kind: InodeKind::Directory,
        generation: 1,
        file_size: (DIRENT_SIZE * root_entry_count) as u64,
        direct_blocks: [root_dir_block, 0, 0, 0, 0, 0, 0, 0],
        single_indirect_block: 0,
        double_indirect_block: 0,
        triple_indirect_block: 0,
        block_count: 1,
        content_hash: blake3_hash(&root_dir_data[..DIRENT_SIZE * root_entry_count]),
        directory_entry_count: root_entry_count as u32,
    });
    let hello_inode = write_inode(InodeWrite {
        kind: InodeKind::Regular,
        generation: 2,
        file_size: HELLO_CONTENT.len() as u64,
        direct_blocks: [hello_data_block, 0, 0, 0, 0, 0, 0, 0],
        single_indirect_block: 0,
        double_indirect_block: 0,
        triple_indirect_block: 0,
        block_count: 1,
        content_hash: blake3_hash(HELLO_CONTENT),
        directory_entry_count: 0,
    });
    let subdir_inode = write_inode(InodeWrite {
        kind: InodeKind::Directory,
        generation: subdir_generation,
        file_size: 0,
        direct_blocks: [0; DIRECT_BLOCKS],
        single_indirect_block: 0,
        double_indirect_block: 0,
        triple_indirect_block: 0,
        block_count: 0,
        content_hash: blake3_hash(&[]),
        directory_entry_count: 0,
    });
    let arthur_inode = if include_arthur_home {
        Some(write_inode(InodeWrite {
            kind: InodeKind::Directory,
            generation: arthur_generation.expect("Arthur generation exists"),
            file_size: (DIRENT_SIZE * arthur_entry_count) as u64,
            direct_blocks: [
                arthur_dir_block.expect("Arthur directory block exists"),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            block_count: 1,
            content_hash: blake3_hash(&arthur_dir_data[..DIRENT_SIZE * arthur_entry_count]),
            directory_entry_count: arthur_entry_count as u32,
        }))
    } else {
        None
    };
    let arthur_memory_inode = if options.include_arthur_memory {
        Some(write_inode(InodeWrite {
            kind: InodeKind::Directory,
            generation: arthur_memory_generation.expect("Arthur memory generation exists"),
            file_size: (DIRENT_SIZE * arthur_memory_entry_count) as u64,
            direct_blocks: arthur_memory_dir_block
                .map(|block| [block, 0, 0, 0, 0, 0, 0, 0])
                .unwrap_or([0; DIRECT_BLOCKS]),
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            block_count: if arthur_memory_dir_block.is_some() {
                1
            } else {
                0
            },
            content_hash: blake3_hash(
                &arthur_memory_dir_data[..DIRENT_SIZE * arthur_memory_entry_count],
            ),
            directory_entry_count: arthur_memory_entry_count as u32,
        }))
    } else {
        None
    };
    let mut arthur_undo_dir_data = [0u8; FS_BLOCK_SIZE as usize];
    let mut arthur_undo_actions_dir_data = [0u8; FS_BLOCK_SIZE as usize];
    if options.include_arthur_reversibility {
        write_dir_entry(
            &mut arthur_undo_dir_data[..DIRENT_SIZE],
            "actions",
            arthur_undo_actions_inode_block.expect("Arthur undo actions inode block exists"),
            InodeKind::Directory,
        )?;
        write_dir_entry(
            &mut arthur_undo_actions_dir_data[..DIRENT_SIZE],
            "cd2dd734db24ea74",
            arthur_undo_scratch_inode_block.expect("Arthur undo scratch inode block exists"),
            InodeKind::Directory,
        )?;
    }
    let arthur_undo_inode = if options.include_arthur_reversibility {
        Some(write_inode(InodeWrite {
            kind: InodeKind::Directory,
            generation: arthur_undo_generation.expect("Arthur undo generation exists"),
            file_size: DIRENT_SIZE as u64,
            direct_blocks: [
                arthur_undo_dir_block.expect("Arthur undo directory block exists"),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            block_count: 1,
            content_hash: blake3_hash(&arthur_undo_dir_data[..DIRENT_SIZE]),
            directory_entry_count: 1,
        }))
    } else {
        None
    };
    let arthur_undo_actions_inode = if options.include_arthur_reversibility {
        Some(write_inode(InodeWrite {
            kind: InodeKind::Directory,
            generation: arthur_undo_actions_generation
                .expect("Arthur undo actions generation exists"),
            file_size: DIRENT_SIZE as u64,
            direct_blocks: [
                arthur_undo_actions_dir_block.expect("Arthur undo actions directory block exists"),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            block_count: 1,
            content_hash: blake3_hash(&arthur_undo_actions_dir_data[..DIRENT_SIZE]),
            directory_entry_count: 1,
        }))
    } else {
        None
    };
    let arthur_undo_scratch_inode = if options.include_arthur_reversibility {
        Some(write_inode(InodeWrite {
            kind: InodeKind::Directory,
            generation: arthur_undo_scratch_generation
                .expect("Arthur undo scratch generation exists"),
            file_size: 0,
            direct_blocks: [0; DIRECT_BLOCKS],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            block_count: 0,
            content_hash: blake3_hash(&[]),
            directory_entry_count: 0,
        }))
    } else {
        None
    };
    let arthur_stakes_inode = if options.include_arthur_stakes_policy {
        let layout = arthur_stakes_layout
            .as_ref()
            .expect("Arthur stakes layout exists");
        Some(write_inode(InodeWrite {
            kind: InodeKind::Regular,
            generation: arthur_stakes_generation.expect("Arthur stakes generation exists"),
            file_size: ARTHUR_STAKES_POLICY.len() as u64,
            direct_blocks: layout.direct_blocks,
            single_indirect_block: layout.single_indirect_block,
            double_indirect_block: layout.double_indirect_block,
            triple_indirect_block: layout.triple_indirect_block,
            block_count: layout.data_blocks.len() as u64,
            content_hash: blake3_hash(ARTHUR_STAKES_POLICY),
            directory_entry_count: 0,
        }))
    } else {
        None
    };

    let root_inode_hash = blake3_hash(&root_inode);
    let superblock = write_superblock(Superblock {
        total_blocks,
        next_free_block,
        generation: 1,
        root_inode_block,
        root_inode_hash,
        inode_size_bytes: INODE_SIZE as u32,
        next_generation,
        free_map: Some(FreeMapRef {
            start_block: free_map_block,
            block_count: free_map_block_count,
            hash: blake3_hash(&free_map_bytes),
        }),
    });

    let mut file = File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    file.set_len(options.image_size_bytes)
        .map_err(|e| format!("set length {}: {e}", path.display()))?;
    for offset in SUPERBLOCK_COPY_OFFSETS {
        write_all_at(&mut file, offset, &superblock)?;
    }
    for idx in 0..free_map_block_count {
        let start = usize::try_from(idx)
            .ok()
            .and_then(|idx| idx.checked_mul(FS_BLOCK_SIZE as usize))
            .ok_or("free-map offset overflow")?;
        write_all_at(
            &mut file,
            block_offset(free_map_block + idx),
            &free_map_bytes[start..start + FS_BLOCK_SIZE as usize],
        )?;
    }
    let root_inode_block_bytes = block_with_prefix(&root_inode);
    write_all_at(
        &mut file,
        block_offset(root_inode_block),
        &root_inode_block_bytes,
    )?;
    write_all_at(&mut file, block_offset(root_dir_block), &root_dir_data)?;
    let hello_inode_block_bytes = block_with_prefix(&hello_inode);
    write_all_at(
        &mut file,
        block_offset(hello_inode_block),
        &hello_inode_block_bytes,
    )?;
    write_all_at(&mut file, block_offset(hello_data_block), &hello_data)?;
    for file_layout in &file_layouts {
        let file_inode = write_inode(InodeWrite {
            kind: file_layout.kind,
            generation: file_layout.generation,
            file_size: file_layout.content.len() as u64,
            direct_blocks: file_layout.layout.direct_blocks,
            single_indirect_block: file_layout.layout.single_indirect_block,
            double_indirect_block: file_layout.layout.double_indirect_block,
            triple_indirect_block: file_layout.layout.triple_indirect_block,
            block_count: file_layout.layout.data_blocks.len() as u64,
            content_hash: blake3_hash(&file_layout.content),
            directory_entry_count: 0,
        });
        let file_inode_block_bytes = block_with_prefix(&file_inode);
        write_all_at(
            &mut file,
            block_offset(file_layout.inode_block),
            &file_inode_block_bytes,
        )?;
        for (idx, block) in data_blocks(&file_layout.content)?.iter().enumerate() {
            write_all_at(
                &mut file,
                block_offset(file_layout.layout.data_blocks[idx]),
                block,
            )?;
        }
        for (block, bytes) in &file_layout.layout.indirect_blocks {
            write_all_at(&mut file, block_offset(*block), bytes)?;
        }
    }
    let subdir_inode_block_bytes = block_with_prefix(&subdir_inode);
    write_all_at(
        &mut file,
        block_offset(subdir_inode_block),
        &subdir_inode_block_bytes,
    )?;
    if let Some(block) = system_inode_block {
        // Empty, like `subdir`.
        let system_inode = write_inode(InodeWrite {
            kind: InodeKind::Directory,
            generation: system_generation,
            file_size: 0,
            direct_blocks: [0; DIRECT_BLOCKS],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            block_count: 0,
            content_hash: blake3_hash(&[]),
            directory_entry_count: 0,
        });
        write_all_at(
            &mut file,
            block_offset(block),
            &block_with_prefix(&system_inode),
        )?;
    }
    if include_arthur_home {
        let arthur_inode_block_bytes =
            block_with_prefix(arthur_inode.as_ref().expect("Arthur inode exists"));
        write_all_at(
            &mut file,
            block_offset(arthur_inode_block.expect("Arthur inode block exists")),
            &arthur_inode_block_bytes,
        )?;
        write_all_at(
            &mut file,
            block_offset(arthur_dir_block.expect("Arthur directory block exists")),
            &arthur_dir_data,
        )?;
        if options.include_arthur_stakes_policy {
            let stakes_inode_block_bytes = block_with_prefix(
                arthur_stakes_inode
                    .as_ref()
                    .expect("Arthur stakes inode exists"),
            );
            write_all_at(
                &mut file,
                block_offset(arthur_stakes_inode_block.expect("Arthur stakes inode block exists")),
                &stakes_inode_block_bytes,
            )?;
            let layout = arthur_stakes_layout
                .as_ref()
                .expect("Arthur stakes layout exists");
            for (idx, block) in data_blocks(ARTHUR_STAKES_POLICY)?.iter().enumerate() {
                write_all_at(&mut file, block_offset(layout.data_blocks[idx]), block)?;
            }
            for (block, bytes) in &layout.indirect_blocks {
                write_all_at(&mut file, block_offset(*block), bytes)?;
            }
        }
    }
    if options.include_arthur_memory {
        let arthur_memory_inode_block_bytes = block_with_prefix(
            arthur_memory_inode
                .as_ref()
                .expect("Arthur memory inode exists"),
        );
        write_all_at(
            &mut file,
            block_offset(arthur_memory_inode_block.expect("Arthur memory inode block exists")),
            &arthur_memory_inode_block_bytes,
        )?;
        if let Some(block) = arthur_memory_dir_block {
            write_all_at(&mut file, block_offset(block), &arthur_memory_dir_data)?;
        }
        for file_layout in &memory_file_layouts {
            let file_inode = write_inode(InodeWrite {
                kind: file_layout.kind,
                generation: file_layout.generation,
                file_size: file_layout.content.len() as u64,
                direct_blocks: file_layout.layout.direct_blocks,
                single_indirect_block: file_layout.layout.single_indirect_block,
                double_indirect_block: file_layout.layout.double_indirect_block,
                triple_indirect_block: file_layout.layout.triple_indirect_block,
                block_count: file_layout.layout.data_blocks.len() as u64,
                content_hash: blake3_hash(&file_layout.content),
                directory_entry_count: 0,
            });
            let file_inode_block_bytes = block_with_prefix(&file_inode);
            write_all_at(
                &mut file,
                block_offset(file_layout.inode_block),
                &file_inode_block_bytes,
            )?;
            for (idx, block) in data_blocks(&file_layout.content)?.iter().enumerate() {
                write_all_at(
                    &mut file,
                    block_offset(file_layout.layout.data_blocks[idx]),
                    block,
                )?;
            }
            for (block, bytes) in &file_layout.layout.indirect_blocks {
                write_all_at(&mut file, block_offset(*block), bytes)?;
            }
        }
    }
    if options.include_arthur_reversibility {
        let arthur_undo_inode_block_bytes = block_with_prefix(
            arthur_undo_inode
                .as_ref()
                .expect("Arthur undo inode exists"),
        );
        write_all_at(
            &mut file,
            block_offset(arthur_undo_inode_block.expect("Arthur undo inode block exists")),
            &arthur_undo_inode_block_bytes,
        )?;
        write_all_at(
            &mut file,
            block_offset(arthur_undo_dir_block.expect("Arthur undo directory block exists")),
            &arthur_undo_dir_data,
        )?;
        let arthur_undo_actions_inode_block_bytes = block_with_prefix(
            arthur_undo_actions_inode
                .as_ref()
                .expect("Arthur undo actions inode exists"),
        );
        write_all_at(
            &mut file,
            block_offset(
                arthur_undo_actions_inode_block.expect("Arthur undo actions inode block exists"),
            ),
            &arthur_undo_actions_inode_block_bytes,
        )?;
        write_all_at(
            &mut file,
            block_offset(
                arthur_undo_actions_dir_block.expect("Arthur undo actions directory block exists"),
            ),
            &arthur_undo_actions_dir_data,
        )?;
        let arthur_undo_scratch_inode_block_bytes = block_with_prefix(
            arthur_undo_scratch_inode
                .as_ref()
                .expect("Arthur undo scratch inode exists"),
        );
        write_all_at(
            &mut file,
            block_offset(
                arthur_undo_scratch_inode_block.expect("Arthur undo scratch inode block exists"),
            ),
            &arthur_undo_scratch_inode_block_bytes,
        )?;
    }
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))?;

    let image_hash = image_hash(path)?;
    Ok(ImageSummary {
        image_size_bytes: options.image_size_bytes,
        image_hash,
        superblock_copies: copy_blocks.len(),
        root_inode_block,
        next_free_block,
        free_map_block,
    })
}

pub fn create_image_variant(
    path: &Path,
    options: &MkfsOptions,
    variant: ImageVariant,
) -> Result<ImageSummary, String> {
    let mut options = options.clone();
    if matches!(
        variant,
        ImageVariant::ArthurMemory
            | ImageVariant::ArthurMemorySystem
            | ImageVariant::ArthurMemoryForgetting
    ) {
        options.include_arthur_memory = true;
    }
    if variant == ImageVariant::ArthurMemorySystem {
        options.include_system_dir = true;
    }
    if variant == ImageVariant::ArthurMemoryForgetting {
        options.include_arthur_memory_forgetting = true;
    }
    if variant == ImageVariant::ArthurReversibility {
        options.include_arthur_reversibility = true;
    }
    if variant == ImageVariant::ArthurReversibilityStakes {
        options.include_arthur_reversibility = true;
        options.include_arthur_stakes_policy = true;
    }
    let mut summary = create_image(path, &options)?;
    match variant {
        ImageVariant::Normal => {}
        ImageVariant::ArthurMemory => {}
        ImageVariant::ArthurMemorySystem => {}
        ImageVariant::ArthurMemoryForgetting => {}
        ImageVariant::ArthurReversibility => {}
        ImageVariant::ArthurReversibilityStakes => {}
        ImageVariant::CorruptPrimary => corrupt_primary_superblock(path)?,
        ImageVariant::HighestGeneration => make_highest_generation_variant(path)?,
        ImageVariant::ReservedType => make_reserved_type_variant(path)?,
        ImageVariant::CorruptFreeMap => make_corrupt_free_map_variant(path)?,
        ImageVariant::SharedInode => make_twice_named_variant(path, false)?,
        ImageVariant::DoubledBlock => make_twice_named_variant(path, true)?,
    }
    summary.image_hash = image_hash(path)?;
    Ok(summary)
}

pub fn verify_image(path: &Path) -> Result<VerifyReport, String> {
    let mut file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let image_size = file
        .metadata()
        .map_err(|e| format!("stat {}: {e}", path.display()))?
        .len();
    let copy_offsets = superblock_copy_offsets(image_size)?;
    let mut valid = Vec::new();
    for offset in copy_offsets {
        let mut bytes = [0u8; SUPERBLOCK_SIZE];
        read_exact_at(&mut file, offset, &mut bytes)?;
        valid.push(read_superblock(&bytes)?);
    }
    let selected = valid
        .iter()
        .max_by_key(|sb| sb.generation)
        .ok_or("no valid superblock copies")?;
    if selected.inode_size_bytes != INODE_SIZE as u32 {
        return Err("inode size mismatch".to_string());
    }
    let free_map_present = match selected.free_map.as_ref() {
        Some(free_map) => {
            read_free_map_bytes(&mut file, free_map)?;
            true
        }
        None => false,
    };

    let root_bytes = read_inode_bytes(&mut file, selected.root_inode_block)?;
    if blake3_hash(&root_bytes) != selected.root_inode_hash {
        return Err("root inode hash mismatch".to_string());
    }
    let root = read_inode(&root_bytes, selected.total_blocks)?;
    if root.kind != InodeKind::Directory {
        return Err("root inode is not a Directory".to_string());
    }

    let mut indirect_zero = inode_indirect_roots_zero(&root);
    let mut reserved_zero = inode_reserved_zero(&root_bytes);
    let entries = read_directory_entries(&mut file, &root)?;
    let mut entry_names = Vec::new();
    let mut saw_hello = false;
    let mut saw_model = false;
    let mut saw_subdir = false;
    let mut saw_arthur = false;
    let mut saw_arthur_memory = false;
    let mut saw_arthur_undo = false;

    for entry in &entries {
        let child_bytes = read_inode_bytes(&mut file, entry.child_block)?;
        let child = read_inode(&child_bytes, selected.total_blocks)?;
        if child.kind != entry.kind {
            return Err(format!(
                "entry {} kind mismatch: entry={} inode={}",
                entry.name,
                entry.kind.name(),
                child.kind.name()
            ));
        }
        indirect_zero &= inode_indirect_roots_zero(&child);
        reserved_zero &= inode_reserved_zero(&child_bytes);
        verify_content_hash(&mut file, &child)?;
        entry_names.push(format!("{}:{}", entry.name, entry.kind.name()));
        match (entry.name.as_str(), entry.kind) {
            ("hello.txt", InodeKind::Regular) => saw_hello = true,
            ("model.neom", InodeKind::Model) => saw_model = true,
            ("subdir", InodeKind::Directory) => saw_subdir = true,
            (".arthur", InodeKind::Directory) => {
                saw_arthur = true;
                let arthur_entries = read_directory_entries(&mut file, &child)?;
                for arthur_entry in &arthur_entries {
                    let arthur_child_bytes = read_inode_bytes(&mut file, arthur_entry.child_block)?;
                    let arthur_child = read_inode(&arthur_child_bytes, selected.total_blocks)?;
                    if arthur_entry.name == "memory" && arthur_child.kind == InodeKind::Directory {
                        saw_arthur_memory = true;
                    } else if arthur_entry.name == "undo"
                        && arthur_child.kind == InodeKind::Directory
                    {
                        saw_arthur_undo = true;
                        let undo_entries = read_directory_entries(&mut file, &arthur_child)?;
                        if !undo_entries.iter().any(|entry| {
                            entry.name == "actions" && entry.kind == InodeKind::Directory
                        }) {
                            return Err(".arthur undo actions directory missing".to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !(saw_hello && saw_model && saw_subdir) {
        return Err("required root entries missing or mistyped".to_string());
    }
    if saw_arthur && !(saw_arthur_memory || saw_arthur_undo) {
        return Err(".arthur directory missing known child".to_string());
    }

    Ok(VerifyReport {
        image_size_bytes: image_size,
        superblock_copies: valid.len(),
        selected_generation: selected.generation,
        root_inode_block: selected.root_inode_block,
        entries: entry_names,
        indirect_zero,
        reserved_zero,
        free_map_present,
    })
}

pub fn verify_rejection_paths() -> Result<(), String> {
    let invalid_block = MkfsOptions {
        block_size: 512,
        ..MkfsOptions::default()
    };
    expect_invalid(&invalid_block, "non-4096 block size")?;

    let too_small = MkfsOptions {
        image_size_bytes: min_image_size_bytes() - 1,
        ..MkfsOptions::default()
    };
    expect_invalid(&too_small, "too-small image")?;

    let unsupported_kind = MkfsOptions {
        model_kind_id: 99,
        ..MkfsOptions::default()
    };
    expect_invalid(&unsupported_kind, "unsupported inode type id")?;

    let long_name = "a".repeat(48);
    let mut entry = [0u8; DIRENT_SIZE];
    if write_dir_entry(&mut entry, &long_name, 1, InodeKind::Regular).is_ok() {
        return Err("long filename was accepted".to_string());
    }

    Ok(())
}

fn expect_invalid(options: &MkfsOptions, label: &str) -> Result<(), String> {
    if validate_options(options).is_ok() {
        Err(format!("{label} was accepted"))
    } else {
        Ok(())
    }
}

fn validate_options(options: &MkfsOptions) -> Result<(), String> {
    if options.block_size != FS_BLOCK_SIZE {
        return Err(format!(
            "BUILD-2 supports only 4096-byte blocks, got {}",
            options.block_size
        ));
    }
    if !options
        .image_size_bytes
        .is_multiple_of(FS_BLOCK_SIZE as u64)
    {
        return Err("image size must be a multiple of 4096 bytes".to_string());
    }
    if options.image_size_bytes < min_image_size_bytes() {
        return Err(format!(
            "image too small for required three-copy policy and mkfs payload: need at least {} bytes",
            min_image_size_bytes()
        ));
    }
    let model_kind = InodeKind::try_from(options.model_kind_id)?;
    if model_kind != InodeKind::Model {
        return Err("model.neom must use the Model inode type id 3".to_string());
    }
    let mut names = vec![
        "hello.txt".to_string(),
        "model.neom".to_string(),
        "subdir".to_string(),
    ];
    if options.include_arthur_memory
        || options.include_arthur_reversibility
        || options.include_arthur_stakes_policy
    {
        names.push(".arthur".to_string());
    }
    if options.include_system_dir {
        names.push(".system".to_string());
    }
    if options.include_arthur_memory_forgetting && !options.include_arthur_memory {
        return Err(
            "Arthur M4 forgetting fixtures require the Arthur memory directory".to_string(),
        );
    }
    if options.include_arthur_stakes_policy && !options.include_arthur_reversibility {
        return Err("Arthur stakes fixtures require the Arthur undo directory".to_string());
    }
    for model in &options.extra_model_files {
        validate_root_filename(&model.name, "model")?;
        if names.iter().any(|name| name == &model.name) {
            return Err(format!("duplicate root entry `{}`", model.name));
        }
        names.push(model.name.clone());
    }
    for file in &options.extra_regular_files {
        validate_root_filename(&file.name, "file")?;
        if names.iter().any(|name| name == &file.name) {
            return Err(format!("duplicate root entry `{}`", file.name));
        }
        names.push(file.name.clone());
    }
    Ok(())
}

fn validate_root_filename(name: &str, label: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!("{label} filename must not be empty"));
    }
    if name.len() > 47 {
        return Err(format!(
            "{label} filename `{name}` exceeds 47-byte NeoFS v1 limit"
        ));
    }
    if name.as_bytes().contains(&b'/') || name.as_bytes().contains(&0) {
        return Err(format!("{label} filename `{name}` is not a root filename"));
    }
    Ok(())
}

fn min_image_size_bytes() -> u64 {
    let copy2_block = SUPERBLOCK_COPY_OFFSETS[2] / FS_BLOCK_SIZE as u64;
    block_offset(copy2_block + 9)
}

fn superblock_copy_offsets(image_size: u64) -> Result<Vec<u64>, String> {
    let offsets: Vec<u64> = SUPERBLOCK_COPY_OFFSETS
        .into_iter()
        .filter(|offset| offset + SUPERBLOCK_SIZE as u64 <= image_size)
        .collect();
    if offsets.len() != SUPERBLOCK_COPY_OFFSETS.len() {
        return Err("image does not contain all three required superblock copies".to_string());
    }
    Ok(offsets)
}

fn superblock_copy_blocks(image_size: u64) -> Result<Vec<u64>, String> {
    Ok(superblock_copy_offsets(image_size)?
        .into_iter()
        .map(|offset| offset / FS_BLOCK_SIZE as u64)
        .collect())
}

fn write_superblock(sb: Superblock) -> [u8; SUPERBLOCK_SIZE] {
    let mut out = [0u8; SUPERBLOCK_SIZE];
    out[0..8].copy_from_slice(SUPERBLOCK_MAGIC);
    put_u16(&mut out, 8, FORMAT_VERSION);
    put_u16(&mut out, 10, FS_BLOCK_SIZE_LOG2);
    out[12] = TYPE_VERSION;
    put_u64(&mut out, 16, sb.total_blocks);
    put_u64(&mut out, 24, sb.next_free_block);
    put_u64(&mut out, 32, sb.generation);
    put_u64(&mut out, 40, sb.root_inode_block);
    out[48..80].copy_from_slice(&sb.root_inode_hash);
    put_u32(&mut out, 80, sb.inode_size_bytes);
    put_u32(&mut out, 84, sb.next_generation);
    if let Some(free_map) = sb.free_map {
        put_u64(&mut out, 88, free_map.start_block);
        put_u64(&mut out, 96, free_map.block_count);
        out[104..136].copy_from_slice(&free_map.hash);
    }
    let checksum = blake3_hash(&out[..480]);
    out[480..512].copy_from_slice(&checksum);
    out
}

fn read_superblock(bytes: &[u8; SUPERBLOCK_SIZE]) -> Result<Superblock, String> {
    if &bytes[0..8] != SUPERBLOCK_MAGIC {
        return Err("bad superblock magic".to_string());
    }
    let checksum = blake3_hash(&bytes[..480]);
    if bytes[480..512] != checksum {
        return Err("bad superblock checksum".to_string());
    }
    if get_u16(bytes, 8) != FORMAT_VERSION {
        return Err("unsupported superblock format version".to_string());
    }
    if get_u16(bytes, 10) != FS_BLOCK_SIZE_LOG2 {
        return Err("unsupported block size".to_string());
    }
    if bytes[12] != TYPE_VERSION {
        return Err("unsupported superblock type version".to_string());
    }
    if !all_zero(&bytes[13..16]) {
        return Err("superblock reserved fields are not zero".to_string());
    }
    let total_blocks = get_u64(bytes, 16);
    let free_map = read_free_map_ref(bytes, total_blocks)?;

    let mut root_inode_hash = [0u8; 32];
    root_inode_hash.copy_from_slice(&bytes[48..80]);
    Ok(Superblock {
        total_blocks,
        next_free_block: get_u64(bytes, 24),
        generation: get_u64(bytes, 32),
        root_inode_block: get_u64(bytes, 40),
        root_inode_hash,
        inode_size_bytes: get_u32(bytes, 80),
        next_generation: get_u32(bytes, 84),
        free_map,
    })
}

fn read_free_map_ref(
    bytes: &[u8; SUPERBLOCK_SIZE],
    total_blocks: u64,
) -> Result<Option<FreeMapRef>, String> {
    if all_zero(&bytes[88..480]) {
        return Ok(None);
    }
    if !all_zero(&bytes[136..480]) {
        return Err("superblock free-map reserved fields are not zero".to_string());
    }
    let start_block = get_u64(bytes, 88);
    let block_count = get_u64(bytes, 96);
    let end_block = start_block
        .checked_add(block_count)
        .ok_or("free-map block range overflow")?;
    if start_block == 0 || block_count == 0 || end_block > total_blocks {
        return Err("free-map block range is invalid".to_string());
    }
    let capacity = block_count
        .checked_mul(FREE_MAP_BITS_PER_BLOCK)
        .ok_or("free-map capacity overflow")?;
    if capacity < total_blocks {
        return Err("free-map capacity is too small for the image".to_string());
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes[104..136]);
    Ok(Some(FreeMapRef {
        start_block,
        block_count,
        hash,
    }))
}

fn free_map_block_count_for_total(total_blocks: u64) -> Result<u64, String> {
    total_blocks
        .checked_add(FREE_MAP_BITS_PER_BLOCK - 1)
        .map(|blocks| blocks / FREE_MAP_BITS_PER_BLOCK)
        .filter(|blocks| *blocks != 0)
        .ok_or_else(|| "free-map block count overflow".to_string())
}

fn free_map_byte_len(block_count: u64) -> Result<usize, String> {
    block_count
        .checked_mul(FS_BLOCK_SIZE as u64)
        .and_then(|len| usize::try_from(len).ok())
        .ok_or_else(|| "free-map byte length overflow".to_string())
}

fn read_free_map_bytes(file: &mut File, free_map: &FreeMapRef) -> Result<Vec<u8>, String> {
    let len = free_map_byte_len(free_map.block_count)?;
    let mut bytes = vec![0u8; len];
    for idx in 0..free_map.block_count {
        let start = usize::try_from(idx)
            .ok()
            .and_then(|idx| idx.checked_mul(FS_BLOCK_SIZE as usize))
            .ok_or("free-map read offset overflow")?;
        read_exact_at(
            file,
            block_offset(free_map.start_block + idx),
            &mut bytes[start..start + FS_BLOCK_SIZE as usize],
        )?;
    }
    if blake3_hash(&bytes) != free_map.hash {
        return Err("free-map hash mismatch".to_string());
    }
    Ok(bytes)
}

struct InodeWrite {
    kind: InodeKind,
    generation: u32,
    file_size: u64,
    direct_blocks: [u64; DIRECT_BLOCKS],
    single_indirect_block: u64,
    double_indirect_block: u64,
    triple_indirect_block: u64,
    block_count: u64,
    content_hash: [u8; 32],
    directory_entry_count: u32,
}

fn write_inode(input: InodeWrite) -> [u8; INODE_SIZE] {
    let mut out = [0u8; INODE_SIZE];
    out[0..8].copy_from_slice(INODE_MAGIC);
    out[8] = input.kind as u8;
    out[9] = TYPE_VERSION;
    put_u32(&mut out, 12, input.generation);
    put_u64(&mut out, 16, input.file_size);
    put_u64(&mut out, 24, input.block_count);
    for (idx, block) in input.direct_blocks.iter().copied().enumerate() {
        put_u64(&mut out, 32 + idx * 8, block);
    }
    put_u64(&mut out, 96, input.single_indirect_block);
    out[104..136].copy_from_slice(&input.content_hash);
    put_u64(&mut out, 136, input.double_indirect_block);
    put_u64(&mut out, 144, input.triple_indirect_block);
    put_u32(&mut out, 152, input.kind.rights());
    put_u32(&mut out, 156, input.directory_entry_count);
    let checksum = blake3_hash(&out[..224]);
    out[224..256].copy_from_slice(&checksum);
    out
}

fn read_inode(bytes: &[u8; INODE_SIZE], total_blocks: u64) -> Result<Inode, String> {
    if &bytes[0..8] != INODE_MAGIC {
        return Err("bad inode magic".to_string());
    }
    let checksum = blake3_hash(&bytes[..224]);
    if bytes[224..256] != checksum {
        return Err("bad inode checksum".to_string());
    }
    if bytes[9] != TYPE_VERSION {
        return Err("unsupported inode type version".to_string());
    }
    if get_u16(bytes, 10) != 0 {
        return Err("inode flags/reserved fields are not zero".to_string());
    }
    let kind = InodeKind::try_from(bytes[8])?;
    let generation = get_u32(bytes, 12);
    let file_size = get_u64(bytes, 16);
    let block_count = get_u64(bytes, 24);
    let expected_blocks = file_data_blocks(file_size)?;
    if generation == 0 {
        return Err("inode generation is zero".to_string());
    }
    if block_count != expected_blocks || block_count > MAX_FILE_BLOCKS {
        return Err("inode block_count does not match file size".to_string());
    }
    if file_size > MAX_FILE_SIZE as u64 {
        return Err("inode exceeds 2 GiB NeoFS file size cap".to_string());
    }
    let mut direct_blocks = [0u64; DIRECT_BLOCKS];
    for (idx, slot) in direct_blocks.iter_mut().enumerate() {
        *slot = get_u64(bytes, 32 + idx * 8);
    }
    let direct_used = (block_count as usize).min(DIRECT_BLOCKS);
    for block in direct_blocks.iter().copied().take(direct_used) {
        validate_fs_block(block, total_blocks)?;
    }
    if direct_blocks[direct_used..].iter().any(|block| *block != 0) {
        return Err("unused direct block slot is not zero".to_string());
    }
    let single_indirect_block = get_u64(bytes, 96);
    let double_indirect_block = get_u64(bytes, 136);
    let triple_indirect_block = get_u64(bytes, 144);
    validate_indirect_root_blocks(
        block_count,
        total_blocks,
        single_indirect_block,
        double_indirect_block,
        triple_indirect_block,
    )?;
    let mut content_hash = [0u8; 32];
    content_hash.copy_from_slice(&bytes[104..136]);
    Ok(Inode {
        kind,
        generation,
        file_size,
        block_count,
        direct_blocks,
        single_indirect_block,
        double_indirect_block,
        triple_indirect_block,
        content_hash,
        directory_entry_count: get_u32(bytes, 156),
        fs_block_count: total_blocks,
    })
}

fn inode_reserved_zero(bytes: &[u8; INODE_SIZE]) -> bool {
    get_u16(bytes, 10) == 0 && all_zero(&bytes[160..224])
}

fn inode_indirect_roots_zero(inode: &Inode) -> bool {
    inode.single_indirect_block == 0
        && inode.double_indirect_block == 0
        && inode.triple_indirect_block == 0
}

fn write_dir_entry(
    out: &mut [u8],
    name: &str,
    child_block: u64,
    kind: InodeKind,
) -> Result<(), String> {
    if out.len() != DIRENT_SIZE {
        return Err("directory entry output slice has wrong size".to_string());
    }
    let name = name.as_bytes();
    if name.len() > 47 {
        return Err("filename exceeds 47-byte NeoFS v1 limit".to_string());
    }
    out.fill(0);
    out[..name.len()].copy_from_slice(name);
    put_u64(out, 48, child_block);
    out[56] = kind as u8;
    Ok(())
}

fn read_directory_entries(file: &mut File, inode: &Inode) -> Result<Vec<DirEntry>, String> {
    if inode.kind != InodeKind::Directory {
        return Err("inode is not a directory".to_string());
    }
    if inode.directory_entry_count as usize * DIRENT_SIZE != inode.file_size as usize {
        return Err("directory file_size does not match entry count".to_string());
    }
    if inode.file_size == 0 {
        return Ok(Vec::new());
    }
    let content = read_logical_content(file, inode)?;
    let mut entries = Vec::new();
    for idx in 0..inode.directory_entry_count as usize {
        let entry = &content[idx * DIRENT_SIZE..(idx + 1) * DIRENT_SIZE];
        let name_end = entry[..48]
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("directory entry name is not NUL-terminated")?;
        if !all_zero(&entry[name_end + 1..48]) {
            return Err("directory entry name padding is not zero".to_string());
        }
        if !all_zero(&entry[57..64]) {
            return Err("directory entry reserved fields are not zero".to_string());
        }
        let name = std::str::from_utf8(&entry[..name_end])
            .map_err(|_| "directory entry name is not UTF-8")?
            .to_string();
        let child_block = get_u64(entry, 48);
        let kind = InodeKind::try_from(entry[56])?;
        entries.push(DirEntry {
            name,
            child_block,
            kind,
        });
    }
    Ok(entries)
}

fn verify_content_hash(file: &mut File, inode: &Inode) -> Result<(), String> {
    let content = read_logical_content(file, inode)?;
    let hash = blake3_hash(&content);
    if hash != inode.content_hash {
        return Err(format!(
            "{} inode generation {} has a content hash mismatch",
            inode.kind.name(),
            inode.generation
        ));
    }
    Ok(())
}

fn read_logical_content(file: &mut File, inode: &Inode) -> Result<Vec<u8>, String> {
    if inode.file_size > MAX_FILE_SIZE as u64 {
        return Err("inode exceeds 2 GiB NeoFS file size cap".to_string());
    }
    if inode.block_count != file_data_blocks(inode.file_size)? {
        return Err("inode file_size does not match block_count".to_string());
    }
    let mut out = Vec::with_capacity(inode.file_size as usize);
    let mut remaining = inode.file_size as usize;
    for logical in 0..inode.block_count {
        let block = resolve_logical_block(file, inode, logical)?;
        let take = remaining.min(FS_BLOCK_SIZE as usize);
        let mut buf = vec![0u8; FS_BLOCK_SIZE as usize];
        read_exact_at(file, block_offset(block), &mut buf)?;
        out.extend_from_slice(&buf[..take]);
        remaining -= take;
    }
    if remaining != 0 {
        return Err("inode file_size exceeds populated direct blocks".to_string());
    }
    Ok(out)
}

fn resolve_logical_block(file: &mut File, inode: &Inode, logical: u64) -> Result<u64, String> {
    resolve_logical_block_with(inode, logical, |block| {
        let mut bytes = [0u8; FS_BLOCK_SIZE as usize];
        read_exact_at(file, block_offset(block), &mut bytes)?;
        Ok(bytes)
    })
}

fn resolve_logical_block_with<F>(
    inode: &Inode,
    logical: u64,
    mut read_indirect_block: F,
) -> Result<u64, String>
where
    F: FnMut(u64) -> Result<[u8; FS_BLOCK_SIZE as usize], String>,
{
    if logical >= inode.block_count {
        return Err("logical block index is past inode block_count".to_string());
    }

    let direct = DIRECT_BLOCKS as u64;
    if logical < direct {
        return validate_fs_block(inode.direct_blocks[logical as usize], inode.fs_block_count);
    }

    let pointers = INDIRECT_POINTERS_PER_BLOCK as u64;
    let single_blocks = SINGLE_INDIRECT_BLOCKS as u64;
    let double_blocks = DOUBLE_INDIRECT_BLOCKS as u64;
    let mut index = logical - direct;

    if index < single_blocks {
        return read_pointer_from_indirect(
            &mut read_indirect_block,
            inode.single_indirect_block,
            index,
            inode.fs_block_count,
        );
    }
    index -= single_blocks;

    if index < double_blocks {
        let leaf = read_pointer_from_indirect(
            &mut read_indirect_block,
            inode.double_indirect_block,
            index / pointers,
            inode.fs_block_count,
        )?;
        return read_pointer_from_indirect(
            &mut read_indirect_block,
            leaf,
            index % pointers,
            inode.fs_block_count,
        );
    }
    index -= double_blocks;

    let level2 = read_pointer_from_indirect(
        &mut read_indirect_block,
        inode.triple_indirect_block,
        index / (pointers * pointers),
        inode.fs_block_count,
    )?;
    let level2_index = index % (pointers * pointers);
    let leaf = read_pointer_from_indirect(
        &mut read_indirect_block,
        level2,
        level2_index / pointers,
        inode.fs_block_count,
    )?;
    read_pointer_from_indirect(
        &mut read_indirect_block,
        leaf,
        level2_index % pointers,
        inode.fs_block_count,
    )
}

fn read_pointer_from_indirect<F>(
    read_indirect_block: &mut F,
    block: u64,
    index: u64,
    fs_block_count: u64,
) -> Result<u64, String>
where
    F: FnMut(u64) -> Result<[u8; FS_BLOCK_SIZE as usize], String>,
{
    validate_fs_block(block, fs_block_count)?;
    let bytes = read_indirect_block(block)?;
    pointer_from_indirect_bytes(&bytes, index, fs_block_count)
}

fn pointer_from_indirect_bytes(
    bytes: &[u8; FS_BLOCK_SIZE as usize],
    index: u64,
    fs_block_count: u64,
) -> Result<u64, String> {
    if index >= INDIRECT_POINTERS_PER_BLOCK as u64 {
        return Err("indirect pointer index is out of range".to_string());
    }
    let offset = index as usize * std::mem::size_of::<u64>();
    let mut pointer = [0u8; 8];
    pointer.copy_from_slice(&bytes[offset..offset + 8]);
    validate_fs_block(u64::from_le_bytes(pointer), fs_block_count)
}

fn validate_indirect_root_blocks(
    block_count: u64,
    total_blocks: u64,
    single: u64,
    double: u64,
    triple: u64,
) -> Result<(), String> {
    let direct = DIRECT_BLOCKS as u64;
    let single_end = direct + SINGLE_INDIRECT_BLOCKS as u64;
    let double_end = single_end + DOUBLE_INDIRECT_BLOCKS as u64;

    validate_indirect_root(single, block_count > direct, total_blocks)?;
    validate_indirect_root(double, block_count > single_end, total_blocks)?;
    validate_indirect_root(triple, block_count > double_end, total_blocks)?;
    Ok(())
}

fn validate_indirect_root(block: u64, required: bool, total_blocks: u64) -> Result<(), String> {
    if required {
        validate_fs_block(block, total_blocks)?;
    } else if block != 0 {
        return Err("unused indirect root block is not zero".to_string());
    }
    Ok(())
}

fn validate_fs_block(block: u64, fs_block_count: u64) -> Result<u64, String> {
    if block == 0 || block >= fs_block_count {
        return Err("NeoFS block pointer is out of range".to_string());
    }
    Ok(block)
}

pub fn image_hash(path: &Path) -> Result<[u8; 32], String> {
    let mut file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(*hasher.finalize().as_bytes())
}

pub fn hex_hash(hash: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in hash {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn model_content() -> Vec<u8> {
    const HEADER_OFF: usize = 0;
    const TENSOR_OFF: usize = 96;
    const NODE_OFF: usize = 192;
    const IO_OFF: usize = 248;
    const STRING_OFF: usize = 408;
    const WEIGHT_OFF: usize = 4096;
    const TOTAL: usize = 4100;

    let mut bytes = vec![0u8; TOTAL];
    let strings = b"weightinputnodeoutput";
    bytes[STRING_OFF..STRING_OFF + strings.len()].copy_from_slice(strings);
    bytes[WEIGHT_OFF..WEIGHT_OFF + 4].copy_from_slice(&1.0f32.to_le_bytes());

    bytes[0..4].copy_from_slice(b"NEOM");
    put_u32(&mut bytes, HEADER_OFF + 4, 1);
    put_u32(&mut bytes, HEADER_OFF + 8, 96);
    put_u32(&mut bytes, HEADER_OFF + 16, 1);
    put_u32(&mut bytes, HEADER_OFF + 20, 1);
    put_u32(&mut bytes, HEADER_OFF + 24, 1);
    put_u32(&mut bytes, HEADER_OFF + 28, 1);
    put_u64(&mut bytes, HEADER_OFF + 32, TENSOR_OFF as u64);
    put_u64(&mut bytes, HEADER_OFF + 40, NODE_OFF as u64);
    put_u64(&mut bytes, HEADER_OFF + 48, IO_OFF as u64);
    put_u64(&mut bytes, HEADER_OFF + 56, STRING_OFF as u64);
    put_u64(&mut bytes, HEADER_OFF + 64, strings.len() as u64);
    put_u64(&mut bytes, HEADER_OFF + 72, WEIGHT_OFF as u64);
    put_u64(&mut bytes, HEADER_OFF + 80, 4);
    put_u64(&mut bytes, HEADER_OFF + 88, TOTAL as u64);

    put_u32(&mut bytes, TENSOR_OFF, 0);
    put_u32(&mut bytes, TENSOR_OFF + 4, 1);
    put_u64(&mut bytes, TENSOR_OFF + 8, 1);
    put_u64(&mut bytes, TENSOR_OFF + 72, WEIGHT_OFF as u64);
    put_u64(&mut bytes, TENSOR_OFF + 80, 4);
    put_u32(&mut bytes, TENSOR_OFF + 88, 0);
    put_u32(&mut bytes, TENSOR_OFF + 92, 6);

    put_u32(&mut bytes, NODE_OFF, 0);
    put_u32(&mut bytes, NODE_OFF + 4, 1);
    put_u32(&mut bytes, NODE_OFF + 8, 1);
    put_u32(&mut bytes, NODE_OFF + 12, (NODE_OFF + 48) as u32);
    put_u32(&mut bytes, NODE_OFF + 16, (NODE_OFF + 52) as u32);
    put_u32(&mut bytes, NODE_OFF + 28, 11);
    put_u32(&mut bytes, NODE_OFF + 32, 4);
    put_u32(&mut bytes, NODE_OFF + 48, 1);
    put_u32(&mut bytes, NODE_OFF + 52, 2);

    put_u32(&mut bytes, IO_OFF, 1);
    put_u32(&mut bytes, IO_OFF + 4, 0);
    put_u32(&mut bytes, IO_OFF + 8, 1);
    put_u64(&mut bytes, IO_OFF + 16, 1);
    put_u32(&mut bytes, IO_OFF + 80, 2);
    put_u32(&mut bytes, IO_OFF + 84, 0);
    put_u32(&mut bytes, IO_OFF + 88, 1);
    put_u32(&mut bytes, IO_OFF + 92, 1);
    put_u64(&mut bytes, IO_OFF + 96, 1);
    bytes
}

fn allocate_file_block_layout(
    next_block: &mut u64,
    block_count: u64,
) -> Result<FileBlockLayout, String> {
    if block_count > MAX_FILE_BLOCKS {
        return Err(format!(
            "content requires {block_count} blocks, exceeding 2 GiB NeoFS file cap"
        ));
    }
    let data_start = *next_block;
    *next_block = next_block
        .checked_add(block_count)
        .ok_or("file data block allocation overflow")?;
    let data_blocks = (0..block_count)
        .map(|idx| data_start + idx)
        .collect::<Vec<_>>();

    let mut direct_blocks = [0u64; DIRECT_BLOCKS];
    for (idx, slot) in direct_blocks
        .iter_mut()
        .enumerate()
        .take((block_count as usize).min(DIRECT_BLOCKS))
    {
        *slot = data_blocks[idx];
    }

    let mut indirect_blocks = Vec::new();
    let mut single_indirect_block = 0;
    let mut double_indirect_block = 0;
    let mut triple_indirect_block = 0;
    let mut cursor = DIRECT_BLOCKS as u64;

    let single_count = block_count
        .saturating_sub(cursor)
        .min(SINGLE_INDIRECT_BLOCKS as u64);
    if single_count != 0 {
        let root = allocate_indirect_block(next_block)?;
        let mut bytes = [0u8; FS_BLOCK_SIZE as usize];
        for idx in 0..single_count {
            put_indirect_pointer(&mut bytes, idx, data_blocks[(cursor + idx) as usize])?;
        }
        indirect_blocks.push((root, bytes));
        single_indirect_block = root;
        cursor += single_count;
    }

    let double_count = block_count
        .saturating_sub(cursor)
        .min(DOUBLE_INDIRECT_BLOCKS as u64);
    if double_count != 0 {
        let root = allocate_indirect_block(next_block)?;
        let leaves = allocate_indirect_tree(
            next_block,
            2,
            &data_blocks[cursor as usize..(cursor + double_count) as usize],
        )?;
        let mut root_bytes = [0u8; FS_BLOCK_SIZE as usize];
        for (idx, (leaf_block, leaf_bytes)) in leaves.into_iter().enumerate() {
            put_indirect_pointer(&mut root_bytes, idx as u64, leaf_block)?;
            indirect_blocks.push((leaf_block, leaf_bytes));
        }
        indirect_blocks.push((root, root_bytes));
        double_indirect_block = root;
        cursor += double_count;
    }

    let triple_count = block_count.saturating_sub(cursor);
    if triple_count != 0 {
        let root = allocate_indirect_block(next_block)?;
        let level2 = allocate_triple_indirect_tree(
            next_block,
            &data_blocks[cursor as usize..(cursor + triple_count) as usize],
        )?;
        let mut root_bytes = [0u8; FS_BLOCK_SIZE as usize];
        for (idx, (level2_block, level2_bytes, leaves)) in level2.into_iter().enumerate() {
            put_indirect_pointer(&mut root_bytes, idx as u64, level2_block)?;
            indirect_blocks.push((level2_block, level2_bytes));
            indirect_blocks.extend(leaves);
        }
        indirect_blocks.push((root, root_bytes));
        triple_indirect_block = root;
    }

    Ok(FileBlockLayout {
        direct_blocks,
        single_indirect_block,
        double_indirect_block,
        triple_indirect_block,
        data_blocks,
        indirect_blocks,
    })
}

fn allocate_indirect_tree(
    next_block: &mut u64,
    depth: u8,
    data_blocks: &[u64],
) -> Result<Vec<(u64, [u8; FS_BLOCK_SIZE as usize])>, String> {
    if depth != 2 {
        return Err("unsupported indirect allocation depth".to_string());
    }
    let mut leaves = Vec::new();
    for chunk in data_blocks.chunks(INDIRECT_POINTERS_PER_BLOCK) {
        let leaf = allocate_indirect_block(next_block)?;
        let mut leaf_bytes = [0u8; FS_BLOCK_SIZE as usize];
        for (idx, data_block) in chunk.iter().copied().enumerate() {
            put_indirect_pointer(&mut leaf_bytes, idx as u64, data_block)?;
        }
        leaves.push((leaf, leaf_bytes));
    }
    Ok(leaves)
}

type TripleLevel2 = (
    u64,
    [u8; FS_BLOCK_SIZE as usize],
    Vec<(u64, [u8; FS_BLOCK_SIZE as usize])>,
);

fn allocate_triple_indirect_tree(
    next_block: &mut u64,
    data_blocks: &[u64],
) -> Result<Vec<TripleLevel2>, String> {
    let mut level2_blocks = Vec::new();
    let span = DOUBLE_INDIRECT_BLOCKS;
    for level2_chunk in data_blocks.chunks(span) {
        let level2 = allocate_indirect_block(next_block)?;
        let leaves = allocate_indirect_tree(next_block, 2, level2_chunk)?;
        let mut level2_bytes = [0u8; FS_BLOCK_SIZE as usize];
        for (idx, (leaf_block, _)) in leaves.iter().enumerate() {
            put_indirect_pointer(&mut level2_bytes, idx as u64, *leaf_block)?;
        }
        level2_blocks.push((level2, level2_bytes, leaves));
    }
    Ok(level2_blocks)
}

fn allocate_indirect_block(next_block: &mut u64) -> Result<u64, String> {
    let block = *next_block;
    *next_block = next_block
        .checked_add(1)
        .ok_or("indirect block allocation overflow")?;
    Ok(block)
}

fn put_indirect_pointer(
    block: &mut [u8; FS_BLOCK_SIZE as usize],
    index: u64,
    value: u64,
) -> Result<(), String> {
    if index >= INDIRECT_POINTERS_PER_BLOCK as u64 {
        return Err("indirect pointer index overflow".to_string());
    }
    let offset = index as usize * std::mem::size_of::<u64>();
    block[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn data_block_count(len: usize) -> Result<u64, String> {
    file_data_blocks(len as u64)
}

fn file_data_blocks(len: u64) -> Result<u64, String> {
    if len == 0 {
        return Ok(0);
    }
    if len > MAX_FILE_SIZE as u64 {
        return Err("content exceeds 2 GiB NeoFS file size cap".to_string());
    }
    let blocks = len
        .checked_add(FS_BLOCK_SIZE as u64 - 1)
        .map(|value| value / FS_BLOCK_SIZE as u64)
        .ok_or("file block count overflow")?;
    if blocks > MAX_FILE_BLOCKS {
        return Err("content exceeds 2 GiB NeoFS file size cap".to_string());
    }
    let addressable_blocks = DIRECT_BLOCKS as u64
        + SINGLE_INDIRECT_BLOCKS as u64
        + DOUBLE_INDIRECT_BLOCKS as u64
        + TRIPLE_INDIRECT_BLOCKS as u64;
    if blocks > addressable_blocks {
        return Err("content exceeds NeoFS indirect block address capacity".to_string());
    }
    Ok(blocks)
}

fn data_blocks(content: &[u8]) -> Result<Vec<[u8; FS_BLOCK_SIZE as usize]>, String> {
    let block_count = data_block_count(content.len())? as usize;
    let mut blocks = Vec::with_capacity(block_count);
    for idx in 0..block_count {
        let start = idx * FS_BLOCK_SIZE as usize;
        let end = (start + FS_BLOCK_SIZE as usize).min(content.len());
        let mut block = [0u8; FS_BLOCK_SIZE as usize];
        block[..end - start].copy_from_slice(&content[start..end]);
        blocks.push(block);
    }
    Ok(blocks)
}

fn data_block(content: &[u8]) -> Result<[u8; FS_BLOCK_SIZE as usize], String> {
    if content.len() > FS_BLOCK_SIZE as usize {
        return Err("content exceeds one BUILD-2 direct data block".to_string());
    }
    let mut out = [0u8; FS_BLOCK_SIZE as usize];
    out[..content.len()].copy_from_slice(content);
    Ok(out)
}

fn block_with_prefix<const N: usize>(prefix: &[u8; N]) -> [u8; FS_BLOCK_SIZE as usize] {
    assert!(N <= FS_BLOCK_SIZE as usize);
    let mut block = [0u8; FS_BLOCK_SIZE as usize];
    block[..N].copy_from_slice(prefix);
    block
}

fn read_inode_bytes(file: &mut File, block: u64) -> Result<[u8; INODE_SIZE], String> {
    let mut block_bytes = [0u8; FS_BLOCK_SIZE as usize];
    read_exact_at(file, block_offset(block), &mut block_bytes)?;
    let mut inode = [0u8; INODE_SIZE];
    inode.copy_from_slice(&block_bytes[..INODE_SIZE]);
    Ok(inode)
}

fn read_exact_at(file: &mut File, offset: u64, buf: &mut [u8]) -> Result<(), String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("seek offset {offset}: {e}"))?;
    file.read_exact(buf)
        .map_err(|e| format!("read offset {offset}: {e}"))
}

fn write_all_at(file: &mut File, offset: u64, buf: &[u8]) -> Result<(), String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("seek offset {offset}: {e}"))?;
    file.write_all(buf)
        .map_err(|e| format!("write offset {offset}: {e}"))
}

fn corrupt_primary_superblock(path: &Path) -> Result<(), String> {
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut checksum_byte = [0u8; 1];
    read_exact_at(&mut file, 480, &mut checksum_byte)?;
    checksum_byte[0] ^= 0xff;
    write_all_at(&mut file, 480, &checksum_byte)?;
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))
}

fn make_highest_generation_variant(path: &Path) -> Result<(), String> {
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    rewrite_superblock_generation(&mut file, SUPERBLOCK_COPY_OFFSETS[1], 2, 6)?;
    rewrite_superblock_generation(&mut file, SUPERBLOCK_COPY_OFFSETS[2], 3, 7)?;
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))
}

fn make_reserved_type_variant(path: &Path) -> Result<(), String> {
    const RESERVED_TYPE_ID: u8 = 0x7f;
    const RESERVED_ENTRY_INDEX: usize = 2;

    let mut file = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    let image_size = file
        .metadata()
        .map_err(|e| format!("stat {}: {e}", path.display()))?
        .len();
    let copy_offsets = superblock_copy_offsets(image_size)?;
    let mut superblock_bytes = [0u8; SUPERBLOCK_SIZE];
    read_exact_at(&mut file, copy_offsets[0], &mut superblock_bytes)?;
    let superblock = read_superblock(&superblock_bytes)?;

    let mut root_inode = read_inode_bytes(&mut file, superblock.root_inode_block)?;
    let root = read_inode(&root_inode, superblock.total_blocks)?;
    if root.kind != InodeKind::Directory || root.block_count != 1 {
        return Err("reserved-type variant requires the deterministic root directory".to_string());
    }
    if root.directory_entry_count <= RESERVED_ENTRY_INDEX as u32 {
        return Err("reserved-type variant requires a third root entry".to_string());
    }

    let mut root_dir_data = [0u8; FS_BLOCK_SIZE as usize];
    read_exact_at(
        &mut file,
        block_offset(root.direct_blocks[0]),
        &mut root_dir_data,
    )?;
    root_dir_data[RESERVED_ENTRY_INDEX * DIRENT_SIZE + 56] = RESERVED_TYPE_ID;
    write_all_at(
        &mut file,
        block_offset(root.direct_blocks[0]),
        &root_dir_data,
    )?;

    let root_content_len = root.file_size as usize;
    root_inode[104..136].copy_from_slice(&blake3_hash(&root_dir_data[..root_content_len]));
    let inode_checksum = blake3_hash(&root_inode[..224]);
    root_inode[224..256].copy_from_slice(&inode_checksum);
    let root_inode_block = block_with_prefix(&root_inode);
    write_all_at(
        &mut file,
        block_offset(superblock.root_inode_block),
        &root_inode_block,
    )?;

    let root_inode_hash = blake3_hash(&root_inode);
    for offset in copy_offsets {
        let mut bytes = [0u8; SUPERBLOCK_SIZE];
        read_exact_at(&mut file, offset, &mut bytes)?;
        read_superblock(&bytes)?;
        bytes[48..80].copy_from_slice(&root_inode_hash);
        let checksum = blake3_hash(&bytes[..480]);
        bytes[480..512].copy_from_slice(&checksum);
        write_all_at(&mut file, offset, &bytes)?;
    }

    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))
}

/// `shared-inode` and `doubled-block`: copies 0 and 1 keep the image as
/// made; copy 2 gets a newer generation whose root adds one entry. For
/// `shared-inode` the entry names `hello.txt`'s inode. For `doubled-block`
/// it names a new inode whose two direct blocks are one block D. Every
/// other check mount makes passes: the hashes, the checksums, and the free
/// map (the new blocks sit above the old `next_free_block`, where the map
/// marks nothing free).
fn make_twice_named_variant(path: &Path, doubled_block: bool) -> Result<(), String> {
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    let image_size = file
        .metadata()
        .map_err(|e| format!("stat {}: {e}", path.display()))?
        .len();
    let copy_offsets = superblock_copy_offsets(image_size)?;
    let mut superblock_bytes = [0u8; SUPERBLOCK_SIZE];
    read_exact_at(&mut file, copy_offsets[0], &mut superblock_bytes)?;
    let mut superblock = read_superblock(&superblock_bytes)?;
    let root = read_inode(
        &read_inode_bytes(&mut file, superblock.root_inode_block)?,
        superblock.total_blocks,
    )?;
    let entries = read_directory_entries(&mut file, &root)?;
    let hello = entries
        .iter()
        .find(|e| e.name == "hello.txt")
        .ok_or("twice-named variants need hello.txt")?
        .child_block;

    let mut next = superblock.next_free_block;
    let mut take = || {
        let b = next;
        next += 1;
        b
    };
    let (name, child) = if doubled_block {
        let data = take();
        let inode = take();
        let block = block_with_prefix(b"doubled block D\n");
        write_all_at(&mut file, block_offset(data), &block)?;
        let mut content = block.to_vec();
        content.extend_from_slice(&block);
        let mut direct = [0u64; DIRECT_BLOCKS];
        direct[0] = data;
        direct[1] = data;
        let bytes = write_inode(InodeWrite {
            kind: InodeKind::Regular,
            generation: superblock.next_generation,
            file_size: content.len() as u64,
            direct_blocks: direct,
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            block_count: 2,
            content_hash: blake3_hash(&content),
            directory_entry_count: 0,
        });
        write_all_at(&mut file, block_offset(inode), &block_with_prefix(&bytes))?;
        ("doubled.txt", inode)
    } else {
        ("twin.txt", hello)
    };

    let dir_block = take();
    let root_block = take();
    let count = entries.len() + 1;
    let mut dir = [0u8; FS_BLOCK_SIZE as usize];
    for (i, entry) in entries.iter().enumerate() {
        write_dir_entry(
            &mut dir[i * DIRENT_SIZE..(i + 1) * DIRENT_SIZE],
            &entry.name,
            entry.child_block,
            entry.kind,
        )?;
    }
    write_dir_entry(
        &mut dir[(count - 1) * DIRENT_SIZE..count * DIRENT_SIZE],
        name,
        child,
        InodeKind::Regular,
    )?;
    write_all_at(&mut file, block_offset(dir_block), &dir)?;
    let mut direct = [0u64; DIRECT_BLOCKS];
    direct[0] = dir_block;
    let root_bytes = write_inode(InodeWrite {
        kind: InodeKind::Directory,
        generation: superblock.next_generation + 1,
        file_size: (count * DIRENT_SIZE) as u64,
        direct_blocks: direct,
        single_indirect_block: 0,
        double_indirect_block: 0,
        triple_indirect_block: 0,
        block_count: 1,
        content_hash: blake3_hash(&dir[..count * DIRENT_SIZE]),
        directory_entry_count: count as u32,
    });
    write_all_at(
        &mut file,
        block_offset(root_block),
        &block_with_prefix(&root_bytes),
    )?;

    superblock.generation += 1;
    superblock.root_inode_block = root_block;
    superblock.root_inode_hash = blake3_hash(&root_bytes);
    superblock.next_free_block = next;
    superblock.next_generation += 2;
    let newer = write_superblock(superblock);
    write_all_at(&mut file, copy_offsets[2], &newer)?;
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))
}

fn make_corrupt_free_map_variant(path: &Path) -> Result<(), String> {
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    let image_size = file
        .metadata()
        .map_err(|e| format!("stat {}: {e}", path.display()))?
        .len();
    let copy_offsets = superblock_copy_offsets(image_size)?;
    let mut superblock_bytes = [0u8; SUPERBLOCK_SIZE];
    read_exact_at(&mut file, copy_offsets[0], &mut superblock_bytes)?;
    let mut superblock = read_superblock(&superblock_bytes)?;
    let free_map = superblock
        .free_map
        .clone()
        .ok_or("corrupt-free-map variant requires BUILD-3 free-map metadata")?;
    let mut new_map = read_free_map_bytes(&mut file, &free_map)?;
    let new_map_block = superblock.next_free_block;
    if new_map_block + free_map.block_count > superblock.total_blocks {
        return Err("corrupt-free-map variant has no room for alternate map".to_string());
    }
    for idx in 0..free_map.block_count {
        let start = usize::try_from(idx)
            .ok()
            .and_then(|idx| idx.checked_mul(FS_BLOCK_SIZE as usize))
            .ok_or("free-map variant offset overflow")?;
        write_all_at(
            &mut file,
            block_offset(new_map_block + idx),
            &new_map[start..start + FS_BLOCK_SIZE as usize],
        )?;
    }

    superblock.generation += 1;
    superblock.next_free_block += free_map.block_count;
    superblock.next_generation += 1;
    superblock.free_map = Some(FreeMapRef {
        start_block: new_map_block,
        block_count: free_map.block_count,
        hash: blake3_hash(&new_map),
    });
    let newer = write_superblock(superblock);
    write_all_at(&mut file, copy_offsets[2], &newer)?;

    new_map[0] ^= 0x01;
    write_all_at(
        &mut file,
        block_offset(new_map_block),
        &new_map[..FS_BLOCK_SIZE as usize],
    )?;
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))
}

fn rewrite_superblock_generation(
    file: &mut File,
    offset: u64,
    generation: u64,
    next_generation: u32,
) -> Result<(), String> {
    let mut bytes = [0u8; SUPERBLOCK_SIZE];
    read_exact_at(file, offset, &mut bytes)?;
    read_superblock(&bytes)?;
    put_u64(&mut bytes, 32, generation);
    put_u32(&mut bytes, 84, next_generation);
    let checksum = blake3_hash(&bytes[..480]);
    bytes[480..512].copy_from_slice(&checksum);
    write_all_at(file, offset, &bytes)
}

fn block_offset(block: u64) -> u64 {
    block * FS_BLOCK_SIZE as u64
}

fn blake3_hash(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn put_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(buf: &mut [u8], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(buf: &[u8], offset: usize) -> u16 {
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&buf[offset..offset + 2]);
    u16::from_le_bytes(bytes)
}

fn get_u32(buf: &[u8], offset: usize) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buf[offset..offset + 4]);
    u32::from_le_bytes(bytes)
}

fn get_u64(buf: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    const LARGE_TEST_BLOCKS: usize = DIRECT_BLOCKS + SINGLE_INDIRECT_BLOCKS + 3;
    const LARGE_TEST_BYTES: usize = LARGE_TEST_BLOCKS * FS_BLOCK_SIZE as usize + 137;

    #[test]
    fn hand_laid_inode_offsets_and_boundary_walk() {
        let triple_boundary =
            DIRECT_BLOCKS as u64 + SINGLE_INDIRECT_BLOCKS as u64 + DOUBLE_INDIRECT_BLOCKS as u64;
        let block_count = triple_boundary + 1;
        let file_size = block_count * FS_BLOCK_SIZE as u64;
        let mut bytes = [0u8; INODE_SIZE];
        bytes[0..8].copy_from_slice(INODE_MAGIC);
        bytes[8] = InodeKind::Model as u8;
        bytes[9] = TYPE_VERSION;
        put_u32(&mut bytes, 12, 9);
        put_u64(&mut bytes, 16, file_size);
        put_u64(&mut bytes, 24, block_count);
        for idx in 0..DIRECT_BLOCKS {
            put_u64(&mut bytes, 32 + idx * 8, 100 + idx as u64);
        }
        put_u64(&mut bytes, 96, 1_000);
        bytes[104..136].copy_from_slice(&[0x5a; 32]);
        put_u64(&mut bytes, 136, 2_000);
        put_u64(&mut bytes, 144, 3_000);
        put_u32(&mut bytes, 152, InodeKind::Model.rights());
        let checksum = blake3_hash(&bytes[..224]);
        bytes[224..256].copy_from_slice(&checksum);

        assert_eq!(get_u64(&bytes, 96), 1_000);
        assert_eq!(&bytes[104..136], &[0x5a; 32]);
        assert_eq!(get_u64(&bytes, 136), 2_000);
        assert_eq!(get_u64(&bytes, 144), 3_000);

        let inode = read_inode(&bytes, 500_000).expect("hand-laid inode parses");
        let mut blocks = BTreeMap::new();
        let mut single = [0u8; FS_BLOCK_SIZE as usize];
        put_indirect_pointer(&mut single, 0, 108).unwrap();
        put_indirect_pointer(&mut single, INDIRECT_POINTERS_PER_BLOCK as u64 - 1, 619).unwrap();
        blocks.insert(1_000, single);

        let mut double_root = [0u8; FS_BLOCK_SIZE as usize];
        put_indirect_pointer(&mut double_root, 0, 2_001).unwrap();
        blocks.insert(2_000, double_root);
        let mut double_leaf = [0u8; FS_BLOCK_SIZE as usize];
        put_indirect_pointer(&mut double_leaf, 0, 620).unwrap();
        blocks.insert(2_001, double_leaf);

        let mut triple_root = [0u8; FS_BLOCK_SIZE as usize];
        put_indirect_pointer(&mut triple_root, 0, 3_001).unwrap();
        blocks.insert(3_000, triple_root);
        let mut triple_level2 = [0u8; FS_BLOCK_SIZE as usize];
        put_indirect_pointer(&mut triple_level2, 0, 3_002).unwrap();
        blocks.insert(3_001, triple_level2);
        let mut triple_leaf = [0u8; FS_BLOCK_SIZE as usize];
        put_indirect_pointer(&mut triple_leaf, 0, 300_000).unwrap();
        blocks.insert(3_002, triple_leaf);

        let read = |block| {
            blocks
                .get(&block)
                .copied()
                .ok_or_else(|| format!("missing test pointer block {block}"))
        };
        assert_eq!(resolve_logical_block_with(&inode, 7, read).unwrap(), 107);

        let read = |block| {
            blocks
                .get(&block)
                .copied()
                .ok_or_else(|| format!("missing test pointer block {block}"))
        };
        assert_eq!(
            resolve_logical_block_with(&inode, DIRECT_BLOCKS as u64, read).unwrap(),
            108
        );

        let read = |block| {
            blocks
                .get(&block)
                .copied()
                .ok_or_else(|| format!("missing test pointer block {block}"))
        };
        assert_eq!(
            resolve_logical_block_with(
                &inode,
                DIRECT_BLOCKS as u64 + SINGLE_INDIRECT_BLOCKS as u64 - 1,
                read
            )
            .unwrap(),
            619
        );

        let read = |block| {
            blocks
                .get(&block)
                .copied()
                .ok_or_else(|| format!("missing test pointer block {block}"))
        };
        assert_eq!(
            resolve_logical_block_with(
                &inode,
                DIRECT_BLOCKS as u64 + SINGLE_INDIRECT_BLOCKS as u64,
                read
            )
            .unwrap(),
            620
        );

        let read = |block| {
            blocks
                .get(&block)
                .copied()
                .ok_or_else(|| format!("missing test pointer block {block}"))
        };
        assert_eq!(
            resolve_logical_block_with(&inode, triple_boundary, read).unwrap(),
            300_000
        );
    }

    #[test]
    fn hand_laid_truncate_oracle_reclaims_indirect_boundaries() {
        let triple_boundary =
            DIRECT_BLOCKS as u64 + SINGLE_INDIRECT_BLOCKS as u64 + DOUBLE_INDIRECT_BLOCKS as u64;
        let original_blocks = triple_boundary + 2;
        let mut next_block = 1_000;
        let layout =
            allocate_file_block_layout(&mut next_block, original_blocks).expect("layout builds");
        let inode = inode_from_layout(
            &layout,
            original_blocks,
            original_blocks * FS_BLOCK_SIZE as u64,
        );
        let indirects = indirect_map(&layout);

        assert_ne!(inode.single_indirect_block, 0);
        assert_ne!(inode.double_indirect_block, 0);
        assert_ne!(inode.triple_indirect_block, 0);
        assert_eq!(
            resolve_from_map(&inode, &indirects, triple_boundary).unwrap(),
            layout.data_blocks[triple_boundary as usize]
        );

        let original_live =
            collect_file_blocks_for_prefix(&inode, &indirects, original_blocks).unwrap();
        assert_eq!(
            original_live.len(),
            original_blocks as usize + layout.indirect_blocks.len()
        );

        let cases = [
            ("zero", 0usize),
            ("mid-direct", FS_BLOCK_SIZE as usize / 2),
            (
                "mid-single",
                (DIRECT_BLOCKS + 3) * FS_BLOCK_SIZE as usize + 17,
            ),
            (
                "exact-boundary",
                (DIRECT_BLOCKS + SINGLE_INDIRECT_BLOCKS) * FS_BLOCK_SIZE as usize,
            ),
            (
                "boundary-plus-one",
                (DIRECT_BLOCKS + SINGLE_INDIRECT_BLOCKS) * FS_BLOCK_SIZE as usize + 1,
            ),
        ];

        for (label, new_len) in cases {
            let new_blocks = file_data_blocks(new_len as u64).unwrap();
            let kept = collect_file_blocks_for_prefix(&inode, &indirects, new_blocks).unwrap();
            assert!(
                kept.is_subset(&original_live),
                "{label}: truncate kept a block outside the original map"
            );
            assert_surviving_byte_samples(
                label,
                &inode,
                &indirects,
                &layout.data_blocks,
                &kept,
                new_len,
            );

            let mut freed = BTreeSet::new();
            for block in original_live.difference(&kept) {
                assert!(
                    freed.insert(*block),
                    "{label}: double-free candidate {block}"
                );
            }
            assert_eq!(
                original_live.len(),
                kept.len() + freed.len(),
                "{label}: released blocks did not return to the free-count oracle"
            );
            assert_eq!(read_len_after_truncate(new_len, new_len, 32), 0);
        }
    }

    fn inode_from_layout(layout: &FileBlockLayout, block_count: u64, file_size: u64) -> Inode {
        Inode {
            kind: InodeKind::Regular,
            generation: 1,
            file_size,
            block_count,
            direct_blocks: layout.direct_blocks,
            single_indirect_block: layout.single_indirect_block,
            double_indirect_block: layout.double_indirect_block,
            triple_indirect_block: layout.triple_indirect_block,
            content_hash: [0; 32],
            directory_entry_count: 0,
            fs_block_count: 1_000_000,
        }
    }

    fn indirect_map(layout: &FileBlockLayout) -> BTreeMap<u64, [u8; FS_BLOCK_SIZE as usize]> {
        layout.indirect_blocks.iter().copied().collect()
    }

    fn resolve_from_map(
        inode: &Inode,
        indirects: &BTreeMap<u64, [u8; FS_BLOCK_SIZE as usize]>,
        logical: u64,
    ) -> Result<u64, String> {
        resolve_logical_block_with(inode, logical, |block| {
            indirects
                .get(&block)
                .copied()
                .ok_or_else(|| format!("missing indirect block {block}"))
        })
    }

    fn collect_file_blocks_for_prefix(
        inode: &Inode,
        indirects: &BTreeMap<u64, [u8; FS_BLOCK_SIZE as usize]>,
        covered_blocks: u64,
    ) -> Result<BTreeSet<u64>, String> {
        if covered_blocks > inode.block_count {
            return Err("truncate prefix exceeds inode block count".to_string());
        }
        let mut live = BTreeSet::new();
        for logical in 0..covered_blocks {
            live.insert(resolve_from_map(inode, indirects, logical)?);
        }

        let mut remaining = covered_blocks.saturating_sub(DIRECT_BLOCKS as u64);
        let single_used = remaining.min(SINGLE_INDIRECT_BLOCKS as u64);
        if single_used != 0 {
            collect_indirect_blocks_for_prefix(
                inode.single_indirect_block,
                1,
                single_used,
                inode.fs_block_count,
                indirects,
                &mut live,
            )?;
            remaining -= single_used;
        }

        let double_used = remaining.min(DOUBLE_INDIRECT_BLOCKS as u64);
        if double_used != 0 {
            collect_indirect_blocks_for_prefix(
                inode.double_indirect_block,
                2,
                double_used,
                inode.fs_block_count,
                indirects,
                &mut live,
            )?;
            remaining -= double_used;
        }

        if remaining != 0 {
            collect_indirect_blocks_for_prefix(
                inode.triple_indirect_block,
                3,
                remaining,
                inode.fs_block_count,
                indirects,
                &mut live,
            )?;
        }
        Ok(live)
    }

    fn collect_indirect_blocks_for_prefix(
        block: u64,
        depth: u8,
        covered_data_blocks: u64,
        fs_block_count: u64,
        indirects: &BTreeMap<u64, [u8; FS_BLOCK_SIZE as usize]>,
        live: &mut BTreeSet<u64>,
    ) -> Result<(), String> {
        if covered_data_blocks == 0 {
            return Ok(());
        }
        validate_fs_block(block, fs_block_count)?;
        live.insert(block);
        if depth == 1 {
            return Ok(());
        }

        let bytes = indirects
            .get(&block)
            .ok_or_else(|| format!("missing indirect block {block}"))?;
        let child_span = match depth - 1 {
            1 => INDIRECT_POINTERS_PER_BLOCK as u64,
            2 => DOUBLE_INDIRECT_BLOCKS as u64,
            _ => return Err("invalid indirect depth".to_string()),
        };
        let mut remaining = covered_data_blocks;
        let mut slot = 0u64;
        while remaining != 0 {
            let child = pointer_from_indirect_bytes(bytes, slot, fs_block_count)?;
            let child_blocks = remaining.min(child_span);
            collect_indirect_blocks_for_prefix(
                child,
                depth - 1,
                child_blocks,
                fs_block_count,
                indirects,
                live,
            )?;
            remaining -= child_blocks;
            slot += 1;
        }
        Ok(())
    }

    fn assert_surviving_byte_samples(
        label: &str,
        inode: &Inode,
        indirects: &BTreeMap<u64, [u8; FS_BLOCK_SIZE as usize]>,
        original_data_blocks: &[u64],
        kept: &BTreeSet<u64>,
        new_len: usize,
    ) {
        if new_len == 0 {
            return;
        }
        let single_boundary = (DIRECT_BLOCKS + SINGLE_INDIRECT_BLOCKS) * FS_BLOCK_SIZE as usize;
        let candidates = [
            0,
            new_len - 1,
            FS_BLOCK_SIZE as usize - 1,
            FS_BLOCK_SIZE as usize,
            DIRECT_BLOCKS * FS_BLOCK_SIZE as usize - 1,
            DIRECT_BLOCKS * FS_BLOCK_SIZE as usize,
            single_boundary.saturating_sub(1),
            single_boundary,
        ];
        for offset in candidates.into_iter().filter(|offset| *offset < new_len) {
            let logical = offset / FS_BLOCK_SIZE as usize;
            let in_block = offset % FS_BLOCK_SIZE as usize;
            let resolved = resolve_from_map(inode, indirects, logical as u64).unwrap();
            assert!(
                kept.contains(&resolved),
                "{label}: surviving byte resolves to released block {resolved}"
            );
            let expected_block = original_data_blocks[logical];
            let expected = oracle_byte_for_block(expected_block, in_block);
            let got = oracle_byte_at(inode, indirects, offset).unwrap();
            assert_eq!(got, expected, "{label}: surviving byte changed at {offset}");
        }
    }

    fn oracle_byte_at(
        inode: &Inode,
        indirects: &BTreeMap<u64, [u8; FS_BLOCK_SIZE as usize]>,
        offset: usize,
    ) -> Result<u8, String> {
        let logical = (offset / FS_BLOCK_SIZE as usize) as u64;
        let in_block = offset % FS_BLOCK_SIZE as usize;
        let block = resolve_from_map(inode, indirects, logical)?;
        Ok(oracle_byte_for_block(block, in_block))
    }

    fn oracle_byte_for_block(block: u64, in_block: usize) -> u8 {
        ((block as u32 * 31 + in_block as u32 * 17 + 0xa5) & 0xff) as u8
    }

    fn read_len_after_truncate(file_len: usize, offset: usize, out_len: usize) -> usize {
        if out_len == 0 || offset >= file_len {
            0
        } else {
            (file_len - offset).min(out_len)
        }
    }

    #[test]
    fn direct_only_image_keeps_legacy_layout_shape() {
        let path = temp_image_path("direct-only");
        let summary =
            create_image(&path, &MkfsOptions::default()).expect("default image should build");
        let report = verify_image(&path).expect("default image verifies");
        assert!(report.indirect_zero);
        assert!(report.reserved_zero);
        assert_eq!(
            report.entries,
            vec![
                "hello.txt:Regular".to_string(),
                "model.neom:Model".to_string(),
                "subdir:Directory".to_string()
            ]
        );

        let mut file = File::open(&path).unwrap();
        let root_bytes = read_inode_bytes(&mut file, summary.root_inode_block).unwrap();
        assert_eq!(get_u64(&root_bytes, 96), 0);
        assert!(all_zero(&root_bytes[136..152]));
        assert!(all_zero(&root_bytes[160..224]));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn direct_only_images_match_legacy_writer_byte_for_byte() {
        assert_legacy_direct_image_matches("legacy-normal", MkfsOptions::default());

        assert_legacy_direct_image_matches(
            "legacy-arthur",
            MkfsOptions {
                include_arthur_memory: true,
                ..MkfsOptions::default()
            },
        );

        assert_legacy_direct_image_matches(
            "legacy-extra-models",
            MkfsOptions {
                model_bytes: Some(b"small primary model\n".to_vec()),
                extra_model_files: vec![
                    ModelFile {
                        name: "model-qwen2.5-0.5b.neom".to_string(),
                        bytes: patterned_bytes(FS_BLOCK_SIZE as usize + 17),
                    },
                    ModelFile {
                        name: "model-qwen2.5-1.5b.neom".to_string(),
                        bytes: patterned_bytes(DIRECT_BLOCKS * FS_BLOCK_SIZE as usize),
                    },
                ],
                ..MkfsOptions::default()
            },
        );
    }

    /// OT-66 L1: the VM image's data disk adds an empty `.system/` and
    /// otherwise names the same entries as `arthur-memory`.
    #[test]
    fn arthur_memory_system_variant_adds_an_empty_system_dir() {
        let plain = temp_image_path("arthur-memory-plain");
        let with = temp_image_path("arthur-memory-system");
        create_image_variant(&plain, &MkfsOptions::default(), ImageVariant::ArthurMemory)
            .expect("arthur-memory builds");
        create_image_variant(
            &with,
            &MkfsOptions::default(),
            ImageVariant::ArthurMemorySystem,
        )
        .expect("arthur-memory-system builds");
        let plain_report = verify_image(&plain).expect("arthur-memory verifies");
        let report = verify_image(&with).expect("arthur-memory-system verifies");
        let mut want = plain_report.entries.clone();
        want.push(".system:Directory".to_string());
        assert_eq!(report.entries, want);
        let (mut file, system) = open_root_child(&with, ".system");
        assert_eq!(system.kind, InodeKind::Directory);
        assert_eq!(system.file_size, 0);
        assert_eq!(system.block_count, 0);
        assert!(
            read_directory_entries(&mut file, &system)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            ImageVariant::from_name("arthur-memory-system"),
            Ok(ImageVariant::ArthurMemorySystem)
        );
        let _ = std::fs::remove_file(plain);
        let _ = std::fs::remove_file(with);
    }

    #[test]
    fn arthur_memory_forgetting_variant_preseeds_memory_files() {
        let path = temp_image_path("arthur-forgetting");
        create_image_variant(
            &path,
            &MkfsOptions::default(),
            ImageVariant::ArthurMemoryForgetting,
        )
        .expect("Arthur M4 image should build");
        let report = verify_image(&path).expect("Arthur M4 image verifies");
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry == ".arthur:Directory")
        );

        let (mut file, arthur) = open_root_child(&path, ".arthur");
        let memory = open_child(&mut file, &arthur, "memory", InodeKind::Directory);
        let memory_entries = read_directory_entries(&mut file, &memory).unwrap();
        let names = memory_entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "fact.mem",
                "preference.mem",
                "event.mem",
                "relationship.mem",
                "fact.key",
                "preference.key",
                "event.key",
                "relationship.key",
            ]
        );

        let fact = open_child(&mut file, &memory, "fact.mem", InodeKind::Regular);
        assert_eq!(
            read_logical_content(&mut file, &fact).unwrap(),
            ARTHUR_M4_FACT_MEM
        );
        let relationship = open_child(&mut file, &memory, "relationship.mem", InodeKind::Regular);
        assert_eq!(
            read_logical_content(&mut file, &relationship).unwrap(),
            ARTHUR_M4_RELATIONSHIP_MEM
        );
        let fact_key = open_child(&mut file, &memory, "fact.key", InodeKind::Regular);
        let fact_key = read_logical_content(&mut file, &fact_key).unwrap();
        assert_eq!(fact_key.len(), ARTHUR_M4_KEY_FILE_LEN);
        assert_eq!(fact_key[0], 0x11);
        assert_eq!(fact_key[ARTHUR_M4_KEY_FILE_LEN - 1], 0x40);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn arthur_reversibility_variant_preseeds_undo_actions_dir() {
        let path = temp_image_path("arthur-reversibility");
        create_image_variant(
            &path,
            &MkfsOptions::default(),
            ImageVariant::ArthurReversibility,
        )
        .expect("Arthur reversibility image should build");
        let report = verify_image(&path).expect("Arthur reversibility image verifies");
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry == ".arthur:Directory")
        );

        let (mut file, arthur) = open_root_child(&path, ".arthur");
        let undo = open_child(&mut file, &arthur, "undo", InodeKind::Directory);
        let _actions = open_child(&mut file, &undo, "actions", InodeKind::Directory);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn arthur_reversibility_stakes_variant_preseeds_policy_file() {
        let path = temp_image_path("arthur-reversibility-stakes");
        create_image_variant(
            &path,
            &MkfsOptions::default(),
            ImageVariant::ArthurReversibilityStakes,
        )
        .expect("Arthur reversibility stakes image should build");
        let report = verify_image(&path).expect("Arthur reversibility stakes image verifies");
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry == ".arthur:Directory")
        );

        let (mut file, arthur) = open_root_child(&path, ".arthur");
        let undo = open_child(&mut file, &arthur, "undo", InodeKind::Directory);
        let _actions = open_child(&mut file, &undo, "actions", InodeKind::Directory);
        let stakes = open_child(&mut file, &arthur, "stakes.json", InodeKind::Regular);
        assert_eq!(
            read_logical_content(&mut file, &stakes).unwrap(),
            ARTHUR_STAKES_POLICY
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn large_extra_file_round_trips_through_double_indirect() {
        let path = temp_image_path("large-roundtrip");
        let content = patterned_bytes(LARGE_TEST_BYTES);
        let options = MkfsOptions {
            extra_regular_files: vec![ExtraFile {
                name: "large-double.bin".to_string(),
                bytes: content.clone(),
            }],
            ..MkfsOptions::default()
        };
        create_image(&path, &options).expect("large image should build");
        let report = verify_image(&path).expect("large image verifies");
        assert!(!report.indirect_zero);
        assert!(report.reserved_zero);
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry == "large-double.bin:Regular")
        );

        let (mut file, inode) = open_root_child(&path, "large-double.bin");
        assert_ne!(inode.single_indirect_block, 0);
        assert_ne!(inode.double_indirect_block, 0);
        assert_eq!(inode.triple_indirect_block, 0);
        assert_eq!(read_logical_content(&mut file, &inode).unwrap(), content);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn corrupt_indirect_pointer_rejects_before_oob_read() {
        let path = temp_image_path("corrupt-pointer");
        let content = patterned_bytes(LARGE_TEST_BYTES);
        let options = MkfsOptions {
            extra_regular_files: vec![ExtraFile {
                name: "large-double.bin".to_string(),
                bytes: content,
            }],
            ..MkfsOptions::default()
        };
        create_image(&path, &options).expect("large image should build");
        let (_file, inode) = open_root_child(&path, "large-double.bin");

        let mut file = File::options().read(true).write(true).open(&path).unwrap();
        let bad_pointer = inode.fs_block_count + 10;
        write_all_at(
            &mut file,
            block_offset(inode.single_indirect_block),
            &bad_pointer.to_le_bytes(),
        )
        .unwrap();
        file.sync_all().unwrap();

        assert!(verify_image(&path).is_err());
        let _ = std::fs::remove_file(path);
    }

    fn assert_legacy_direct_image_matches(label: &str, options: MkfsOptions) {
        let path = temp_image_path(label);
        create_image(&path, &options).expect("current direct-only image should build");
        let actual = std::fs::read(&path).expect("read current image");
        let expected = legacy_direct_only_image_bytes(&options);
        assert_eq!(actual.len(), expected.len());
        let actual_hash = blake3_hash(&actual);
        let expected_hash = blake3_hash(&expected);
        if actual_hash != expected_hash {
            let first_diff = actual
                .iter()
                .zip(expected.iter())
                .position(|(actual, expected)| actual != expected);
            panic!(
                "direct-only image diverged from legacy writer: label={label} first_diff={first_diff:?} actual_hash={} expected_hash={}",
                hex_hash(&actual_hash),
                hex_hash(&expected_hash)
            );
        }
        let _ = std::fs::remove_file(path);
    }

    struct LegacyRootFile {
        name: String,
        kind: InodeKind,
        generation: u32,
        inode_block: u64,
        data_block: u64,
        block_count: u64,
        content: Vec<u8>,
    }

    fn legacy_direct_only_image_bytes(options: &MkfsOptions) -> Vec<u8> {
        assert!(options.extra_regular_files.is_empty());
        let total_blocks = options.image_size_bytes / FS_BLOCK_SIZE as u64;
        let copy_blocks = superblock_copy_blocks(options.image_size_bytes).unwrap();
        let first_data_block = copy_blocks.iter().max().copied().unwrap() + 1;
        let free_map_block_count = free_map_block_count_for_total(total_blocks).unwrap();
        let free_map_block = first_data_block;
        let root_inode_block = free_map_block + free_map_block_count;
        let root_dir_block = root_inode_block + 1;
        let hello_inode_block = root_dir_block + 1;
        let hello_data_block = hello_inode_block + 1;

        let mut root_specs = Vec::with_capacity(1 + options.extra_model_files.len());
        root_specs.push(ModelFile {
            name: "model.neom".to_string(),
            bytes: options.model_bytes.clone().unwrap_or_else(model_content),
        });
        root_specs.extend(options.extra_model_files.clone());

        let mut next_block = hello_data_block + 1;
        let mut root_files = Vec::with_capacity(root_specs.len());
        for (idx, spec) in root_specs.into_iter().enumerate() {
            let block_count = data_block_count(spec.bytes.len()).unwrap();
            assert!(block_count <= DIRECT_BLOCKS as u64);
            let inode_block = next_block;
            let data_block = next_block + 1;
            next_block += 1 + block_count;
            root_files.push(LegacyRootFile {
                name: spec.name,
                kind: InodeKind::Model,
                generation: 3 + idx as u32,
                inode_block,
                data_block,
                block_count,
                content: spec.bytes,
            });
        }

        let subdir_inode_block = next_block;
        let arthur_inode_block = subdir_inode_block + 1;
        let arthur_dir_block = arthur_inode_block + 1;
        let arthur_memory_inode_block = arthur_dir_block + 1;
        let next_free_block = if options.include_arthur_memory {
            arthur_memory_inode_block + 1
        } else {
            subdir_inode_block + 1
        };

        let root_entry_count =
            1 + root_files.len() + 1 + usize::from(options.include_arthur_memory);
        let mut root_dir_data = [0u8; FS_BLOCK_SIZE as usize];
        let mut entry_index = 0usize;
        let mut append_entry = |data: &mut [u8; FS_BLOCK_SIZE as usize],
                                name: &str,
                                child_block: u64,
                                kind: InodeKind| {
            let start = entry_index * DIRENT_SIZE;
            write_dir_entry(
                &mut data[start..start + DIRENT_SIZE],
                name,
                child_block,
                kind,
            )
            .unwrap();
            entry_index += 1;
        };
        append_entry(
            &mut root_dir_data,
            "hello.txt",
            hello_inode_block,
            InodeKind::Regular,
        );
        for file in &root_files {
            append_entry(&mut root_dir_data, &file.name, file.inode_block, file.kind);
        }
        append_entry(
            &mut root_dir_data,
            "subdir",
            subdir_inode_block,
            InodeKind::Directory,
        );
        if options.include_arthur_memory {
            append_entry(
                &mut root_dir_data,
                ".arthur",
                arthur_inode_block,
                InodeKind::Directory,
            );
        }

        let mut arthur_dir_data = [0u8; FS_BLOCK_SIZE as usize];
        write_dir_entry(
            &mut arthur_dir_data[0..DIRENT_SIZE],
            "memory",
            arthur_memory_inode_block,
            InodeKind::Directory,
        )
        .unwrap();

        let subdir_generation = 3 + root_files.len() as u32;
        let arthur_generation = subdir_generation + 1;
        let arthur_memory_generation = arthur_generation + 1;
        let next_generation = if options.include_arthur_memory {
            arthur_memory_generation + 1
        } else {
            subdir_generation + 1
        };

        let root_inode = legacy_inode_bytes(LegacyInodeWrite {
            kind: InodeKind::Directory,
            generation: 1,
            file_size: (DIRENT_SIZE * root_entry_count) as u64,
            block_count: 1,
            direct_blocks: [root_dir_block, 0, 0, 0, 0, 0, 0, 0],
            content_hash: blake3_hash(&root_dir_data[..DIRENT_SIZE * root_entry_count]),
            directory_entry_count: root_entry_count as u32,
        });
        let root_inode_hash = blake3_hash(&root_inode);
        let free_map_bytes = vec![0u8; free_map_byte_len(free_map_block_count).unwrap()];
        let superblock = write_superblock(Superblock {
            total_blocks,
            next_free_block,
            generation: 1,
            root_inode_block,
            root_inode_hash,
            inode_size_bytes: INODE_SIZE as u32,
            next_generation,
            free_map: Some(FreeMapRef {
                start_block: free_map_block,
                block_count: free_map_block_count,
                hash: blake3_hash(&free_map_bytes),
            }),
        });

        let mut image = vec![0u8; options.image_size_bytes as usize];
        for offset in SUPERBLOCK_COPY_OFFSETS {
            legacy_write_at(&mut image, offset, &superblock);
        }
        for idx in 0..free_map_block_count {
            let start = idx as usize * FS_BLOCK_SIZE as usize;
            legacy_write_at(
                &mut image,
                block_offset(free_map_block + idx),
                &free_map_bytes[start..start + FS_BLOCK_SIZE as usize],
            );
        }

        legacy_write_at(
            &mut image,
            block_offset(root_inode_block),
            &block_with_prefix(&root_inode),
        );
        legacy_write_at(&mut image, block_offset(root_dir_block), &root_dir_data);

        let hello_inode = legacy_inode_bytes(LegacyInodeWrite {
            kind: InodeKind::Regular,
            generation: 2,
            file_size: HELLO_CONTENT.len() as u64,
            block_count: 1,
            direct_blocks: [hello_data_block, 0, 0, 0, 0, 0, 0, 0],
            content_hash: blake3_hash(HELLO_CONTENT),
            directory_entry_count: 0,
        });
        legacy_write_at(
            &mut image,
            block_offset(hello_inode_block),
            &block_with_prefix(&hello_inode),
        );
        legacy_write_at(
            &mut image,
            block_offset(hello_data_block),
            &data_block(HELLO_CONTENT).unwrap(),
        );

        for file in &root_files {
            let mut direct_blocks = [0u64; DIRECT_BLOCKS];
            for (idx, slot) in direct_blocks
                .iter_mut()
                .enumerate()
                .take(file.block_count as usize)
            {
                *slot = file.data_block + idx as u64;
            }
            let inode = legacy_inode_bytes(LegacyInodeWrite {
                kind: file.kind,
                generation: file.generation,
                file_size: file.content.len() as u64,
                block_count: file.block_count,
                direct_blocks,
                content_hash: blake3_hash(&file.content),
                directory_entry_count: 0,
            });
            legacy_write_at(
                &mut image,
                block_offset(file.inode_block),
                &block_with_prefix(&inode),
            );
            for (idx, block) in data_blocks(&file.content).unwrap().iter().enumerate() {
                legacy_write_at(
                    &mut image,
                    block_offset(file.data_block + idx as u64),
                    block,
                );
            }
        }

        let subdir_inode = legacy_inode_bytes(LegacyInodeWrite {
            kind: InodeKind::Directory,
            generation: subdir_generation,
            file_size: 0,
            block_count: 0,
            direct_blocks: [0; DIRECT_BLOCKS],
            content_hash: blake3_hash(&[]),
            directory_entry_count: 0,
        });
        legacy_write_at(
            &mut image,
            block_offset(subdir_inode_block),
            &block_with_prefix(&subdir_inode),
        );

        if options.include_arthur_memory {
            let arthur_inode = legacy_inode_bytes(LegacyInodeWrite {
                kind: InodeKind::Directory,
                generation: arthur_generation,
                file_size: DIRENT_SIZE as u64,
                block_count: 1,
                direct_blocks: [arthur_dir_block, 0, 0, 0, 0, 0, 0, 0],
                content_hash: blake3_hash(&arthur_dir_data[..DIRENT_SIZE]),
                directory_entry_count: 1,
            });
            legacy_write_at(
                &mut image,
                block_offset(arthur_inode_block),
                &block_with_prefix(&arthur_inode),
            );
            legacy_write_at(&mut image, block_offset(arthur_dir_block), &arthur_dir_data);
            let memory_inode = legacy_inode_bytes(LegacyInodeWrite {
                kind: InodeKind::Directory,
                generation: arthur_memory_generation,
                file_size: 0,
                block_count: 0,
                direct_blocks: [0; DIRECT_BLOCKS],
                content_hash: blake3_hash(&[]),
                directory_entry_count: 0,
            });
            legacy_write_at(
                &mut image,
                block_offset(arthur_memory_inode_block),
                &block_with_prefix(&memory_inode),
            );
        }

        image
    }

    struct LegacyInodeWrite {
        kind: InodeKind,
        generation: u32,
        file_size: u64,
        block_count: u64,
        direct_blocks: [u64; DIRECT_BLOCKS],
        content_hash: [u8; 32],
        directory_entry_count: u32,
    }

    fn legacy_inode_bytes(input: LegacyInodeWrite) -> [u8; INODE_SIZE] {
        let mut out = [0u8; INODE_SIZE];
        out[0..8].copy_from_slice(INODE_MAGIC);
        out[8] = input.kind as u8;
        out[9] = TYPE_VERSION;
        put_u32(&mut out, 12, input.generation);
        put_u64(&mut out, 16, input.file_size);
        put_u64(&mut out, 24, input.block_count);
        for (idx, block) in input.direct_blocks.iter().copied().enumerate() {
            put_u64(&mut out, 32 + idx * 8, block);
        }
        put_u64(&mut out, 96, 0);
        out[104..136].copy_from_slice(&input.content_hash);
        put_u32(&mut out, 152, input.kind.rights());
        put_u32(&mut out, 156, input.directory_entry_count);
        let checksum = blake3_hash(&out[..224]);
        out[224..256].copy_from_slice(&checksum);
        out
    }

    fn legacy_write_at(image: &mut [u8], offset: u64, bytes: &[u8]) {
        let offset = offset as usize;
        image[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    fn open_root_child(path: &Path, name: &str) -> (File, Inode) {
        let mut file = File::open(path).unwrap();
        let mut sb_bytes = [0u8; SUPERBLOCK_SIZE];
        read_exact_at(&mut file, SUPERBLOCK_COPY_OFFSETS[0], &mut sb_bytes).unwrap();
        let sb = read_superblock(&sb_bytes).unwrap();
        let root_bytes = read_inode_bytes(&mut file, sb.root_inode_block).unwrap();
        let root = read_inode(&root_bytes, sb.total_blocks).unwrap();
        let entries = read_directory_entries(&mut file, &root).unwrap();
        let entry = entries
            .iter()
            .find(|entry| entry.name == name)
            .expect("root child exists");
        let child_bytes = read_inode_bytes(&mut file, entry.child_block).unwrap();
        let child = read_inode(&child_bytes, sb.total_blocks).unwrap();
        (file, child)
    }

    fn open_child(file: &mut File, parent: &Inode, name: &str, expected_kind: InodeKind) -> Inode {
        let entries = read_directory_entries(file, parent).unwrap();
        let entry = entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("missing child `{name}`"));
        let child_bytes = read_inode_bytes(file, entry.child_block).unwrap();
        let child = read_inode(&child_bytes, parent.fs_block_count).unwrap();
        assert_eq!(entry.kind, expected_kind);
        assert_eq!(child.kind, expected_kind);
        child
    }

    fn patterned_bytes(len: usize) -> Vec<u8> {
        (0..len)
            .map(|offset| {
                let block = offset / FS_BLOCK_SIZE as usize;
                let byte = offset % FS_BLOCK_SIZE as usize;
                ((block as u32 * 31 + byte as u32 * 17 + 0xa5) & 0xff) as u8
            })
            .collect()
    }

    fn temp_image_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("neofs-mkfs-{label}-{}.img", std::process::id()))
    }
}
