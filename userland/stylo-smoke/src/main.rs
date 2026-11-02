// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.4 — `stylo-smoke`: upstream Stylo computes the styles of an
//! html5ever document on-target (survey:
//! `docs/RESEARCH-2026-10-30-ot61-c2.4-stylo-closure-SURVEY.md`).
//!
//! The DOM side Stylo needs (`TDocument`, `TNode`, `TElement`, and
//! `selectors::Element` over Stylo's `SelectorImpl`) is the job Servo's
//! `script` does in the real engine. Here it is a minimal arena: html5ever
//! builds an `Rc` tree through the same `TreeSink` as C2.3b's smoke, and it is
//! flattened into a `Vec<Node>` whose nodes refer to each other by index. The
//! shape of the trait impls follows Blitz's `blitz-dom` (`stylo.rs`, MIT OR
//! Apache-2.0, <https://github.com/dioxuslabs/blitz>), read for its approach;
//! this file implements the subset a static document needs (no animations, no
//! shadow DOM, no snapshots, no incremental restyle).
//!
//! The check: one sequential style traversal (Stylo's pool is off: `num_cpus`
//! has no arm for our OS), then every element in `EXPECT` must have the
//! computed `display`, `color`, `font-size` and `margin-left` written there.
//! Each value is derived by hand from the CSS cascade (origin, specificity,
//! `!important`, the `style` attribute), inheritance, `em`/`%` font sizes,
//! `var()`, and media queries against the 800 x 600 viewport the smoke gives
//! Stylo, not from running Stylo, so a wrong cascade fails.
//!
//! Exit 0 on a pass; a failed check prints `FAIL(n)` and exits `n`.

#![no_main]

use std::borrow::Cow;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode as HtmlQuirks, TreeSink};
use html5ever::{Attribute, LocalName, Namespace, QualName, local_name, ns, parse_document};
use neo_std_pal::SyscallHeap;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::{BLOOM_HASH_MASK, BloomFilter};
use selectors::matching::{ElementSelectorFlags, MatchingContext, VisitedHandlingMode};
use selectors::sink::Push;
use selectors::{Element as SelectorsElement, OpaqueElement};
use style::applicable_declarations::ApplicableDeclarationBlock;
use style::bloom::each_relevant_element_hash;
use style::color::AbsoluteColor;
use style::context::{
    QuirksMode, RegisteredSpeculativePainter, RegisteredSpeculativePainters, SharedStyleContext,
    StyleContext,
};
use style::data::{ElementDataMut, ElementDataRef, ElementDataWrapper};
use style::device::Device;
use style::device::servo::FontMetricsProvider;
use style::dom::{LayoutIterator, NodeInfo, OpaqueNode, TDocument, TElement, TNode, TShadowRoot};
use style::font_metrics::FontMetrics;
use style::global_style_data::{GLOBAL_STYLE_DATA, STYLE_THREAD_POOL};
use style::media_queries::{MediaList, MediaType};
use style::properties::style_structs::Font;
use style::properties::{ComputedValues, PropertyDeclarationBlock, parse_style_attribute};
use style::queries::values::PrefersColorScheme;
use style::selector_parser::{
    AttrValue, Lang, NonTSPseudoClass, PseudoElement, RestyleDamage, SelectorImpl, SnapshotMap,
};
use style::servo::media_features::PointerCapabilities;
use style::servo_arc::{Arc, ArcBorrow};
use style::shared_lock::{Locked, SharedRwLock, StylesheetGuards};
use style::stylesheets::{
    AllowImportRules, CssRuleType, DocumentStyleSheet, Origin, Stylesheet, UrlExtraData,
};
use style::stylist::{CascadeData, Stylist};
use style::thread_state::{self, ThreadState};
use style::traversal::{DomTraversal, recalc_style_at};
use style::traversal_flags::TraversalFlags;
use style::values::computed::font::{GenericFontFamily, QueryFontMetricsFlags};
use style::values::computed::{CSSPixelLength, Display, Length};
use style::values::generics::length::GenericMargin;
use style::values::{AtomIdent, GenericAtomIdent};
use style::{Atom, CaseSensitivityExt};
use style_dom::ElementState;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ stylo ]";

fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok: {what}");
    } else {
        println!("{TAG} FAIL({code}): {what}");
        std::process::exit(code);
    }
}

// --- The page and its hand-derived computed values --------------------------

/// The viewport Stylo is given (CSS px): it decides the two media queries.
const VIEWPORT: (f32, f32) = (800.0, 600.0);

/// The user-agent sheet: the few HTML defaults the page relies on.
const UA_CSS: &str = "html, body, div, p, h1, ul, li { display: block }\n\
head, style, title, script { display: none }\n\
span, b, a { display: inline }\n";

