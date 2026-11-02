// SPDX-License-Identifier: MPL-2.0
//! The Stylo-facing wrappers (`script`'s `ServoDangerousStyle*` names) and
//! Stylo's DOM traits on them. Adapted from #4's `stylo-smoke` arena impls;
//! the trait shapes follow `servo-script` 0.7.0's `layout_dom` (MPL-2.0).
//! A static document: no shadow DOM, no animations, no interaction state.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr::NonNull;
use std::sync::atomic::Ordering;

use html5ever::{LocalName, Namespace, local_name, ns};
use layout_api::{DangerousStyleElement, DangerousStyleNode, LayoutDamage};
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::{BLOOM_HASH_MASK, BloomFilter};
use selectors::matching::{ElementSelectorFlags, MatchingContext, VisitedHandlingMode};
use selectors::sink::Push;
use selectors::{Element as SelectorsElement, OpaqueElement};
use servo_arc::{Arc, ArcBorrow};
use style::applicable_declarations::ApplicableDeclarationBlock;
use style::bloom::each_relevant_element_hash;
use style::context::{QuirksMode, SharedStyleContext};
use style::data::{ElementDataMut, ElementDataRef};
use style::dom::{LayoutIterator, NodeInfo, OpaqueNode, TDocument, TElement, TNode, TShadowRoot};
use style::properties::{ComputedValues, PropertyDeclarationBlock};
// The selector-side attribute value (an `AtomString`), not layout's
// `style::attr::AttrValue`.
use style::selector_parser::{
    AttrValue, Lang, NonTSPseudoClass, PseudoElement, RestyleDamage, SelectorImpl,
};
use style::shared_lock::{Locked, SharedRwLock, SharedRwLockReadGuard};
use style::stylist::{CascadeData, Stylist};
use style::values::computed::Display;
use style::values::{AtomIdent, GenericAtomIdent};
use style::{Atom, CaseSensitivityExt};
use style_dom::ElementState;

use crate::dom::{Node, NodeData, SharedRwLocks};
use crate::layout_impl::{NeoLayoutDomTypeBundle, ServoLayoutElement, ServoLayoutNode};

macro_rules! node_wrapper {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, Copy)]
        pub struct $name<'dom>(pub(crate) &'dom Node);

        impl PartialEq for $name<'_> {
            fn eq(&self, other: &Self) -> bool {
                std::ptr::eq(self.0, other.0)
            }
        }

        impl Eq for $name<'_> {}

        impl Hash for $name<'_> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                state.write_usize(self.0.id)
            }
        }

        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.0.element() {
                    Some(e) => write!(f, "<{}#{}>", e.name.local, self.0.id),
                    None => write!(f, "node#{}", self.0.id),
                }
            }
        }
    };
}

node_wrapper!(
    /// Any node, as Stylo sees it.
    ServoDangerousStyleNode
);
node_wrapper!(
    /// An element, as Stylo sees it.
    ServoDangerousStyleElement
);
node_wrapper!(
    /// The document node, as Stylo sees it.
    ServoDangerousStyleDocument
);
node_wrapper!(
    /// Never constructed: a static document has no shadow roots.
    ServoDangerousStyleShadowRoot
);

fn opaque(n: &Node) -> OpaqueNode {
    OpaqueNode(n as *const Node as usize)
}

// --- The document -------------------------------------------------------------

impl<'dom> TDocument for ServoDangerousStyleDocument<'dom> {
    type ConcreteNode = ServoDangerousStyleNode<'dom>;

    fn as_node(&self) -> ServoDangerousStyleNode<'dom> {
        ServoDangerousStyleNode(self.0)
    }

    fn quirks_mode(&self) -> QuirksMode {
        self.0.doc().quirks_mode
    }

    fn is_html_document(&self) -> bool {
        true
    }

    fn shared_lock(&self) -> &SharedRwLock {
        &self.0.doc().locks.author
    }
}

/// The inherent methods layout calls on `script`'s document wrapper, with the
/// same signatures.
impl<'dom> ServoDangerousStyleDocument<'dom> {
    /// The root element.
    pub fn root_element(&self) -> Option<ServoLayoutElement<'dom>> {
        let doc = self.0;
        doc.children
            .iter()
            .map(|&c| doc.with(c))
            .find(|n| n.element().is_some())
            .map(ServoLayoutElement::from_node)
    }

    /// The style locks.
    pub fn shared_style_locks(&self) -> &'dom SharedRwLocks {
        &self.0.doc().locks
    }

    /// A static document has no shadow roots, so there is nothing to flush.
    pub fn flush_shadow_root_stylesheets_if_necessary(
        &self,
        _stylist: &mut Stylist,
        _guard: &SharedRwLockReadGuard,
    ) {
    }
}

