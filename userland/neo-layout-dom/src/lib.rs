// SPDX-License-Identifier: MPL-2.0
//! OT-61 L0 — `neo-layout-dom`: the DOM that Servo's layout lays out on
//! LogansOS before SpiderMonkey exists (option A,
//! `Plans/DECISION-2026-11-02-ot61-first-page-option-a.md`).
//!
//! Upstream `servo-layout` 0.7.0 names `servo-script`'s DOM wrapper types
//! (`script::layout_dom::{ServoLayoutNode, ServoLayoutElement,
//! ServoDangerousStyleElement, ServoDangerousStyleDocument}`). Our patched copy
//! names them through one alias module; on `x86_64-unknown-neoplutarch` the
//! alias points here. So this crate exports types with exactly those names,
//! implementing `servo-layout-api`'s `LayoutNode`, `LayoutElement`,
//! `DangerousStyleNode` and `DangerousStyleElement`, plus the four inherent
//! methods layout calls (`ServoLayoutNode::new` and the document's
//! `root_element`, `shared_style_locks`,
//! `flush_shadow_root_stylesheets_if_necessary`).
//!
//! The document is static: html5ever parses it once (no script runs, because
//! there is no script engine) into an arena whose shape never changes. Stylo
//! and layout write only each node's style and layout data.

mod dom;
mod layout_impl;
mod style_impl;

pub use dom::{Document, Node, SharedRwLocks};
pub use layout_impl::{NeoLayoutDomTypeBundle, ServoLayoutElement, ServoLayoutNode};
pub use style_impl::{
    ServoDangerousStyleDocument, ServoDangerousStyleElement, ServoDangerousStyleNode,
    ServoDangerousStyleShadowRoot,
};