const PAGE: &str = "<!DOCTYPE html>\n\
<html><head><title>C2.4</title><style>\n\
:root { --accent: rgb(0, 128, 0); }\n\
body { color: rgb(10, 20, 30); font-size: 20px; }\n\
p { color: rgb(200, 0, 0); margin-left: 5px; }\n\
#main p.note { color: var(--accent); }\n\
.imp2 { color: rgb(9, 9, 9) !important; }\n\
.big { font-size: 2em; }\n\
span { font-size: 50%; }\n\
.hidden { display: none; }\n\
@media (min-width: 400px) { .wide { margin-left: 11px; } }\n\
@media (max-width: 400px) { .wide { margin-left: 99px; } }\n\
</style></head><body>\n\
<div id=\"main\">\n\
<p id=\"a\">plain</p>\n\
<p id=\"b\" class=\"note\">note</p>\n\
<p id=\"c\" style=\"color: rgb(1, 2, 3)\">inline</p>\n\
<p id=\"d\" class=\"imp2\" style=\"color: rgb(1, 2, 3)\">important</p>\n\
<p id=\"e\" class=\"big wide\">big <span id=\"f\">small</span></p>\n\
<p id=\"g\" class=\"hidden\">gone</p>\n\
</div>\n\
<div id=\"h\">div</div>\n\
</body></html>\n";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Disp {
    Block,
    Inline,
    None,
}

/// `(id, display, color rgb, font-size px, margin-left px)`.
/// - `body` sets the text color and 20px; `p` sets red and 5px margin.
/// - `#a`: only `p` applies.
/// - `#b`: `#main p.note` (1,1,1) beats `p` (0,0,1); `var(--accent)` is
///   inherited from `:root`.
/// - `#c`: the `style` attribute beats every author rule of normal importance.
/// - `#d`: an author `!important` beats the `style` attribute's normal one.
/// - `#e`: `2em` of the inherited 20px is 40px; the viewport is 800px wide, so
///   only the `min-width: 400px` query applies (11px).
/// - `#f`: `50%` of its parent's 40px is 20px; color and margin: the color is
///   inherited from `#e`, the margin is not (0, the initial value).
/// - `#g`: `display: none` (its other values are still computed).
/// - `#h`: a `div` gets no `p` rule: the color inherited from `body`, margin 0.
const EXPECT: &[(&str, Disp, (u8, u8, u8), f32, f32)] = &[
    ("a", Disp::Block, (200, 0, 0), 20.0, 5.0),
    ("b", Disp::Block, (0, 128, 0), 20.0, 5.0),
    ("c", Disp::Block, (1, 2, 3), 20.0, 5.0),
    ("d", Disp::Block, (9, 9, 9), 20.0, 5.0),
    ("e", Disp::Block, (200, 0, 0), 40.0, 11.0),
    ("f", Disp::Inline, (200, 0, 0), 20.0, 0.0),
    ("g", Disp::None, (200, 0, 0), 20.0, 5.0),
    ("h", Disp::Block, (10, 20, 30), 20.0, 0.0),
];

// --- Parsing: C2.3b's Rc sink, then an arena --------------------------------

#[derive(Debug)]
enum RcData {
    Document,
    Other,
    Text(RefCell<StrTendril>),
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,
    },
}

#[derive(Debug)]
struct RcNode {
    data: RcData,
    parent: RefCell<Option<Weak<RcNode>>>,
    children: RefCell<Vec<Rc<RcNode>>>,
}

type RcHandle = Rc<RcNode>;

fn rc_node(data: RcData) -> RcHandle {
    Rc::new(RcNode {
        data,
        parent: RefCell::new(None),
        children: RefCell::new(Vec::new()),
    })
}

fn rc_parent(n: &RcHandle) -> Option<RcHandle> {
    n.parent.borrow().as_ref().and_then(Weak::upgrade)
}

fn rc_append(parent: &RcHandle, child: RcHandle) {
    *child.parent.borrow_mut() = Some(Rc::downgrade(parent));
    parent.children.borrow_mut().push(child);
}

fn rc_detach(target: &RcHandle) {
    if let Some(parent) = rc_parent(target) {
        parent
            .children
            .borrow_mut()
            .retain(|c| !Rc::ptr_eq(c, target));
    }
    *target.parent.borrow_mut() = None;
}

fn rc_append_text(parent: &RcHandle, text: StrTendril) {
    if let Some(last) = parent.children.borrow().last()
        && let RcData::Text(t) = &last.data
    {
        t.borrow_mut().push_tendril(&text);
        return;
    }
    rc_append(parent, rc_node(RcData::Text(RefCell::new(text))));
}

struct Sink {
    document: RcHandle,
}

impl TreeSink for Sink {
    type Handle = RcHandle;
    type Output = RcHandle;
    type ElemName<'a>
        = &'a QualName
    where
        Self: 'a;

