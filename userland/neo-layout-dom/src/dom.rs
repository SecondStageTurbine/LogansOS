// SPDX-License-Identifier: MPL-2.0
//! The arena: html5ever parses into an `Rc` tree (C2.3b's sink), which is
//! flattened into a boxed `Document` whose nodes refer to each other by index.
//! The tree's shape never changes after `Document::parse`; only the per-node
//! style and layout data, written by Stylo and layout, do.

use std::borrow::Cow;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::c_void;
use std::rc::{Rc, Weak};
use std::sync::atomic::AtomicBool;

use atomic_refcell::AtomicRefCell;
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode as HtmlQuirks, TreeSink};
use html5ever::{Attribute, LocalName, QualName, local_name, ns, parse_document};
use layout_api::{GenericLayoutData, StyleData, TrustedNodeAddress};
use selectors::matching::ElementSelectorFlags;
use servo_arc::Arc;
use style::Atom;
use style::attr::AttrValue;
use style::context::QuirksMode;
use style::properties::{PropertyDeclarationBlock, parse_style_attribute};
use style::shared_lock::{Locked, SharedRwLock};
use style::stylesheets::{CssRuleType, UrlExtraData};

/// The style locks layout reads (`ServoDangerousStyleDocument::shared_style_locks`),
/// with the field names of `script`'s `SharedRwLocks`.
pub struct SharedRwLocks {
    /// Author stylesheets and `style` attributes.
    pub author: SharedRwLock,
    /// User-agent and user stylesheets.
    pub ua_or_user: SharedRwLock,
}

pub(crate) enum NodeData {
    Document,
    Text(AtomicRefCell<String>),
    Other,
    Element(ElementData),
}

pub(crate) struct Attr {
    pub(crate) name: QualName,
    pub(crate) value: AttrValue,
}

pub(crate) struct ElementData {
    pub(crate) name: QualName,
    pub(crate) attrs: Vec<Attr>,
    pub(crate) id: Option<Atom>,
    pub(crate) style_attribute: Option<Arc<Locked<PropertyDeclarationBlock>>>,
}

/// One node of a [`Document`].
pub struct Node {
    pub(crate) id: usize,
    pub(crate) doc: *const Document,
    pub(crate) parent: Option<usize>,
    pub(crate) children: Vec<usize>,
    pub(crate) data: NodeData,
    /// Stylo's data for an element. Written only by Stylo's traversal, which
    /// has the node to itself meanwhile (the contract `script` relies on too).
    pub(crate) style_data: UnsafeCell<Option<Box<StyleData>>>,
    /// Layout's box-tree data. Written once, by layout, before it reads it.
    pub(crate) layout_data: UnsafeCell<Option<Box<GenericLayoutData>>>,
    pub(crate) selector_flags: Cell<ElementSelectorFlags>,
    pub(crate) dirty_descendants: AtomicBool,
    pub(crate) has_snapshot: AtomicBool,
    pub(crate) handled_snapshot: AtomicBool,
}

// SAFETY: Stylo and layout require their node handles to be Send + Sync. The
// tree's shape is immutable after parsing; the interior-mutable fields are
// written under the per-node exclusivity Stylo's and layout's traversals give
// (the same contract `script`'s `LayoutDom` wrappers rely on).
unsafe impl Send for Node {}
unsafe impl Sync for Node {}

/// A parsed static document. Boxed so node addresses stay put.
pub struct Document {
    pub(crate) nodes: Vec<Node>,
    pub(crate) locks: SharedRwLocks,
    pub(crate) quirks_mode: QuirksMode,
}

impl Node {
    pub(crate) fn doc(&self) -> &Document {
        // SAFETY: `doc` points at the boxed `Document` that owns this node,
        // which outlives every handle given out.
        unsafe { &*self.doc }
    }

    pub(crate) fn with(&self, id: usize) -> &Node {
        &self.doc().nodes[id]
    }

    pub(crate) fn element(&self) -> Option<&ElementData> {
        match &self.data {
            NodeData::Element(e) => Some(e),
            _ => None,
        }
    }

    pub(crate) fn attr_value(
        &self,
        ns: &html5ever::Namespace,
        local: &LocalName,
    ) -> Option<&AttrValue> {
        self.element()?
            .attrs
            .iter()
            .find(|a| a.name.ns == *ns && a.name.local == *local)
            .map(|a| &a.value)
    }

    pub(crate) fn attr(&self, local: &LocalName) -> Option<&str> {
        self.attr_value(&ns!(), local).map(|v| &**v)
    }

    pub(crate) fn sibling(&self, step: isize) -> Option<&Node> {
        let parent = self.with(self.parent?);
        let at = parent.children.iter().position(|&c| c == self.id)? as isize + step;
        (at >= 0)
            .then(|| parent.children.get(at as usize))
            .flatten()
            .map(|&c| self.with(c))
    }

    pub(crate) fn style_data(&self) -> Option<&StyleData> {
        // SAFETY: a shared read; Stylo writes it only with the node to itself.
        unsafe { &*self.style_data.get() }.as_deref()
    }

    /// # Safety
    /// Only while the caller has the node to itself (Stylo's traversal, or
    /// layout's initialisation of an unstyled element).
    pub(crate) unsafe fn ensure_style_data(&self) -> &StyleData {
        if self.style_data().is_none() {
            unsafe { *self.style_data.get() = Some(Box::default()) };
        }
        self.style_data().expect("just initialised")
    }

