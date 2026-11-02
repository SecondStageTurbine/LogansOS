// SPDX-License-Identifier: MPL-2.0
//! OT-61 L0: names patched `servo-layout`'s entry point so the whole closure
//! (layout over `neo-layout-dom`, no `servo-script`) is built for the target.
pub use layout::LayoutFactoryImpl;
pub use neo_layout_dom::Document;