    fn finish(self) -> RcHandle {
        self.document
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> RcHandle {
        self.document.clone()
    }

    fn elem_name<'a>(&'a self, target: &'a RcHandle) -> &'a QualName {
        match &target.data {
            RcData::Element { name, .. } => name,
            _ => panic!("not an element"),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _: ElementFlags) -> RcHandle {
        rc_node(RcData::Element {
            name,
            attrs: RefCell::new(attrs),
        })
    }

    fn create_comment(&self, _text: StrTendril) -> RcHandle {
        rc_node(RcData::Other)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> RcHandle {
        rc_node(RcData::Other)
    }

    fn append(&self, parent: &RcHandle, child: NodeOrText<RcHandle>) {
        match child {
            NodeOrText::AppendNode(node) => rc_append(parent, node),
            NodeOrText::AppendText(text) => rc_append_text(parent, text),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &RcHandle,
        prev_element: &RcHandle,
        child: NodeOrText<RcHandle>,
    ) {
        if rc_parent(element).is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {
        rc_append(&self.document, rc_node(RcData::Other));
    }

    fn get_template_contents(&self, target: &RcHandle) -> RcHandle {
        // The page has no <template>.
        target.clone()
    }

    fn same_node(&self, x: &RcHandle, y: &RcHandle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn set_quirks_mode(&self, _mode: HtmlQuirks) {}

    fn append_before_sibling(&self, sibling: &RcHandle, new_node: NodeOrText<RcHandle>) {
        let parent = rc_parent(sibling).expect("sibling has a parent");
        let idx = parent
            .children
            .borrow()
            .iter()
            .position(|c| Rc::ptr_eq(c, sibling))
            .expect("sibling is a child of its parent");
        let node = match new_node {
            NodeOrText::AppendNode(node) => {
                rc_detach(&node);
                node
            }
            NodeOrText::AppendText(text) => rc_node(RcData::Text(RefCell::new(text))),
        };
        *node.parent.borrow_mut() = Some(Rc::downgrade(&parent));
        parent.children.borrow_mut().insert(idx, node);
    }

    fn add_attrs_if_missing(&self, target: &RcHandle, attrs: Vec<Attribute>) {
        if let RcData::Element {
            attrs: existing, ..
        } = &target.data
        {
            let mut existing = existing.borrow_mut();
            for a in attrs {
                if !existing.iter().any(|e| e.name == a.name) {
                    existing.push(a);
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &RcHandle) {
        rc_detach(target);
    }

    fn reparent_children(&self, node: &RcHandle, new_parent: &RcHandle) {
        let children = std::mem::take(&mut *node.children.borrow_mut());
        for c in children {
            rc_append(new_parent, c);
        }
    }
}

// --- The arena DOM Stylo styles ---------------------------------------------

enum NodeData {
    Document,
    Text,
    Other,
    Element(ElementData),
}

struct ElementData {
    name: QualName,
    attrs: Vec<Attribute>,
    id: Option<Atom>,
    style_attribute: Option<Arc<Locked<PropertyDeclarationBlock>>>,
}

/// Stylo's per-element data. Stylo's traversal has exclusive access to a node
/// while it styles it, which is what makes the interior mutability sound (the
/// same contract as Blitz's `StyloData`).
#[derive(Default)]
struct StyloData(UnsafeCell<Option<ElementDataWrapper>>);

impl StyloData {
    fn has_data(&self) -> bool {
        // SAFETY: a shared read; writers hold Stylo's exclusive access.
        unsafe { &*self.0.get() }.is_some()
    }

    fn get(&self) -> Option<ElementDataRef<'_>> {
        // SAFETY: as `has_data`.
        unsafe { &*self.0.get() }.as_ref().map(|w| w.borrow())
    }

    /// # Safety
    /// Only during Stylo's traversal, which has exclusive access to the node.
    unsafe fn get_mut(&self) -> Option<ElementDataMut<'_>> {
        unsafe { &mut *self.0.get() }
            .as_mut()
            .map(|w| w.borrow_mut())
    }

    /// # Safety
    /// As `get_mut`, and no borrow of the data may be outstanding.
    unsafe fn ensure(&self) -> ElementDataMut<'_> {
        if !self.has_data() {
            unsafe { *self.0.get() = Some(ElementDataWrapper::default()) };
        }
        unsafe { self.get_mut() }.expect("just initialised")
    }

    /// # Safety
    /// As `ensure`.
    unsafe fn clear(&self) {
        unsafe { *self.0.get() = None };
    }
}

struct Node {
    id: usize,
    tree: *const Tree,
    parent: Option<usize>,
    children: Vec<usize>,
    data: NodeData,
    stylo: StyloData,
    selector_flags: Cell<ElementSelectorFlags>,
    dirty_descendants: AtomicBool,
    handled_snapshot: AtomicBool,
}

// SAFETY: Stylo requires its node handles to be Send + Sync. The tree is not
// mutated while it is styled except through `StyloData`/the atomics, whose
// access Stylo's traversal serialises per node; the traversal here is
// sequential besides.
unsafe impl Send for Node {}
unsafe impl Sync for Node {}

struct Tree {
    nodes: Vec<Node>,
    lock: SharedRwLock,
}

impl Node {
    fn with(&self, id: usize) -> &Node {
        // SAFETY: `tree` points at the `Tree` that owns this node; the tree is
        // boxed and outlives every handle given out.
        unsafe { &(&(*self.tree).nodes)[id] }
    }

    fn tree(&self) -> &Tree {
        // SAFETY: as `with`.
        unsafe { &*self.tree }
    }

    fn element(&self) -> Option<&ElementData> {
        match &self.data {
            NodeData::Element(e) => Some(e),
            _ => None,
        }
    }

    fn attr(&self, local: &LocalName) -> Option<&str> {
        self.element()?
            .attrs
            .iter()
            .find(|a| a.name.ns == ns!() && a.name.local == *local)
            .map(|a| &*a.value)
    }

    fn sibling(&self, step: isize) -> Option<&Node> {
        let parent = self.with(self.parent?);
        let at = parent.children.iter().position(|&c| c == self.id)? as isize + step;
        (at >= 0)
            .then(|| parent.children.get(at as usize))
            .flatten()
            .map(|&c| self.with(c))
    }
}

/// The Stylo-facing handle: a node of the arena.
type N<'a> = &'a Node;

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.element() {
            Some(e) => write!(f, "<{}#{}>", e.name.local, self.id),
            None => write!(f, "node#{}", self.id),
        }
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Node {}

impl Hash for Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.id)
    }
}

/// Flatten the parsed tree into `Tree`, parsing every `style` attribute.
fn build_tree(doc: &RcHandle, url: &UrlExtraData) -> Box<Tree> {
    let mut tree = Box::new(Tree {
        nodes: Vec::new(),
        lock: SharedRwLock::new(),
    });
    let tree_ptr: *const Tree = &*tree;
    fn walk(
        tree: &mut Tree,
        tree_ptr: *const Tree,
        url: &UrlExtraData,
        n: &RcHandle,
        parent: Option<usize>,
    ) -> usize {
        let data = match &n.data {
            RcData::Document => NodeData::Document,
            RcData::Text(_) => NodeData::Text,
            RcData::Other => NodeData::Other,
            RcData::Element { name, attrs } => {
                let attrs = attrs.borrow().clone();
                let find = |l: &LocalName| {
                    attrs
                        .iter()
                        .find(|a| a.name.ns == ns!() && a.name.local == *l)
                        .map(|a| a.value.to_string())
                };
                let id = find(&local_name!("id")).map(|v| Atom::from(v.as_str()));
                let style_attribute = find(&local_name!("style")).map(|v| {
                    Arc::new(tree.lock.wrap(parse_style_attribute(
                        &v,
                        url,
                        None,
                        QuirksMode::NoQuirks,
                        CssRuleType::Style,
                    )))
                });
                NodeData::Element(ElementData {
                    name: name.clone(),
                    attrs,
                    id,
                    style_attribute,
                })
            }
        };
        let id = tree.nodes.len();
        tree.nodes.push(Node {
            id,
            tree: tree_ptr,
            parent,
            children: Vec::new(),
            data,
            stylo: StyloData::default(),
            selector_flags: Cell::new(ElementSelectorFlags::empty()),
            dirty_descendants: AtomicBool::new(true),
            handled_snapshot: AtomicBool::new(false),
        });
        let kids: Vec<RcHandle> = n.children.borrow().clone();
        for k in &kids {
            let c = walk(tree, tree_ptr, url, k, Some(id));
            tree.nodes[id].children.push(c);
        }
        id
    }
    walk(&mut tree, tree_ptr, url, doc, None);
    tree
}

/// The text of every `<style>` element, in document order.
fn author_css(tree: &Tree, rc: &RcHandle) -> String {
    let _ = tree;
    let mut css = String::new();
    fn walk(n: &RcHandle, in_style: bool, css: &mut String) {
        match &n.data {
            RcData::Text(t) if in_style => css.push_str(&t.borrow()),
            RcData::Element { name, .. } => {
                let style = name.local == local_name!("style");
                for c in n.children.borrow().iter() {
                    walk(c, style, css);
                }
                return;
            }
            _ => {}
        }
        for c in n.children.borrow().iter() {
            walk(c, in_style, css);
        }
    }
    walk(rc, false, &mut css);
    css
}

// --- Stylo's traits ---------------------------------------------------------

impl<'a> TDocument for N<'a> {
    type ConcreteNode = N<'a>;

    fn as_node(&self) -> N<'a> {
        self
    }

    fn is_html_document(&self) -> bool {
        true
    }

    fn quirks_mode(&self) -> QuirksMode {
        QuirksMode::NoQuirks
    }

    fn shared_lock(&self) -> &SharedRwLock {
        &self.tree().lock
    }
}

impl NodeInfo for N<'_> {
    fn is_element(&self) -> bool {
        matches!(self.data, NodeData::Element(_))
    }

    fn is_text_node(&self) -> bool {
        matches!(self.data, NodeData::Text)
    }
}

impl<'a> TShadowRoot for N<'a> {
    type ConcreteNode = N<'a>;