// --- Shadow roots (none) -----------------------------------------------------

impl<'dom> TShadowRoot for ServoDangerousStyleShadowRoot<'dom> {
    type ConcreteNode = ServoDangerousStyleNode<'dom>;

    fn as_node(&self) -> ServoDangerousStyleNode<'dom> {
        ServoDangerousStyleNode(self.0)
    }

    fn host(&self) -> ServoDangerousStyleElement<'dom> {
        unreachable!("a static document has no shadow roots")
    }

    fn style_data<'a>(&self) -> Option<&'a CascadeData>
    where
        Self: 'a,
    {
        None
    }
}

// --- Nodes -----------------------------------------------------------------------

impl NodeInfo for ServoDangerousStyleNode<'_> {
    fn is_element(&self) -> bool {
        matches!(self.0.data, NodeData::Element(_))
    }

    fn is_text_node(&self) -> bool {
        matches!(self.0.data, NodeData::Text(_))
    }
}

impl<'dom> TNode for ServoDangerousStyleNode<'dom> {
    type ConcreteElement = ServoDangerousStyleElement<'dom>;
    type ConcreteDocument = ServoDangerousStyleDocument<'dom>;
    type ConcreteShadowRoot = ServoDangerousStyleShadowRoot<'dom>;

    fn parent_node(&self) -> Option<Self> {
        self.0.parent.map(|p| Self(self.0.with(p)))
    }

    fn first_child(&self) -> Option<Self> {
        self.0.children.first().map(|&c| Self(self.0.with(c)))
    }

    fn last_child(&self) -> Option<Self> {
        self.0.children.last().map(|&c| Self(self.0.with(c)))
    }

    fn prev_sibling(&self) -> Option<Self> {
        self.0.sibling(-1).map(Self)
    }

    fn next_sibling(&self) -> Option<Self> {
        self.0.sibling(1).map(Self)
    }

    fn owner_doc(&self) -> ServoDangerousStyleDocument<'dom> {
        ServoDangerousStyleDocument(self.0.with(0))
    }

    fn is_in_document(&self) -> bool {
        true
    }

    fn traversal_parent(&self) -> Option<ServoDangerousStyleElement<'dom>> {
        self.parent_node().and_then(|p| p.as_element())
    }

    fn opaque(&self) -> OpaqueNode {
        opaque(self.0)
    }

    fn debug_id(self) -> usize {
        self.0.id
    }

    fn as_element(&self) -> Option<ServoDangerousStyleElement<'dom>> {
        self.is_element()
            .then_some(ServoDangerousStyleElement(self.0))
    }

    fn as_document(&self) -> Option<ServoDangerousStyleDocument<'dom>> {
        matches!(self.0.data, NodeData::Document).then_some(ServoDangerousStyleDocument(self.0))
    }

    fn as_shadow_root(&self) -> Option<ServoDangerousStyleShadowRoot<'dom>> {
        None
    }
}

impl<'dom> DangerousStyleNode<'dom> for ServoDangerousStyleNode<'dom> {
    type ConcreteTypeBundle = NeoLayoutDomTypeBundle<'dom>;

    fn layout_node(&self) -> ServoLayoutNode<'dom> {
        ServoLayoutNode::from_node(self.0)
    }
}

// --- Elements ------------------------------------------------------------------

impl<'dom> ServoDangerousStyleElement<'dom> {
    fn data(&self) -> &'dom crate::dom::ElementData {
        self.0
            .element()
            .expect("an element wrapper wraps an element")
    }

    fn sibling_element(&self, dir: isize) -> Option<Self> {
        let mut step = dir;
        while let Some(n) = self.0.sibling(step) {
            if n.element().is_some() {
                return Some(Self(n));
            }
            step += dir;
        }
        None
    }
}

