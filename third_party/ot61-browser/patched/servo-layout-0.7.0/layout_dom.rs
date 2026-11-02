// SPDX-License-Identifier: MPL-2.0
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The one place layout names its concrete DOM types (LogansOS OT-61, option A:
//! `Plans/DECISION-2026-11-02-ot61-first-page-option-a.md`).
//!
//! Upstream, layout imports these straight from `script::layout_dom`. Every
//! such import now goes through here instead, so a target without `script`
//! can supply its own types implementing `layout_api`'s DOM traits. Only types
//! move: on every other target these are script's types, unchanged.

#[cfg(not(target_os = "neoplutarch"))]
pub(crate) use script::layout_dom::{
    ServoDangerousStyleDocument, ServoDangerousStyleElement, ServoLayoutElement, ServoLayoutNode,
};

#[cfg(target_os = "neoplutarch")]
pub(crate) use neo_layout_dom::{
    ServoDangerousStyleDocument, ServoDangerousStyleElement, ServoLayoutElement, ServoLayoutNode,
};