    fn as_node(&self) -> N<'a> {
        self
    }

    fn host(&self) -> N<'a> {
        unreachable!("no shadow roots: as_shadow_root() is always None")
    }

    fn style_data<'b>(&self) -> Option<&'b CascadeData>
    where
        Self: 'b,
    {
        None
    }
}

impl<'a> TNode for N<'a> {
    type ConcreteElement = N<'a>;
    type ConcreteDocument = N<'a>;
    type ConcreteShadowRoot = N<'a>;

    fn parent_node(&self) -> Option<Self> {
        self.parent.map(|p| self.with(p))
    }

    fn first_child(&self) -> Option<Self> {
        self.children.first().map(|&c| self.with(c))
    }

    fn last_child(&self) -> Option<Self> {
        self.children.last().map(|&c| self.with(c))
    }

    fn prev_sibling(&self) -> Option<Self> {
        self.sibling(-1)
    }

    fn next_sibling(&self) -> Option<Self> {
        self.sibling(1)
    }

    fn owner_doc(&self) -> N<'a> {
        self.with(0)
    }

    fn is_in_document(&self) -> bool {
        true
    }

    fn traversal_parent(&self) -> Option<N<'a>> {
        self.parent_node().filter(|p| p.is_element())
    }

    fn opaque(&self) -> OpaqueNode {
        OpaqueNode(self.id)
    }

    fn debug_id(self) -> usize {
        self.id
    }

    fn as_element(&self) -> Option<N<'a>> {
        self.is_element().then_some(*self)
    }

    fn as_document(&self) -> Option<N<'a>> {
        matches!(self.data, NodeData::Document).then_some(*self)
    }

    fn as_shadow_root(&self) -> Option<N<'a>> {
        None
    }
}