    /// # Safety
    /// As `ensure_style_data`, with no borrow of the data outstanding.
    pub(crate) unsafe fn clear_style_data(&self) {
        unsafe { *self.style_data.get() = None };
    }

    pub(crate) fn layout_data(&self) -> Option<&GenericLayoutData> {
        // SAFETY: a shared read; written once, before it is read.
        unsafe { &*self.layout_data.get() }.as_deref()
    }

    /// # Safety
    /// Only while the caller has the node to itself.
    pub(crate) unsafe fn set_layout_data(&self, data: Box<GenericLayoutData>) {
        unsafe { *self.layout_data.get() = Some(data) };
    }
}

impl Document {
    /// Parse `html` (no scripts run: there is no script engine) and parse each
    /// element's `style` attribute against `url`.
    pub fn parse(html: &str, url: &UrlExtraData) -> Box<Document> {
        let sink = Sink {
            document: rc_node(RcData::Document),
        };
        let rc = parse_document(sink, Default::default()).one(html);
        build(&rc, url)
    }

    /// The document node.
    pub fn document_node(&self) -> &Node {
        &self.nodes[0]
    }

    /// How many nodes the document has (the document node included).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the document has only its document node.
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// The address layout's messages carry for node `id` (`ServoLayoutNode::new`).
    pub fn address(&self, id: usize) -> TrustedNodeAddress {
        TrustedNodeAddress(&self.nodes[id] as *const Node as *const c_void)
    }

    /// The first element with `id` attribute `value`.
    pub fn element_by_id(&self, value: &str) -> Option<usize> {
        self.nodes
            .iter()
            .find(|n| {
                n.element()
                    .and_then(|e| e.id.as_ref())
                    .is_some_and(|a| &**a == value)
            })
            .map(|n| n.id)
    }

    /// The style locks.
    pub fn shared_style_locks(&self) -> &SharedRwLocks {
        &self.locks
    }

    /// The text of every `<style>` element, in document order.
    pub fn style_texts(&self) -> String {
        let mut css = String::new();
        for n in &self.nodes {
            if n.element()
                .is_some_and(|e| e.name.local == local_name!("style"))
            {
                for &c in &n.children {
                    if let NodeData::Text(t) = &self.nodes[c].data {
                        css.push_str(&t.borrow());
                    }
                }
            }
        }
        css
    }
}

// --- html5ever's sink: an Rc tree -------------------------------------------

enum RcData {
    Document,
    Other,
    Text(RefCell<StrTendril>),
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,
    },
}

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
        // A static page's <template> contents are inert; keep them in place.
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

/// An attribute's value as `script` stores it: `id` as an atom, `class` as a
/// token list, everything else as a string.
fn attr_value(name: &QualName, value: &str) -> AttrValue {
    if name.ns == ns!() && name.local == local_name!("class") {
        AttrValue::from_serialized_tokenlist(value.to_owned())
    } else if name.ns == ns!() && name.local == local_name!("id") {
        AttrValue::from_atomic(value.to_owned())
    } else {
        AttrValue::String(value.to_owned())
    }
}

fn build(doc: &RcHandle, url: &UrlExtraData) -> Box<Document> {
    let mut document = Box::new(Document {
        nodes: Vec::new(),
        locks: SharedRwLocks {
            author: SharedRwLock::new(),
            ua_or_user: SharedRwLock::new(),
        },
        quirks_mode: QuirksMode::NoQuirks,
    });
    let doc_ptr: *const Document = &*document;
    fn walk(
        document: &mut Document,
        doc_ptr: *const Document,
        url: &UrlExtraData,
        n: &RcHandle,
        parent: Option<usize>,
    ) -> usize {
        let data = match &n.data {
            RcData::Document => NodeData::Document,
            RcData::Text(t) => NodeData::Text(AtomicRefCell::new(t.borrow().to_string())),
            RcData::Other => NodeData::Other,
            RcData::Element { name, attrs } => {
                let attrs: Vec<Attr> = attrs
                    .borrow()
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: attr_value(&a.name, &a.value),
                    })
                    .collect();
                let find = |l: &LocalName| {
                    attrs
                        .iter()
                        .find(|a| a.name.ns == ns!() && a.name.local == *l)
                        .map(|a| (*a.value).to_owned())
                };
                let id = find(&local_name!("id")).map(|v| Atom::from(v.as_str()));
                let style_attribute = find(&local_name!("style")).map(|v| {
                    Arc::new(document.locks.author.wrap(parse_style_attribute(
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
        let id = document.nodes.len();
        document.nodes.push(Node {
            id,
            doc: doc_ptr,
            parent,
            children: Vec::new(),
            data,
            style_data: UnsafeCell::new(None),
            layout_data: UnsafeCell::new(None),
            selector_flags: Cell::new(ElementSelectorFlags::empty()),
            dirty_descendants: AtomicBool::new(true),
            has_snapshot: AtomicBool::new(false),
            handled_snapshot: AtomicBool::new(false),
        });
        let kids: Vec<RcHandle> = n.children.borrow().clone();
        for k in &kids {
            let c = walk(document, doc_ptr, url, k, Some(id));
            document.nodes[id].children.push(c);
        }
        id
    }
    walk(&mut document, doc_ptr, url, doc, None);
    document
}
