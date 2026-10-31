// SPDX-License-Identifier: MPL-2.0
//! Capability-backed validated .neom model object.

extern crate alloc;

#[cfg(feature = "phase9-real-quant")]
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "gguf")]
use super::gguf_dequant_logic::{self, GgufDequantError};
#[cfg(feature = "gguf")]
use super::gguf_logic::{self, GgufTensorInfo};
use super::model_logic::{self, IoMeta, NodeMeta, TensorMeta};
#[cfg(feature = "neo-test-ot16-gdn-walker-execution")]
pub(crate) use super::model_logic::{LayerEntry, LayerKind, LayerSchedule, walk_qwen2_schedule};
pub use super::model_logic::{
    ModelConfig, ModelFormatError, NEOM_HEADER_SIZE_V4, NEOM_MAX_FILE_SIZE, NeomDtype, ShapeMeta,
    neom_total_size_hint, validate_neom,
};
use super::tensor::{self, Tensor, TensorCreateDesc};
use crate::fs::FileProvenance;
use crate::sched::process::ProcessId;
#[cfg(feature = "neo-test-arthur-real-brain-boot-qwen35")]
use crate::serial_println;

pub const MODEL_QUERY_VERSION: u32 = 1;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModelQueryDesc {
    pub version: u32,
    pub tensor_count: u32,
    pub node_count: u32,
    pub input_count: u32,
    pub output_count: u32,
    pub value_count: u32,
    pub first_tensor_dtype: u32,
    pub first_tensor_rank: u32,
    pub first_tensor_dims: [u64; tensor::MAX_RANK],
    pub first_tensor_byte_len: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ModelError {
    Format(ModelFormatError),
    #[cfg(feature = "gguf")]
    #[allow(dead_code)] // default-off importer error, exercised through host logic KATs first
    Gguf(gguf_logic::GgufError),
    Tensor(tensor::TensorError),
    Stale,
}

pub struct Model {
    /// BUILD-O1b (ADR-043): the arena owns the weights at rest; the model
    /// NAMES them. Each key was `alloc`'d (miss) or `retain`'d (dedup hit,
    /// BUILD-O2/ADR-044 — N models, ONE slot, one resting `Arc`, idle ⇒
    /// claimable); `Drop` releases — free-on-last-handle either way.
    weights: Vec<crate::ai::arena::SlotKey>,
    /// Cached physical ids (index-parallel with `weights`) so the §9.7
    /// reader scans answer without minting.
    weight_ids: Vec<u64>,
    weight_metas: Vec<TensorMeta>,
    #[cfg(feature = "phase9-real-quant")]
    weight_names: Vec<String>,
    nodes: Vec<NodeMeta>,
    ios: Vec<IoMeta>,
    node_count: u32,
    input_count: u32,
    output_count: u32,
    value_count: u32,
    #[allow(dead_code)]
    // F2 model-object config surface; consumed by the feature-gated forward path.
    model_config: Option<ModelConfig>,
    /// OT-16: the resolved layer-kind schedule the walker executes — parsed from
    /// a v3 `.neom`, or a synthesized homogeneous Qwen2 schedule for v1/v2. Only
    /// present under the walker feature; with it off the `Model` is unchanged
    /// (kill-switch 1).
    #[cfg(feature = "ot16-graph-walker")]
    schedule: Option<model_logic::LayerSchedule>,
    provenance: FileProvenance,
    #[cfg(feature = "gguf")]
    #[allow(dead_code)] // retained for GGUF tensor enumeration before syscall exposure
    gguf_tensors: Vec<GgufTensorInfo>,
}

impl Model {
    /// Parse + validate a `.neom` model. When `dedup_owner` is `Some(principal)`,
    /// each immutable weight is **content-hash deduplicated** (Phase 7 BUILD-3, ISC-3):
    /// a weight whose bytes match an already-resident immutable tensor shares that one
    /// physical copy (refcount++) instead of allocating a second; a fresh weight is
    /// registered as a dedup candidate owned by `principal`. `None` loads without
    /// dedup (used by single-load host/test paths whose behaviour must not change).
    pub fn from_neom_bytes(
        bytes: &[u8],
        provenance: FileProvenance,
        dedup_owner: Option<ProcessId>,
    ) -> Result<Self, ModelError> {
        let validated = validate_neom(bytes).map_err(ModelError::Format)?;
        let model_config = optional_model_config(bytes, &validated)?;
        // OT-16: resolve the walker's layer-kind schedule at load. validate_neom
        // already rejected a malformed v3 schedule; this re-resolves it (or
        // synthesizes the homogeneous Qwen2 schedule for v1/v2).
        #[cfg(feature = "ot16-graph-walker")]
        let schedule = match &model_config {
            Some(cfg) => Some(
                model_logic::resolve_schedule(bytes, &validated.header, cfg.layer_count)
                    .map_err(ModelError::Format)?,
            ),
            None => None,
        };
        #[cfg(feature = "phase9-real-quant")]
        let mut weight_names = Vec::with_capacity(validated.tensors.len());
        #[cfg(feature = "phase9-real-quant")]
        for meta in &validated.tensors {
            weight_names.push(
                model_logic::tensor_name(bytes, &validated.header, meta)
                    .map_err(ModelError::Format)?
                    .into(),
            );
        }
        let mut weights = Vec::with_capacity(validated.tensors.len());
        let mut weight_ids = Vec::with_capacity(validated.tensors.len());
        // A failure on weight N must release the slots already allocated for
        // weights 0..N — `SlotKey` has no RAII and no `Model` exists yet to
        // drop, so the pre-O1b Arc-Vec auto-free has to be explicit here
        // (Forge C-1: the silent error-path slot leak).
        let release_partial = |allocated: &Vec<crate::ai::arena::SlotKey>| {
            for key in allocated {
                crate::ai::arena::release(*key);
            }
        };
        for (tensor_index, meta) in validated.tensors.iter().enumerate() {
            let weight = match model_logic::weight_slice(bytes, &validated.header, meta) {
                Ok(weight) => weight,
                Err(e) => {
                    release_partial(&weights);
                    return Err(ModelError::Format(e));
                }
            };
            let desc = tensor_desc_from_meta(meta);
            #[cfg(feature = "freeze-watchdog")]
            crate::watchdog::set_weight(tensor_index as u64, validated.tensors.len() as u64);
            log_qwen35_model_weight("begin", tensor_index, validated.tensors.len(), weight.len());
            let (key, weight_id) = match dedup_owner {
                Some(owner) => match share_or_allocate_weight(desc, weight, owner) {
                    Ok(found) => found,
                    Err(e) => {
                        release_partial(&weights);
                        return Err(e);
                    }
                },
                None => match Tensor::from_desc_and_bytes(desc, weight) {
                    Ok(tensor) => {
                        let arc = Arc::new(tensor);
                        let id = arc.id();
                        // The arena becomes the owner at rest (BUILD-O1b);
                        // the model keeps the creator count until Drop.
                        (
                            crate::ai::arena::alloc(arc, crate::ai::arena::default_home()),
                            id,
                        )
                    }
                    Err(e) => {
                        release_partial(&weights);
                        return Err(ModelError::Tensor(e));
                    }
                },
            };
            weight_ids.push(weight_id);
            weights.push(key);
            log_qwen35_model_weight(
                "resident",
                tensor_index,
                validated.tensors.len(),
                weight.len(),
            );
        }
        Ok(Self {
            weights,
            weight_ids,
            weight_metas: validated.tensors,
            #[cfg(feature = "phase9-real-quant")]
            weight_names,
            nodes: validated.nodes,
            ios: validated.ios,
            node_count: validated.header.node_count,
            input_count: validated.header.input_count,
            output_count: validated.header.output_count,
            value_count: validated.value_count,
            model_config,
            #[cfg(feature = "ot16-graph-walker")]
            schedule,
            provenance,
            #[cfg(feature = "gguf")]
            gguf_tensors: Vec::new(),
        })
    }

    /// OT-16: the resolved layer-kind schedule the walker executes (walker
    /// feature only). `None` when the model has no derivable config.
    #[cfg(feature = "ot16-graph-walker")]
    pub fn schedule(&self) -> Option<&model_logic::LayerSchedule> {
        self.schedule.as_ref()
    }

    /// Import a GGUF v3 model into the Model object without making GGUF native.
    /// Quantized GGML tensors are preserved byte-for-byte as immutable U8
    /// storage; their GGUF shape/type/block metadata stays in `gguf_tensors`
    /// for enumeration and future 8.3 dequant. `.neom` graph execution remains
    /// the native path.
    #[cfg(feature = "gguf")]
    #[allow(dead_code)] // default-off importer entry point, not wired to syscalls yet
    pub fn from_gguf_bytes(
        bytes: &[u8],
        provenance: FileProvenance,
        dedup_owner: Option<ProcessId>,
    ) -> Result<Self, ModelError> {
        let imported = gguf_logic::parse_gguf(bytes).map_err(ModelError::Gguf)?;
        let mut weights = Vec::with_capacity(imported.tensors.len());
        let mut weight_ids = Vec::with_capacity(imported.tensors.len());
        let mut weight_metas = Vec::with_capacity(imported.tensors.len());
        #[cfg(feature = "phase9-real-quant")]
        let mut weight_names = Vec::with_capacity(imported.tensors.len());
        let release_partial = |allocated: &Vec<crate::ai::arena::SlotKey>| {
            for key in allocated {
                crate::ai::arena::release(*key);
            }
        };

        for tensor in &imported.tensors {
            let weight = match imported.tensor_data(bytes, tensor) {
                Ok(weight) => weight,
                Err(e) => {
                    release_partial(&weights);
                    return Err(ModelError::Gguf(e));
                }
            };
            let desc = byte_tensor_desc(weight.len() as u64)?;
            let (key, weight_id) = match dedup_owner {
                Some(owner) => match share_or_allocate_weight(desc, weight, owner) {
                    Ok(found) => found,
                    Err(e) => {
                        release_partial(&weights);
                        return Err(e);
                    }
                },
                None => match Tensor::from_desc_and_bytes(desc, weight) {
                    Ok(tensor) => {
                        let arc = Arc::new(tensor);
                        let id = arc.id();
                        (
                            crate::ai::arena::alloc(arc, crate::ai::arena::default_home()),
                            id,
                        )
                    }
                    Err(e) => {
                        release_partial(&weights);
                        return Err(ModelError::Tensor(e));
                    }
                },
            };
            weights.push(key);
            weight_ids.push(weight_id);
            weight_metas.push(byte_tensor_meta(weight.len() as u64));
            #[cfg(feature = "phase9-real-quant")]
            weight_names.push(tensor.name.clone());
        }

        Ok(Self {
            weights,
            weight_ids,
            weight_metas,
            #[cfg(feature = "phase9-real-quant")]
            weight_names,
            nodes: Vec::new(),
            ios: Vec::new(),
            node_count: 0,
            input_count: 0,
            output_count: 0,
            value_count: 0,
            model_config: None,
            // A GGUF import carries no v3 schedule (that path is B3); the walker
            // has nothing to run for it and fails closed if invoked.
            #[cfg(feature = "ot16-graph-walker")]
            schedule: None,
            provenance,
            gguf_tensors: imported.tensors,
        })
    }

    pub fn validate_current(&self) -> Result<(), ModelError> {
        self.provenance
            .validate_current()
            .map_err(|_| ModelError::Stale)
    }

    /// Whether this model holds physical tensor `tensor_id` among its weights (Phase 7
    /// BUILD-3). Lets the cross-process reader scan recognise a principal that
    /// references a deduplicated weight through its loaded model, not a direct cap.
    pub fn references_tensor_id(&self, tensor_id: u64) -> bool {
        self.weight_ids.contains(&tensor_id)
    }

    /// Physical id of this model's first weight (Phase 7 BUILD-3) — the gate uses it to
    /// observe the shared/deduped copy's reader set.
    #[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
    pub fn first_weight_id(&self) -> Option<u64> {
        self.weight_ids.first().copied()
    }

    pub fn query_desc(&self) -> ModelQueryDesc {
        let mut first_tensor_dims = [0u64; tensor::MAX_RANK];
        let (first_tensor_dtype, first_tensor_rank, first_tensor_byte_len) =
            if let Some(meta) = self.weight_metas.first() {
                first_tensor_dims = meta.shape.dims;
                (meta.dtype.as_abi(), meta.shape.rank, meta.weight_len)
            } else {
                (0, 0, 0)
            };
        ModelQueryDesc {
            version: MODEL_QUERY_VERSION,
            tensor_count: self.weights.len() as u32,
            node_count: self.node_count,
            input_count: self.input_count,
            output_count: self.output_count,
            value_count: self.value_count,
            first_tensor_dtype,
            first_tensor_rank,
            first_tensor_dims,
            first_tensor_byte_len,
        }
    }

    /// The weight slot keys, in `.neom` order — the executor mints each at
    /// pass start (the pass IS the use: the minted `Arc`s pin the weights,
    /// I-9, for exactly the pass duration) and drops them with its value
    /// table.
    pub fn weight_slots(&self) -> &[crate::ai::arena::SlotKey] {
        &self.weights
    }

    #[cfg(feature = "phase9-real-quant")]
    pub fn mint_weight_named(&self, name: &str) -> Option<(TensorMeta, Arc<Tensor>)> {
        let idx = self.weight_names.iter().position(|found| found == name)?;
        let meta = *self.weight_metas.get(idx)?;
        let tensor = crate::ai::arena::mint(*self.weights.get(idx)?)?;
        Some((meta, tensor))
    }

    pub fn nodes(&self) -> &[NodeMeta] {
        &self.nodes
    }

    pub fn ios(&self) -> &[IoMeta] {
        &self.ios
    }

    pub const fn value_count(&self) -> u32 {
        self.value_count
    }

    pub const fn input_count(&self) -> u32 {
        self.input_count
    }

    pub const fn output_count(&self) -> u32 {
        self.output_count
    }

    #[allow(dead_code)] // F2 accessor for feature-gated Qwen2 config selection.
    pub const fn model_config(&self) -> Option<ModelConfig> {
        self.model_config
    }

    #[cfg(feature = "gguf")]
    #[allow(dead_code)] // default-off importer enumeration accessor
    pub fn gguf_tensors(&self) -> &[GgufTensorInfo] {
        &self.gguf_tensors
    }

    #[cfg(feature = "gguf")]
    #[allow(dead_code)] // default-off BUILD-8.3 integration entry point
    pub fn dequantize_gguf_tensor_to_f32(&self, name: &str) -> Result<Vec<f32>, GgufDequantError> {
        let (idx, tensor) = self
            .gguf_tensors
            .iter()
            .enumerate()
            .find(|(_, tensor)| tensor.name == name)
            .ok_or(GgufDequantError::TensorNotFound)?;
        let weight = self
            .weights
            .get(idx)
            .and_then(|handle| crate::ai::arena::mint(*handle))
            .ok_or(GgufDequantError::InvalidTensor)?;
        gguf_dequant_logic::dequantize_tensor_data_to_f32(tensor, weight.bytes())
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        // Last Model reference: release the creator count on every weight
        // slot — free-on-last-handle (a deduped weight's Arc survives in the
        // other model's slot; the physical copy dies with its last slot).
        for key in &self.weights {
            crate::ai::arena::release(*key);
        }
    }
}

fn optional_model_config(
    bytes: &[u8],
    validated: &model_logic::ValidatedNeom,
) -> Result<Option<ModelConfig>, ModelError> {
    match model_logic::derive_model_config(bytes, validated) {
        Ok(config) => Ok(config),
        Err(err) => {
            #[cfg(feature = "phase9-data-driven-config")]
            {
                Err(ModelError::Format(err))
            }
            #[cfg(not(feature = "phase9-data-driven-config"))]
            {
                let _ = err;
                Ok(None)
            }
        }
    }
}

#[cfg(feature = "neo-test-arthur-real-brain-boot-qwen35")]
fn log_qwen35_model_weight(phase: &str, index: usize, count: usize, bytes: usize) {
    serial_println!(
        "qwen35_model_weight phase={} index={} count={} bytes={}",
        phase,
        index,
        count,
        bytes
    );
}

#[cfg(not(feature = "neo-test-arthur-real-brain-boot-qwen35"))]
fn log_qwen35_model_weight(_phase: &str, _index: usize, _count: usize, _bytes: usize) {}

/// Content-hash dedup of one immutable model weight (BUILD-3 semantics on the
/// BUILD-O2 slot key). Probe by BLAKE3 hash: on a hit whose shape and dtype
/// match, `retain` and share the existing SLOT (one slot, one resting `Arc`,
/// one physical copy — and the copy can move without breaking the share); a
/// hit whose slot dies between the probe's mint and the retain falls through
/// to fresh allocation. On a miss, allocate fresh, `alloc` into the arena,
/// register the count-once accounting, and publish the hash → slot candidacy
/// (which pins nothing) owned by
/// `owner`. (A hash hit with a mismatched shape/dtype — a pathological collision —
/// falls through to a fresh, unregistered allocation rather than aliasing wrong bytes.)
fn share_or_allocate_weight(
    desc: TensorCreateDesc,
    weight: &[u8],
    owner: ProcessId,
) -> Result<(crate::ai::arena::SlotKey, u64), ModelError> {
    let hash = crate::ai::sharing::content_hash(weight);
    if let Some((key, existing)) = crate::ai::sharing::dedup_probe(hash) {
        let dtype = tensor::Dtype::from_abi(desc.dtype).map_err(ModelError::Tensor)?;
        let shape = tensor::Shape::new(desc.rank, desc.dims).map_err(ModelError::Tensor)?;
        if existing.dtype() == dtype && existing.shape() == shape {
            // The hit shares the SLOT: this model's count rides the same
            // arena entry (released by its Drop). A retain refusal means the
            // slot died between the probe's mint and here — fall through to
            // a fresh allocation; the transient `existing` Arc just drops.
            if crate::ai::arena::retain(key) {
                crate::ai::sharing::on_dedup_hit(existing.id(), owner);
                return Ok((key, existing.id()));
            }
        }
    }
    let tensor = Tensor::from_desc_and_bytes(desc, weight).map_err(ModelError::Tensor)?;
    let arc = Arc::new(tensor);
    let id = arc.id();
    // Count-once accounting + registry entry first (needs the &Arc), then
    // the arena takes ownership, then the candidacy publishes the slot.
    crate::ai::sharing::register_dedup_weight(&arc, owner, hash);
    let key = crate::ai::arena::alloc(arc, crate::ai::arena::default_home());
    crate::ai::sharing::publish_dedup_candidate(hash, key);
    Ok((key, id))
}

fn tensor_desc_from_meta(meta: &TensorMeta) -> TensorCreateDesc {
    TensorCreateDesc {
        rank: meta.shape.rank,
        dtype: match meta.dtype {
            NeomDtype::Q5K => tensor::Dtype::U8.as_abi(),
            other => other.as_abi(),
        },
        mutable: 0,
        _pad: 0,
        dims: meta.shape.dims,
        data_ptr: 0,
        data_len: meta.weight_len,
    }
}

#[cfg(feature = "gguf")]
#[allow(dead_code)] // consumed by the default-off GGUF importer constructor
fn byte_tensor_desc(len: u64) -> Result<TensorCreateDesc, ModelError> {
    if len == 0 {
        return Err(ModelError::Tensor(tensor::TensorError::InvalidLength));
    }
    let mut dims = [0u64; tensor::MAX_RANK];
    dims[0] = len;
    Ok(TensorCreateDesc {
        rank: 1,
        dtype: tensor::Dtype::U8.as_abi(),
        mutable: 0,
        _pad: 0,
        dims,
        data_ptr: 0,
        data_len: len,
    })
}

#[cfg(feature = "gguf")]
#[allow(dead_code)] // consumed by the default-off GGUF importer constructor
fn byte_tensor_meta(len: u64) -> TensorMeta {
    let mut dims = [0u64; tensor::MAX_RANK];
    dims[0] = len;
    TensorMeta {
        dtype: NeomDtype::U8,
        shape: ShapeMeta { rank: 1, dims },
        weight_off: 0,
        weight_len: len,
        name_off: 0,
        name_len: 0,
    }
}

pub fn model_errno(err: ModelError) -> i64 {
    match err {
        ModelError::Format(_) => crate::errno::EINVAL,
        #[cfg(feature = "gguf")]
        ModelError::Gguf(_) => crate::errno::EINVAL,
        ModelError::Tensor(tensor::TensorError::OutOfMemory) => crate::errno::ENOMEM,
        ModelError::Tensor(_) => crate::errno::EINVAL,
        ModelError::Stale => crate::errno::ESTALE,
    }
}