impl SelectorsElement for N<'_> {
    type Impl = SelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::from_non_null_ptr(
            NonNull::new(self.id.wrapping_add(1) as *mut ()).expect("id + 1 is non-zero"),
        )
    }

    fn parent_element(&self) -> Option<Self> {
        TNode::traversal_parent(self)
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        let mut step = -1;
        while let Some(n) = self.sibling(step) {
            if n.is_element() {
                return Some(n);
            }
            step -= 1;
        }
        None
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let mut step = 1;
        while let Some(n) = self.sibling(step) {
            if n.is_element() {
                return Some(n);
            }
            step += 1;
        }
        None
    }

    fn first_element_child(&self) -> Option<Self> {
        self.children
            .iter()
            .map(|&c| self.with(c))
            .find(|c| c.is_element())
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.element().is_some_and(|e| e.name.ns == ns!(html))
    }

    fn has_local_name(&self, local_name: &LocalName) -> bool {
        self.element().is_some_and(|e| e.name.local == *local_name)
    }

    fn has_namespace(&self, ns: &Namespace) -> bool {
        self.element().is_some_and(|e| e.name.ns == *ns)
    }

    fn is_same_type(&self, other: &Self) -> bool {
        match (self.element(), other.element()) {
            (Some(a), Some(b)) => a.name.local == b.name.local && a.name.ns == b.name.ns,
            _ => false,
        }
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&GenericAtomIdent<html5ever::NamespaceStaticSet>>,
        local_name: &GenericAtomIdent<html5ever::LocalNameStaticSet>,
        operation: &AttrSelectorOperation<&AttrValue>,
    ) -> bool {
        let Some(e) = self.element() else {
            return false;
        };
        e.attrs.iter().any(|a| {
            a.name.local == local_name.0
                && match ns {
                    NamespaceConstraint::Any => true,
                    NamespaceConstraint::Specific(ns) => a.name.ns == ns.0,
                }
                && operation.eval_str(&a.value)
        })
    }

    fn match_non_ts_pseudo_class(
        &self,
        pc: &NonTSPseudoClass,
        _context: &mut MatchingContext<SelectorImpl>,
    ) -> bool {
        // A static document: no interaction state, so only `:link` and
        // `:any-link` can match (an `<a href>`).
        match pc {
            NonTSPseudoClass::Link | NonTSPseudoClass::AnyLink => self.is_link(),
            _ => false,
        }
    }

    fn match_pseudo_element(
        &self,
        _pe: &PseudoElement,
        _context: &mut MatchingContext<SelectorImpl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, flags: ElementSelectorFlags) {
        let own = flags.for_self();
        if !own.is_empty() {
            self.selector_flags.set(self.selector_flags.get() | own);
        }
        let parent = flags.for_parent();
        if !parent.is_empty()
            && let Some(p) = TNode::parent_node(self)
        {
            p.selector_flags.set(p.selector_flags.get() | parent);
        }
    }

    fn is_link(&self) -> bool {
        self.has_local_name(&local_name!("a")) && self.attr(&local_name!("href")).is_some()
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &AtomIdent, case_sensitivity: CaseSensitivity) -> bool {
        self.element()
            .and_then(|e| e.id.as_ref())
            .is_some_and(|own| case_sensitivity.eq_atom(own, id))
    }

    fn has_class(&self, name: &AtomIdent, case_sensitivity: CaseSensitivity) -> bool {
        self.attr(&local_name!("class")).is_some_and(|v| {
            v.split_ascii_whitespace()
                .any(|c| case_sensitivity.eq_atom(&Atom::from(c), name))
        })
    }

    fn has_custom_state(&self, _name: &AtomIdent) -> bool {
        false
    }

    fn imported_part(&self, _name: &AtomIdent) -> Option<AtomIdent> {
        None
    }

    fn is_part(&self, _name: &AtomIdent) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        self.children.iter().all(|&c| match &self.with(c).data {
            NodeData::Element(_) | NodeData::Text => false,
            _ => true,
        })
    }

    fn is_root(&self) -> bool {
        TNode::parent_node(self).is_some_and(|p| matches!(p.data, NodeData::Document))
    }

    fn add_element_unique_hashes(&self, filter: &mut BloomFilter) -> bool {
        each_relevant_element_hash(*self, |hash| filter.insert_hash(hash & BLOOM_HASH_MASK));
        true
    }
}

