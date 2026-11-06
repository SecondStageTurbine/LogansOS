// SPDX-License-Identifier: MPL-2.0
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! LogansOS (`target_os = "neoplutarch"`, OT-61 L1): fonts come from
//! `neo-font-store` (compiled into the content process), are read with
//! `skrifa`/`read-fonts` (pure Rust, no FreeType), and are shaped by the
//! crate's existing HarfBuzz shaper.

pub mod font;
pub mod font_list;
