// SPDX-License-Identifier: MPL-2.0
//! L2's oracle: a block-only page whose border boxes are worked out by hand
//! from CSS 2.1 (§8.3.1 margin collapsing, §10.3.3 block widths), not from
//! running layout. No text, so no font metrics enter the numbers.
//!
//! The author sheet sets `display: block` and zero margins itself, so nothing
//! depends on a user-agent stylesheet. Viewport: 800 x 600 CSS px.
//!
//! Working, in CSS px (y grows down; boxes are border boxes):
//!
//! - `html`: the root. Its margins don't collapse with its children's
//!   (§8.3.1), so collapsed child margins sit inside its content box.
//!   x = 0, width = 800.
//! - `body` (margin 8) and `#a` (margin-top 10): body has no border or padding,
//!   so its top margin and #a's collapse into one margin of max(8, 10) = 10.
//!   Both border boxes start at y = 10. body: x = 8, width = 800 - 2*8 = 784.
//! - `#a`: 200 x 50, margin 10 20 30 20. x = 8 + 20 = 28, y = 10.
//! - `#a`/`#b`: #a's bottom margin 30 and #b's top margin 20 collapse to 30.
//!   #b: y = 10 + 50 + 30 = 90, x = 8 (margin-left 0).
//!   Border box = 100 + 2*5 + 2*2 = 114 wide, 40 + 2*5 + 2*2 = 54 tall.
//! - `#b`/`#c`: #b's bottom margin 0 and #c's top margin 15 collapse to 15.
//!   #c: y = 90 + 54 + 15 = 159, x = 8, width auto = 784, height 25.
//! - `#c`'s bottom margin 15 collapses through body (no border, padding or
//!   height) with body's bottom margin 8, to 15, below body. body's border box
//!   ends where #c's does: 159 + 25 = 184, so body is 184 - 10 = 174 tall.
//! - `html`: content = body's collapsed top margin 10 + body 174 + collapsed
//!   bottom margin 15 = 199 tall.

/// The page.
pub const PAGE: &str = "<!DOCTYPE html><html id=html><head><style>\
html, body, div { display: block; margin: 0; padding: 0; border: 0 }\
body { margin: 8px }\
#a { width: 200px; height: 50px; margin: 10px 20px 30px 20px }\
#b { width: 100px; height: 40px; margin-top: 20px; padding: 5px; border: 2px solid black }\
#c { height: 25px; margin: 15px 0 }\
</style></head><body id=body><div id=a></div><div id=b></div><div id=c></div></body></html>";

/// The viewport the expectations are for.
pub const VIEWPORT: (f32, f32) = (800.0, 600.0);

/// Each box's id and its border box (x, y, width, height), in CSS px.
pub const EXPECT: &[(&str, [i32; 4])] = &[
    ("html", [0, 0, 800, 199]),
    ("body", [8, 10, 784, 174]),
    ("a", [28, 10, 200, 50]),
    ("b", [8, 90, 114, 54]),
    ("c", [8, 159, 784, 25]),
];