/// The children Stylo walks: every child, in order.
struct Children<'a> {
    parent: N<'a>,
    at: usize,
}

impl<'a> Iterator for Children<'a> {
    type Item = N<'a>;

    fn next(&mut self) -> Option<N<'a>> {
        let id = *self.parent.children.get(self.at)?;
        self.at += 1;
        Some(self.parent.with(id))
    }
}

impl<'a> TElement for N<'a> {
    type ConcreteNode = N<'a>;
    type TraversalChildrenIterator = Children<'a>;

    fn as_node(&self) -> N<'a> {
        self
    }

    fn traversal_children(&self) -> LayoutIterator<Children<'a>> {
        LayoutIterator(Children {
            parent: *self,
            at: 0,
        })
    }

    fn is_html_element(&self) -> bool {
        self.is_html_element_in_html_document()
    }

    fn is_mathml_element(&self) -> bool {
        self.element().is_some_and(|e| e.name.ns == ns!(mathml))
    }

    fn is_svg_element(&self) -> bool {
        self.element().is_some_and(|e| e.name.ns == ns!(svg))
    }

    fn style_attribute(&self) -> Option<ArcBorrow<'_, Locked<PropertyDeclarationBlock>>> {
        self.element()?
            .style_attribute
            .as_ref()
            .map(|a| a.borrow_arc())
    }

    fn animation_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }

    fn transition_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }

    fn state(&self) -> ElementState {
        ElementState::empty()
    }

    fn has_part_attr(&self) -> bool {
        false
    }

    fn exports_any_part(&self) -> bool {
        false
    }

    fn id(&self) -> Option<&Atom> {
        self.element()?.id.as_ref()
    }

    fn each_class<F>(&self, mut callback: F)
    where
        F: FnMut(&AtomIdent),
    {
        if let Some(v) = self.attr(&local_name!("class")) {
            for c in v.split_ascii_whitespace() {
                callback(AtomIdent::cast(&Atom::from(c)));
            }
        }
    }

    fn each_custom_state<F>(&self, _callback: F)
    where
        F: FnMut(&AtomIdent),
    {
    }

    fn each_attr_name<F>(&self, mut callback: F)
    where
        F: FnMut(&style::LocalName),
    {
        if let Some(e) = self.element() {
            for a in &e.attrs {
                callback(&GenericAtomIdent(a.name.local.clone()));
            }
        }
    }

    fn has_dirty_descendants(&self) -> bool {
        self.dirty_descendants.load(Ordering::Relaxed)
    }

    fn has_snapshot(&self) -> bool {
        false
    }

    fn handled_snapshot(&self) -> bool {
        self.handled_snapshot.load(Ordering::Relaxed)
    }

    unsafe fn set_handled_snapshot(&self) {
        self.handled_snapshot.store(true, Ordering::Relaxed);
    }

    unsafe fn set_dirty_descendants(&self) {
        self.dirty_descendants.store(true, Ordering::Relaxed);
    }

    unsafe fn unset_dirty_descendants(&self) {
        self.dirty_descendants.store(false, Ordering::Relaxed);
    }

    fn store_children_to_process(&self, _n: isize) {
        unreachable!("only the parallel traversal counts children; this one is sequential")
    }

    fn did_process_child(&self) -> isize {
        unreachable!("only the parallel traversal counts children; this one is sequential")
    }

    unsafe fn ensure_data(&self) -> ElementDataMut<'_> {
        // SAFETY: called by Stylo's traversal, which owns the node meanwhile.
        unsafe { self.stylo.ensure() }
    }

    unsafe fn clear_data(&self) {
        // SAFETY: as `ensure_data`.
        unsafe { self.stylo.clear() }
    }

    fn has_data(&self) -> bool {
        self.stylo.has_data()
    }

    fn borrow_data(&self) -> Option<ElementDataRef<'_>> {
        self.stylo.get()
    }

    fn mutate_data(&self) -> Option<ElementDataMut<'_>> {
        // SAFETY: Stylo calls this only on the node it is styling.
        unsafe { self.stylo.get_mut() }
    }

    fn skip_item_display_fixup(&self) -> bool {
        false
    }

    fn may_have_animations(&self) -> bool {
        false
    }

    fn has_animations(&self, _context: &SharedStyleContext) -> bool {
        false
    }

    fn has_css_animations(
        &self,
        _context: &SharedStyleContext,
        _pseudo_element: Option<PseudoElement>,
    ) -> bool {
        false
    }

    fn has_css_transitions(
        &self,
        _context: &SharedStyleContext,
        _pseudo_element: Option<PseudoElement>,
    ) -> bool {
        false
    }

    fn shadow_root(&self) -> Option<N<'a>> {
        None
    }

    fn containing_shadow(&self) -> Option<N<'a>> {
        None
    }

    fn lang_attr(&self) -> Option<AttrValue> {
        None
    }

    fn match_element_lang(&self, _override_lang: Option<Option<AttrValue>>, _value: &Lang) -> bool {
        false
    }

    fn is_html_document_body_element(&self) -> bool {
        self.has_local_name(&local_name!("body"))
            && TNode::parent_node(self).is_some_and(|p| SelectorsElement::is_root(&p))
    }

    fn synthesize_presentational_hints_for_legacy_attributes<V>(
        &self,
        _visited_handling: VisitedHandlingMode,
        _hints: &mut V,
    ) where
        V: Push<ApplicableDeclarationBlock>,
    {
        // The page uses no legacy presentational attributes.
    }

    fn local_name(&self) -> &html5ever::LocalName {
        &self.element().expect("an element").name.local
    }

    fn namespace(&self) -> &html5ever::Namespace {
        &self.element().expect("an element").name.ns
    }

    fn query_container_size(
        &self,
        _display: &Display,
    ) -> euclid::default::Size2D<Option<app_units::Au>> {
        Default::default()
    }

    fn has_selector_flags(&self, flags: ElementSelectorFlags) -> bool {
        self.selector_flags.get().contains(flags)
    }

    fn relative_selector_search_direction(&self) -> ElementSelectorFlags {
        self.selector_flags
            .get()
            .intersection(ElementSelectorFlags::RELATIVE_SELECTOR_SEARCH_DIRECTION_ANCESTOR_SIBLING)
    }

    fn get_attr(&self, attr: &style::LocalName, namespace: &style::Namespace) -> Option<String> {
        self.element()?
            .attrs
            .iter()
            .find(|a| a.name.local == attr.0 && a.name.ns == namespace.0)
            .map(|a| a.value.to_string())
    }

    fn compute_layout_damage(_old: &ComputedValues, _new: &ComputedValues) -> RestyleDamage {
        RestyleDamage::empty()
    }
}