impl SelectorsElement for ServoDangerousStyleElement<'_> {
    type Impl = SelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::from_non_null_ptr(NonNull::from(self.0).cast::<()>())
    }

    fn parent_element(&self) -> Option<Self> {
        ServoDangerousStyleNode(self.0).traversal_parent()
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
        self.sibling_element(-1)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.sibling_element(1)
    }

    fn first_element_child(&self) -> Option<Self> {
        self.0
            .children
            .iter()
            .map(|&c| self.0.with(c))
            .find(|c| c.element().is_some())
            .map(Self)
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.data().name.ns == ns!(html)
    }

    fn has_local_name(&self, local_name: &LocalName) -> bool {
        self.data().name.local == *local_name
    }

    fn has_namespace(&self, ns: &Namespace) -> bool {
        self.data().name.ns == *ns
    }

    fn is_same_type(&self, other: &Self) -> bool {
        let (a, b) = (self.data(), other.data());
        a.name.local == b.name.local && a.name.ns == b.name.ns
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&GenericAtomIdent<html5ever::NamespaceStaticSet>>,
        local_name: &GenericAtomIdent<html5ever::LocalNameStaticSet>,
        operation: &AttrSelectorOperation<&AttrValue>,
    ) -> bool {
        self.data().attrs.iter().any(|a| {
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
            self.0.insert_selector_flags(own);
        }
        let parent = flags.for_parent();
        if !parent.is_empty()
            && let Some(p) = self.0.parent
        {
            let p = self.0.with(p);
            p.insert_selector_flags(parent);
        }
    }

    fn is_link(&self) -> bool {
        self.has_local_name(&local_name!("a")) && self.0.attr(&local_name!("href")).is_some()
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &AtomIdent, case_sensitivity: CaseSensitivity) -> bool {
        self.data()
            .id
            .as_ref()
            .is_some_and(|own| case_sensitivity.eq_atom(own, id))
    }

    fn has_class(&self, name: &AtomIdent, case_sensitivity: CaseSensitivity) -> bool {
        self.0.attr(&local_name!("class")).is_some_and(|v| {
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
        self.0.children.iter().all(|&c| match &self.0.with(c).data {
            NodeData::Element(_) => false,
            NodeData::Text(t) => t.borrow().is_empty(),
            _ => true,
        })
    }

    fn is_root(&self) -> bool {
        self.0
            .parent
            .is_some_and(|p| matches!(self.0.with(p).data, NodeData::Document))
    }

    fn add_element_unique_hashes(&self, filter: &mut BloomFilter) -> bool {
        each_relevant_element_hash(*self, |hash| filter.insert_hash(hash & BLOOM_HASH_MASK));
        true
    }
}

/// The children Stylo walks: every child, in order.
pub struct Children<'dom> {
    parent: &'dom Node,
    at: usize,
}

impl<'dom> Iterator for Children<'dom> {
    type Item = ServoDangerousStyleNode<'dom>;

    fn next(&mut self) -> Option<Self::Item> {
        let id = *self.parent.children.get(self.at)?;
        self.at += 1;
        Some(ServoDangerousStyleNode(self.parent.with(id)))
    }
}

impl<'dom> TElement for ServoDangerousStyleElement<'dom> {
    type ConcreteNode = ServoDangerousStyleNode<'dom>;
    type TraversalChildrenIterator = Children<'dom>;

    fn as_node(&self) -> ServoDangerousStyleNode<'dom> {
        ServoDangerousStyleNode(self.0)
    }

    fn traversal_children(&self) -> LayoutIterator<Children<'dom>> {
        LayoutIterator(Children {
            parent: self.0,
            at: 0,
        })
    }

    fn is_html_element(&self) -> bool {
        self.data().name.ns == ns!(html)
    }

    fn is_mathml_element(&self) -> bool {
        self.data().name.ns == ns!(mathml)
    }

    fn is_svg_element(&self) -> bool {
        self.data().name.ns == ns!(svg)
    }

    fn style_attribute(&self) -> Option<ArcBorrow<'_, Locked<PropertyDeclarationBlock>>> {
        self.data().style_attribute.as_ref().map(|a| a.borrow_arc())
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
        self.data().id.as_ref()
    }

    fn each_class<F>(&self, mut callback: F)
    where
        F: FnMut(&AtomIdent),
    {
        if let Some(v) = self.0.attr(&local_name!("class")) {
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
        for a in &self.data().attrs {
            callback(&GenericAtomIdent(a.name.local.clone()));
        }
    }

    fn has_dirty_descendants(&self) -> bool {
        self.0.dirty_descendants.load(Ordering::Relaxed)
    }

    fn has_snapshot(&self) -> bool {
        self.0.has_snapshot.load(Ordering::Relaxed)
    }

    fn handled_snapshot(&self) -> bool {
        self.0.handled_snapshot.load(Ordering::Relaxed)
    }

    unsafe fn set_handled_snapshot(&self) {
        self.0.handled_snapshot.store(true, Ordering::Relaxed);
    }

    unsafe fn set_dirty_descendants(&self) {
        self.0.dirty_descendants.store(true, Ordering::Relaxed);
    }

    unsafe fn unset_dirty_descendants(&self) {
        self.0.dirty_descendants.store(false, Ordering::Relaxed);
    }

    fn store_children_to_process(&self, n: isize) {
        let data = self.0.style_data().expect("styled before its children");
        data.parallel
            .children_to_process
            .store(n, Ordering::Relaxed);
    }

    fn did_process_child(&self) -> isize {
        let data = self.0.style_data().expect("styled before its children");
        data.parallel
            .children_to_process
            .fetch_sub(1, Ordering::Relaxed)
            - 1
    }

    unsafe fn ensure_data(&self) -> ElementDataMut<'_> {
        // SAFETY: called by Stylo's traversal, which has the node to itself.
        unsafe { self.0.ensure_style_data() }
            .element_data
            .borrow_mut()
    }

    unsafe fn clear_data(&self) {
        // SAFETY: as `ensure_data`.
        unsafe { self.0.clear_style_data() }
    }

    fn has_data(&self) -> bool {
        self.0.style_data().is_some()
    }

    fn borrow_data(&self) -> Option<ElementDataRef<'_>> {
        self.0.style_data().map(|d| d.element_data.borrow())
    }

    fn mutate_data(&self) -> Option<ElementDataMut<'_>> {
        self.0.style_data().map(|d| d.element_data.borrow_mut())
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

    fn shadow_root(&self) -> Option<ServoDangerousStyleShadowRoot<'dom>> {
        None
    }

    fn containing_shadow(&self) -> Option<ServoDangerousStyleShadowRoot<'dom>> {
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
            && self
                .parent_element()
                .is_some_and(|p| SelectorsElement::is_root(&p))
    }

    fn synthesize_presentational_hints_for_legacy_attributes<V>(
        &self,
        _visited_handling: VisitedHandlingMode,
        _hints: &mut V,
    ) where
        V: Push<ApplicableDeclarationBlock>,
    {
        // Legacy presentational attributes (`bgcolor`, `width`, ...) are not
        // mapped yet: a later rung, with its own oracle.
    }

    fn local_name(&self) -> &html5ever::LocalName {
        &self.data().name.local
    }

    fn namespace(&self) -> &html5ever::Namespace {
        &self.data().name.ns
    }

    fn query_container_size(
        &self,
        _display: &Display,
    ) -> euclid::default::Size2D<Option<app_units::Au>> {
        Default::default()
    }

    fn has_selector_flags(&self, flags: ElementSelectorFlags) -> bool {
        self.0.selector_flags().contains(flags)
    }

    fn relative_selector_search_direction(&self) -> ElementSelectorFlags {
        self.0
            .selector_flags()
            .intersection(ElementSelectorFlags::RELATIVE_SELECTOR_SEARCH_DIRECTION_ANCESTOR_SIBLING)
    }

    fn get_attr(&self, attr: &style::LocalName, namespace: &style::Namespace) -> Option<String> {
        self.0
            .attr_value(&namespace.0, &attr.0)
            .map(|v| (**v).to_owned())
    }

    /// A static page is styled once, so every restyle of an element that was
    /// already styled rebuilds its boxes: the conservative answer. (`script`
    /// narrows this per property; a later rung can port that when restyles
    /// happen.)
    fn compute_layout_damage(_old: &ComputedValues, _new: &ComputedValues) -> RestyleDamage {
        RestyleDamage::from_bits_retain(LayoutDamage::BoxDamage.bits())
    }
}

impl<'dom> DangerousStyleElement<'dom> for ServoDangerousStyleElement<'dom> {
    type ConcreteTypeBundle = NeoLayoutDomTypeBundle<'dom>;

    fn layout_element(&self) -> ServoLayoutElement<'dom> {
        ServoLayoutElement::from_node(self.0)
    }
}
