// SPDX-License-Identifier: MPL-2.0
//! Bundled faces, each an unmodified upstream release under the SIL OFL-1.1
//! (assets/fonts/OFL-Inter.txt, assets/fonts/OFL-IBM-Plex.txt; provenance in
//! assets/fonts/README.md). Feature-gated so a binary links only the faces it
//! uses.

/// Inter Regular 4.001 (release v4.1, static TTF) - Arthur's proportional
/// voice (design language §6.1).
#[cfg(feature = "inter")]
pub static INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");

/// IBM Plex Mono Regular 2.005 - the `neo>` rail and structured data.
#[cfg(feature = "plex-mono")]
pub static PLEX_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/IBMPlexMono-Regular.ttf");