// --- Styling ----------------------------------------------------------------

/// Fixed font metrics: the smoke checks no font-relative unit but `em` and
/// `%`, which Stylo resolves without them.
#[derive(Debug)]
struct NoFonts;

impl FontMetricsProvider for NoFonts {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        _font: &Font,
        _base_size: CSSPixelLength,
        _flags: QueryFontMetricsFlags,
    ) -> FontMetrics {
        FontMetrics::default()
    }

    fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
        Length::new(16.0)
    }
}

struct NoPainters;

impl RegisteredSpeculativePainters for NoPainters {
    fn get(&self, _name: &Atom) -> Option<&dyn RegisteredSpeculativePainter> {
        None
    }
}

struct Recalc<'a> {
    context: SharedStyleContext<'a>,
}

impl<'dom> DomTraversal<N<'dom>> for Recalc<'_> {
    fn process_preorder<F: FnMut(N<'dom>)>(
        &self,
        context: &mut StyleContext<N<'dom>>,
        node: N<'dom>,
        note_child: F,
    ) {
        if let Some(el) = node.as_element() {
            // SAFETY: the traversal owns `el` while it styles it.
            let mut data = unsafe { el.ensure_data() };
            recalc_style_at(self, context, el, &mut data, note_child);
            // SAFETY: as above.
            unsafe { el.unset_dirty_descendants() };
        }
    }

    fn needs_postorder_traversal() -> bool {
        false
    }

    fn process_postorder(&self, _context: &mut StyleContext<N<'dom>>, _node: N<'dom>) {
        unreachable!("needs_postorder_traversal() is false")
    }

    fn shared_context(&self) -> &SharedStyleContext<'_> {
        &self.context
    }
}

fn sheet(css: &str, origin: Origin, url: &UrlExtraData, lock: &SharedRwLock) -> DocumentStyleSheet {
    DocumentStyleSheet(Arc::new(Stylesheet::from_str(
        css,
        url.clone(),
        origin,
        Arc::new(lock.wrap(MediaList::empty())),
        lock.clone(),
        None,
        None,
        QuirksMode::NoQuirks,
        AllowImportRules::No,
    )))
}

