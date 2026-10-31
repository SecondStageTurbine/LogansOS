// SPDX-License-Identifier: MPL-2.0
//! B1b: what Arthur says about its model when it comes from a boot module
//! (`docs/THINK-2026-10-19-arthur-first-boot-model.md`, Q4).
//!
//! Pure: host-tested by `tools/arthur-logic-tests` through `#[path]`.
//!
//! A missing or bad model never takes Arthur down. It answers every line with
//! one plain sentence that names the state and the next step; the detail is a
//! serial tag for power users and the bare-metal log. The "not enough memory"
//! state belongs to the tier rule (B2) and is not produced here.

/// The kernel's `-ENOENT`: no such module (absent, or already consumed).
pub const ENOENT: i64 = -2;
/// The kernel's `-EBADMSG`: the module failed its pinned-digest check.
pub const EBADMSG: i64 = -74;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelState {
    Ready,
    Absent,
    Damaged,
    LoadFailed,
}

/// The state a failed `sys_model_load` of the boot-module path means.
pub const fn state_for_load_error(errno: i64) -> ModelState {
    match errno {
        ENOENT => ModelState::Absent,
        EBADMSG => ModelState::Damaged,
        _ => ModelState::LoadFailed,
    }
}

/// The serial tag (`ARTHUR model state=<tag>`).
pub const fn state_tag(state: ModelState) -> &'static [u8] {
    match state {
        ModelState::Ready => b"ready",
        ModelState::Absent => b"absent",
        ModelState::Damaged => b"damaged",
        ModelState::LoadFailed => b"load_failed",
    }
}

/// The one sentence a beginner sees. `None` when the model is ready.
pub const fn beginner_line(state: ModelState) -> Option<&'static [u8]> {
    match state {
        ModelState::Ready => None,
        ModelState::Absent => Some(
            b"I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with.",
        ),
        ModelState::Damaged => Some(
            b"My model file is damaged, so I won't use it. Re-create the USB from the original image.",
        ),
        ModelState::LoadFailed => {
            Some(b"My model didn't load, so I'm running without it. The details are in /model.")
        }
    }
}

/// B1c (F8): how the vocab load went. The vocab is part of the model: the
/// model is only taken when the vocab `Loaded`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VocabLoad {
    Loaded,
    /// No vocab module (`ENOENT`).
    Absent,
    /// The vocab failed its pin (`EBADMSG`).
    Damaged,
    /// The open or a read failed with another errno.
    Unreadable(i64),
    /// It was read but did not parse.
    Unparseable,
}

/// The vocab outcome a failed open or read means.
pub const fn vocab_load_for_error(errno: i64) -> VocabLoad {
    match errno {
        ENOENT => VocabLoad::Absent,
        EBADMSG => VocabLoad::Damaged,
        other => VocabLoad::Unreadable(other),
    }
}

/// B1c (F8): compose the vocab outcome with the model load. `load_model` runs
/// only when the vocab `Loaded`, so a bad vocab never consumes the model's
/// boot module (by construction, not by convention).
///
/// | Vocab | Model | State |
/// |---|---|---|
/// | `Absent` | not taken | `Absent` |
/// | `Damaged` | not taken | `Damaged` |
/// | `Unreadable` / `Unparseable` | not taken | `LoadFailed` |
/// | `Loaded` | `ENOENT` / `EBADMSG` / other / ok | `Absent` / `Damaged` / `LoadFailed` / `Ready` |
pub fn compose(vocab: VocabLoad, load_model: impl FnOnce() -> Result<(), i64>) -> ModelState {
    match vocab {
        VocabLoad::Absent => ModelState::Absent,
        VocabLoad::Damaged => ModelState::Damaged,
        VocabLoad::Unreadable(_) | VocabLoad::Unparseable => ModelState::LoadFailed,
        VocabLoad::Loaded => match load_model() {
            Ok(()) => ModelState::Ready,
            Err(errno) => state_for_load_error(errno),
        },
    }
}

/// OT-87 R2: where Arthur looked for its model. The boot modules come first;
/// the installed NeoFS volume is tried only when they have no model (or no
/// vocab) at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelSource {
    BootModule,
    Neofs,
}

/// The source Arthur reports for its model state: the vocab's when the vocab
/// didn't load (the state is about the vocab, and `compose` didn't take the
/// model), else the model's. With a boot-module model and a NeoFS vocab it is
/// the boot module (Chad's R2 review, item 4: the vocab's fallback used to
/// relabel the model).
pub const fn reported_source(
    vocab: VocabLoad,
    vocab_source: ModelSource,
    model_source: ModelSource,
) -> ModelSource {
    match vocab {
        VocabLoad::Loaded => model_source,
        _ => vocab_source,
    }
}

/// The serial tag (`ARTHUR model source=<tag> state=…`).
pub const fn source_tag(source: ModelSource) -> &'static [u8] {
    match source {
        ModelSource::BootModule => b"boot-module",
        ModelSource::Neofs => b"neofs",
    }
}

/// The one sentence for `state` when the model was looked for at `source`.
/// The boot-module sentences are [`beginner_line`]'s. From the installed
/// disk, absent and damaged name the disk: there is no USB to point at.
pub const fn line_for(state: ModelState, source: ModelSource) -> Option<&'static [u8]> {
    match (source, state) {
        (ModelSource::Neofs, ModelState::Absent) => Some(
            b"I can hear you, but my model is missing from this disk, so I'm running without it.",
        ),
        (ModelSource::Neofs, ModelState::Damaged) => Some(
            b"My model file on this disk is damaged, so I won't use it. Reinstall LogansOS to repair it.",
        ),
        _ => beginner_line(state),
    }
}