/// Parse, style, and return the tree (with every element's styles stored).
fn style_page() -> Box<Tree> {
    let rc = parse_document(
        Sink {
            document: rc_node(RcData::Document),
        },
        Default::default(),
    )
    .one(StrTendril::from(PAGE));
    let url = UrlExtraData::from(url::Url::parse("about:blank").expect("about:blank"));
    let tree = build_tree(&rc, &url);
    let author = author_css(&tree, &rc);

    let device = Device::new(
        MediaType::screen(),
        QuirksMode::NoQuirks,
        euclid::Size2D::new(VIEWPORT.0, VIEWPORT.1),
        euclid::Size2D::new(VIEWPORT.0, VIEWPORT.1),
        euclid::Scale::new(1.0),
        Box::new(NoFonts),
        ComputedValues::initial_values_with_font_override(Font::initial_values()),
        PrefersColorScheme::Light,
        PointerCapabilities::default(),
        PointerCapabilities::default(),
    );
    let mut stylist = Stylist::new(device, QuirksMode::NoQuirks);
    {
        let read = tree.lock.read();
        stylist.append_stylesheet(sheet(UA_CSS, Origin::UserAgent, &url, &tree.lock), &read);
        stylist.append_stylesheet(sheet(&author, Origin::Author, &url, &tree.lock), &read);
    }

    thread_state::enter(ThreadState::LAYOUT);
    let read = tree.lock.read();
    let guards = StylesheetGuards {
        author: &read,
        ua_or_user: &read,
    };
    let html: N<'_> = TDocument::as_node(&&tree.nodes[0])
        .first_element_child()
        .expect("<html>");
    stylist.flush(&guards).process_style(html, None);
    let snapshots = SnapshotMap::new();
    let context = SharedStyleContext {
        traversal_flags: TraversalFlags::empty(),
        stylist: &stylist,
        options: GLOBAL_STYLE_DATA.options.clone(),
        guards,
        visited_styles_enabled: false,
        animations: Default::default(),
        current_time_for_animations: 0.0,
        snapshot_map: &snapshots,
        registered_speculative_painters: &NoPainters,
    };
    let token = Recalc::pre_traverse(html, &context);
    if token.should_traverse() {
        let traversal = Recalc { context };
        style::driver::traverse_dom(&traversal, token, None);
    }
    drop(read);
    thread_state::exit(ThreadState::LAYOUT);
    tree
}

fn by_id<'a>(tree: &'a Tree, id: &str) -> Option<&'a Node> {
    tree.nodes.iter().find(|n| {
        n.element()
            .and_then(|e| e.id.as_ref())
            .is_some_and(|a| &**a == id)
    })
}

fn margin_left_px(style: &ComputedValues) -> Option<f32> {
    match &style.get_margin().margin_left {
        GenericMargin::LengthPercentage(lp) => lp.to_length().map(|l| l.px()),
        _ => None,
    }
}

/// The stack the style traversal runs on. The process's main thread has the
/// kernel's 64 KiB user stack, and Stylo's debug-build traversal overflowed
/// it (a ring-3 #PF about 21 KiB below the stack's base, CI run 41276778648).
/// Servo styles on threads with large stacks for the same reason; 8 MiB is
/// half the kernel's per-thread ceiling (`MAX_MAP_REGION_SIZE`, 16 MiB).
const STYLE_STACK: usize = 8 * 1024 * 1024;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("{TAG} start: stylo 0.22.0 over html5ever 0.40.1 (vendored), sequential traversal");
    let styled = std::thread::Builder::new()
        .name("style".into())
        .stack_size(STYLE_STACK)
        .spawn(run)
        .map(|t| t.join().is_ok());
    check(
        3,
        "the style thread ran to the end",
        matches!(styled, Ok(true)),
    );
    println!("{TAG} pass");
    std::process::exit(0)
}

/// The checks, on the style thread. A failed check exits the whole process.
fn run() {
    let tree = style_page();
    // Survey §3b item 1: num_cpus has no arm for our OS (it returns 1), so
    // Stylo's default builds no pool and styles on this thread. A change
    // that turns the pool on must show up here, not be inherited.
    check(
        2,
        "Stylo built no thread pool (num_cpus is 1 here)",
        STYLE_THREAD_POOL.num_threads.is_none(),
    );
    check(
        1,
        "the traversal styled <html>",
        by_id(&tree, "a").is_some_and(|n| n.stylo.has_data()),
    );
    for (i, &(id, disp, (r, g, b), font_px, margin_px)) in EXPECT.iter().enumerate() {
        let code = 10 + i as i32;
        let Some(node) = by_id(&tree, id) else {
            check(code, &format!("#{id} exists"), false);
            unreachable!();
        };
        let Some(data) = node.stylo.get() else {
            check(code, &format!("#{id} was styled"), false);
            unreachable!();
        };
        let style = data.styles.primary();
        let got_disp = match style.get_box().clone_display() {
            d if d == Display::None => Some(Disp::None),
            d if d == Display::Block => Some(Disp::Block),
            d if d == Display::Inline => Some(Disp::Inline),
            _ => None,
        };
        let got_color = style.get_inherited_text().clone_color();
        let want_color = AbsoluteColor::srgb_legacy(r, g, b, 1.0);
        let got_font = style.get_font().font_size.computed_size().px();
        let got_margin = margin_left_px(style);
        println!(
            "{TAG} #{id}: display {got_disp:?} color {got_color:?} font-size {got_font}px margin-left {got_margin:?}"
        );
        check(
            code,
            &format!(
                "#{id}: display {disp:?}, rgb({r},{g},{b}), {font_px}px, margin-left {margin_px}px"
            ),
            got_disp == Some(disp)
                && got_color == want_color
                && (got_font - font_px).abs() < 0.01
                && got_margin.is_some_and(|m| (m - margin_px).abs() < 0.01),
        );
    }
}
