// SPDX-License-Identifier: MPL-2.0
//! The layout-facing wrappers (`script`'s `ServoLayoutNode` and
//! `ServoLayoutElement` names) and `servo-layout-api`'s `LayoutNode` and
//! `LayoutElement` on them. Ported from `servo-script` 0.7.0's
//! `layout_dom/servo_layout_{node,element}.rs` (MPL-2.0, Copyright The Servo
//! Project Developers): the same logic, over the static arena instead of
//! script's GC'd DOM. What a static page can't have (images, canvas, media,
//! iframes, form widgets, selections, a caret) answers "none".

use std::marker::PhantomData;
use std::sync::atomic::Ordering;

use atomic_refcell::{AtomicRef, AtomicRefCell};
use html5ever::{LocalName, Namespace, local_name, ns};
use layout_api::{
    GenericLayoutData, HTMLCanvasData, HTMLMediaData, LayoutDataTrait, LayoutDomTypeBundle,
    LayoutElement, LayoutElementType, LayoutNode, LayoutNodeType, PseudoElementChain,
    SVGElementData, StyleData, TrustedNodeAddress,
};
use net_traits::image_cache::Image;
use pixels::ImageMetadata;
use servo_arc::Arc;
use servo_base::id::{BrowsingContextId, PipelineId};
use servo_base::text::{RangeAny, Utf32CodeUnits};
use servo_url::ServoUrl;
use style::attr::AttrValue;
use style::context::SharedStyleContext;
use style::data::{ElementDataMut, ElementDataRef};
use style::dom::{LayoutIterator, NodeInfo, OpaqueNode, TNode};
use style::properties::ComputedValues;
use style::selector_parser::{PseudoElement, PseudoElementCascadeType};
use style::stylist::RuleInclusion;

use crate::dom::{Node, NodeData};
use crate::style_impl::{ServoDangerousStyleElement, ServoDangerousStyleNode};

/// The bundle of this crate's four concrete layout DOM types.
pub struct NeoLayoutDomTypeBundle<'dom> {
    phantom: PhantomData<&'dom ()>,
}

impl<'dom> LayoutDomTypeBundle<'dom> for NeoLayoutDomTypeBundle<'dom> {
    type ConcreteLayoutElement = ServoLayoutElement<'dom>;
    type ConcreteLayoutNode = ServoLayoutNode<'dom>;
    type ConcreteDangerousStyleNode = ServoDangerousStyleNode<'dom>;
    type ConcreteDangerousStyleElement = ServoDangerousStyleElement<'dom>;
}

/// A node as layout sees it.
#[derive(Clone, Copy)]
pub struct ServoLayoutNode<'dom> {
    pub(crate) node: &'dom Node,
    pub(crate) pseudo_element_chain: PseudoElementChain,
}

/// An element as layout sees it.
#[derive(Clone, Copy)]
pub struct ServoLayoutElement<'dom> {
    pub(crate) node: &'dom Node,
    pub(crate) pseudo_element_chain: PseudoElementChain,
}

macro_rules! layout_wrapper_traits {
    ($name:ident) => {
        impl PartialEq for $name<'_> {
            fn eq(&self, other: &Self) -> bool {
                std::ptr::eq(self.node, other.node)
                    && self.pseudo_element_chain == other.pseudo_element_chain
            }
        }

        impl Eq for $name<'_> {}

        impl std::hash::Hash for $name<'_> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                state.write_usize(self.node.id);
                self.pseudo_element_chain.hash(state);
            }
        }

        impl std::fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.node.element() {
                    Some(e) => write!(f, "<{}#{}>", e.name.local, self.node.id)?,
                    None => write!(f, "node#{}", self.node.id)?,
                }
                if !self.pseudo_element_chain.is_empty() {
                    write!(f, "{:?}", self.pseudo_element_chain)?;
                }
                Ok(())
            }
        }
    };
}

layout_wrapper_traits!(ServoLayoutNode);
layout_wrapper_traits!(ServoLayoutElement);

impl<'dom> ServoLayoutNode<'dom> {
    pub(crate) fn from_node(node: &'dom Node) -> Self {
        Self {
            node,
            pseudo_element_chain: Default::default(),
        }
    }

    /// The node at `address`, which the host took from `Document::address`.
    ///
    /// # Safety
    ///
    /// `address` must point at a node of a `Document` that outlives `'dom`
    /// (`script`'s contract for the same constructor).
    pub unsafe fn new(address: &TrustedNodeAddress) -> Self {
        Self::from_node(unsafe { &*(address.0 as *const Node) })
    }

    fn child(&self, at: usize) -> Option<Self> {
        self.node
            .children
            .get(at)
            .map(|&c| Self::from_node(self.node.with(c)))
    }
}

impl<'dom> ServoLayoutElement<'dom> {
    pub(crate) fn from_node(node: &'dom Node) -> Self {
        Self {
            node,
            pseudo_element_chain: Default::default(),
        }
    }

    fn is_html_element(&self) -> bool {
        self.node.element().is_some_and(|e| e.name.ns == ns!(html))
    }
}

/// The children of a node, in DOM order (no shadow trees, no slots).
pub struct ChildrenIter<'dom> {
    parent: ServoLayoutNode<'dom>,
    at: usize,
}

impl<'dom> Iterator for ChildrenIter<'dom> {
    type Item = ServoLayoutNode<'dom>;

    fn next(&mut self) -> Option<Self::Item> {
        let child = self.parent.child(self.at)?;
        self.at += 1;
        Some(child)
    }
}

fn element_type(name: &html5ever::QualName) -> LayoutElementType {
    use LayoutElementType as T;
    if name.ns == ns!(svg) {
        return match name.local {
            local_name!("svg") => T::SVGSVGElement,
            local_name!("image") => T::SVGImageElement,
            _ => T::Element,
        };
    }
    if name.ns != ns!(html) {
        return T::Element;
    }
    match name.local {
        local_name!("body") => T::HTMLBodyElement,
        local_name!("button") => T::HTMLButtonElement,
        local_name!("br") => T::HTMLBRElement,
        local_name!("canvas") => T::HTMLCanvasElement,
        local_name!("html") => T::HTMLHtmlElement,
        local_name!("iframe") => T::HTMLIFrameElement,
        local_name!("img") => T::HTMLImageElement,
        local_name!("input") => T::HTMLInputElement,
        local_name!("audio") | local_name!("video") => T::HTMLMediaElement,
        local_name!("object") => T::HTMLObjectElement,
        local_name!("optgroup") => T::HTMLOptGroupElement,
        local_name!("option") => T::HTMLOptionElement,
        local_name!("p") => T::HTMLParagraphElement,
        local_name!("pre") | local_name!("listing") | local_name!("xmp") => T::HTMLPreElement,
        local_name!("select") => T::HTMLSelectElement,
        local_name!("td") | local_name!("th") => T::HTMLTableCellElement,
        local_name!("col") | local_name!("colgroup") => T::HTMLTableColElement,
        local_name!("table") => T::HTMLTableElement,
        local_name!("tr") => T::HTMLTableRowElement,
        local_name!("thead") | local_name!("tbody") | local_name!("tfoot") => {
            T::HTMLTableSectionElement
        }
        local_name!("textarea") => T::HTMLTextAreaElement,
        _ => T::Element,
    }
}

fn span_attr(node: &Node, name: &LocalName) -> Option<u32> {
    node.attr(name)?.trim().parse().ok()
}

impl NodeInfo for ServoLayoutNode<'_> {
    fn is_element(&self) -> bool {
        matches!(self.node.data, NodeData::Element(_))
    }

    fn is_text_node(&self) -> bool {
        matches!(self.node.data, NodeData::Text(_))
    }
}

/// An empty string for `text_content` on a node that has no text.
static NO_TEXT: AtomicRefCell<String> = AtomicRefCell::new(String::new());

impl<'dom> LayoutNode<'dom> for ServoLayoutNode<'dom> {
    type ConcreteTypeBundle = NeoLayoutDomTypeBundle<'dom>;

    fn with_pseudo(&self, pseudo_element_type: PseudoElement) -> Option<Self> {
        Some(
            self.as_element()?
                .with_pseudo(pseudo_element_type)?
                .as_node(),
        )
    }

    fn pseudo_element_chain(&self) -> PseudoElementChain {
        self.pseudo_element_chain
    }

    unsafe fn dangerous_style_node(self) -> ServoDangerousStyleNode<'dom> {
        ServoDangerousStyleNode(self.node)
    }

    unsafe fn dangerous_dom_parent(self) -> Option<Self> {
        self.node.parent.map(|p| Self::from_node(self.node.with(p)))
    }

    unsafe fn dangerous_flat_tree_parent(self) -> Option<Self> {
        ServoDangerousStyleNode(self.node)
            .traversal_parent()
            .map(|e| Self::from_node(e.0))
    }

    fn layout_data(&self) -> Option<&'dom GenericLayoutData> {
        self.node.layout_data()
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn opaque(&self) -> OpaqueNode {
        OpaqueNode(self.node as *const Node as usize)
    }

    fn type_id(&self) -> Option<LayoutNodeType> {
        if !self.pseudo_element_chain.is_empty() {
            return None;
        }
        match &self.node.data {
            NodeData::Element(e) => Some(LayoutNodeType::Element(element_type(&e.name))),
            NodeData::Text(_) => Some(LayoutNodeType::Text),
            // `script` panics here for other node kinds (doctype, comment):
            // layout never asks, since it walks elements and text only.
            NodeData::Document | NodeData::Other => None,
        }
    }

    fn initialize_layout_data<RequestedLayoutDataType: LayoutDataTrait>(&self) {
        if self.node.layout_data().is_none() {
            // SAFETY: layout initialises a node's data before any other access
            // to it, from the one thread building that node's boxes.
            unsafe {
                self.node
                    .set_layout_data(Box::<RequestedLayoutDataType>::default())
            };
        }
    }

    fn flat_tree_children(&self) -> impl Iterator<Item = Self> + Sized {
        LayoutIterator(ChildrenIter {
            parent: Self::from_node(self.node),
            at: 0,
        })
    }

    fn dom_children(&self) -> impl Iterator<Item = Self> + Sized {
        LayoutIterator(ChildrenIter {
            parent: Self::from_node(self.node),
            at: 0,
        })
    }

    fn as_html_element(&self) -> Option<ServoLayoutElement<'dom>> {
        self.as_element().filter(|e| e.is_html_element())
    }

    fn as_element(&self) -> Option<ServoLayoutElement<'dom>> {
        self.node.element().map(|_| ServoLayoutElement {
            node: self.node,
            pseudo_element_chain: self.pseudo_element_chain,
        })
    }

    fn style(&self, context: &SharedStyleContext) -> Arc<ComputedValues> {
        if let Some(element) = self.as_element() {
            element.style(context)
        } else {
            debug_assert!(self.is_text_node());
            self.parent_style(context)
        }
    }

    fn parent_style(&self, context: &SharedStyleContext) -> Arc<ComputedValues> {
        if let Some(chain) = self.pseudo_element_chain.without_innermost() {
            let mut parent = *self;
            parent.pseudo_element_chain = chain;
            return parent.style(context);
        }
        unsafe { self.dangerous_flat_tree_parent() }
            .expect("a styled non-element has a parent element")
            .style(context)
    }

    fn selected_style(&self, context: &SharedStyleContext) -> Arc<ComputedValues> {
        let Some(element) = self.as_element() else {
            return unsafe { self.dangerous_flat_tree_parent() }
                .expect("a non-element has a parent")
                .selected_style(context);
        };
        let selected = element
            .element_data()
            .styles
            .pseudos
            .get(&PseudoElement::Selection)
            .cloned();
        selected.unwrap_or_else(|| self.style(context))
    }

    fn text_content(self) -> AtomicRef<'dom, str> {
        let cell: &'dom AtomicRefCell<String> = match &self.node.data {
            NodeData::Text(t) => t,
            _ => &NO_TEXT,
        };
        AtomicRef::map(cell.borrow(), |s| s.as_str())
    }

    fn text_node_selection(&self) -> Option<RangeAny<Utf32CodeUnits>> {
        None
    }

    fn text_node_paints_caret(&self) -> bool {
        false
    }

    fn image_url(&self) -> Option<ServoUrl> {
        None
    }

    fn image_density(&self) -> Option<f64> {
        None
    }

    fn image_data(&self) -> Option<(Option<Image>, Option<ImageMetadata>)> {
        None
    }

    fn replaced_is_selected(&self) -> bool {
        false
    }

    fn showing_broken_image_icon(&self) -> bool {
        false
    }

    fn canvas_data(&self) -> Option<HTMLCanvasData> {
        None
    }

    fn svg_data(&self) -> Option<SVGElementData<'dom>> {
        None
    }

    fn media_data(&self) -> Option<HTMLMediaData> {
        None
    }

    fn iframe_browsing_context_id(&self) -> Option<BrowsingContextId> {
        None
    }

    fn iframe_pipeline_id(&self) -> Option<PipelineId> {
        None
    }

    fn table_span(&self) -> Option<u32> {
        span_attr(self.node, &local_name!("span"))
    }

    fn table_colspan(&self) -> Option<u32> {
        span_attr(self.node, &local_name!("colspan"))
    }

    fn table_rowspan(&self) -> Option<u32> {
        span_attr(self.node, &local_name!("rowspan"))
    }

    fn is_single_line_text_input(&self) -> bool {
        false
    }

    fn is_root_of_user_agent_widget(&self) -> bool {
        false
    }

    fn set_uses_content_attribute_with_attr(&self, _uses_content_attribute_with_attr: bool) {
        // Attributes never change on a static page, so there is nothing to
        // re-run `content: attr()` for.
    }
}

impl<'dom> LayoutElement<'dom> for ServoLayoutElement<'dom> {
    type ConcreteTypeBundle = NeoLayoutDomTypeBundle<'dom>;

    fn with_pseudo(&self, pseudo_element: PseudoElement) -> Option<Self> {
        if pseudo_element.is_eager()
            && self
                .element_data()
                .styles
                .pseudos
                .get(&pseudo_element)
                .is_none()
        {
            return None;
        }

        if pseudo_element == PseudoElement::DetailsContent
            && (self.local_name() != &local_name!("details")
                || !self.is_html_element()
                || self.attribute(&ns!(), &local_name!("open")).is_none())
        {
            return None;
        }

        if !self.pseudo_element_chain.is_empty() {
            assert!(!pseudo_element.is_eager());
            assert!(pseudo_element != PseudoElement::DetailsContent);
        }

        Some(Self {
            node: self.node,
            pseudo_element_chain: self.pseudo_element_chain.with_pseudo(pseudo_element),
        })
    }

    fn pseudo_element_chain(&self) -> PseudoElementChain {
        self.pseudo_element_chain
    }

    fn as_node(&self) -> ServoLayoutNode<'dom> {
        ServoLayoutNode {
            node: self.node,
            pseudo_element_chain: self.pseudo_element_chain,
        }
    }

    unsafe fn dangerous_style_element(self) -> ServoDangerousStyleElement<'dom> {
        ServoDangerousStyleElement(self.node)
    }

    fn initialize_style_and_layout_data<RequestedLayoutDataType: LayoutDataTrait>(&self) {
        // SAFETY: as `LayoutNode::initialize_layout_data`; Stylo has not
        // started on this element yet.
        unsafe {
            self.node.ensure_style_data();
            if self.node.layout_data().is_none() {
                self.node
                    .set_layout_data(Box::<RequestedLayoutDataType>::default());
            }
        }
    }

    fn unset_snapshot_flags(&self) {
        self.node.has_snapshot.store(false, Ordering::Relaxed);
        self.node.handled_snapshot.store(false, Ordering::Relaxed);
    }

    fn set_has_snapshot(&self) {
        self.node.has_snapshot.store(true, Ordering::Relaxed);
    }

    fn style_data(self) -> Option<&'dom StyleData> {
        self.node.style_data()
    }

    fn type_id(&self) -> Option<LayoutNodeType> {
        self.as_node().type_id()
    }

    fn local_name(&self) -> &LocalName {
        &self.node.element().expect("an element").name.local
    }

    fn attribute(&self, namespace: &Namespace, name: &LocalName) -> Option<&AttrValue> {
        self.node.attr_value(namespace, name)
    }

    fn attribute_as_str<'a>(&'a self, namespace: &Namespace, name: &LocalName) -> Option<&'a str> {
        self.node.attr_value(namespace, name).map(|v| &**v)
    }

    fn element_data(&self) -> ElementDataRef<'dom> {
        self.style_data()
            .expect("Unstyled layout node?")
            .element_data
            .borrow()
    }

    fn element_data_mut(&self) -> ElementDataMut<'dom> {
        self.style_data()
            .expect("Unstyled layout node?")
            .element_data
            .borrow_mut()
    }

    fn style(&self, context: &SharedStyleContext) -> Arc<ComputedValues> {
        let get_style_for_pseudo_element =
            |data: &ElementDataRef<'_>,
             base_style: &Arc<ComputedValues>,
             pseudo_element: PseudoElement| {
                match pseudo_element.cascade_type() {
                    PseudoElementCascadeType::Eager => {
                        data.styles.pseudos.get(&pseudo_element).unwrap().clone()
                    }
                    PseudoElementCascadeType::Precomputed => context
                        .stylist
                        .precomputed_values_for_pseudo::<ServoDangerousStyleElement<'dom>>(
                        &context.guards,
                        &pseudo_element,
                        Some(base_style),
                    ),
                    PseudoElementCascadeType::Lazy => context
                        .stylist
                        .lazily_compute_pseudo_element_style(
                            &context.guards,
                            unsafe { self.dangerous_style_element() },
                            &pseudo_element,
                            RuleInclusion::All,
                            base_style,
                            /* is_probe = */ false,
                            /* matching_func = */ None,
                        )
                        .unwrap(),
                }
            };

        let data = self.element_data();
        let element_style = data.styles.primary();
        let chain = self.pseudo_element_chain();

        let primary_pseudo_style = match chain.primary {
            Some(pseudo_element) => {
                get_style_for_pseudo_element(&data, element_style, pseudo_element)
            }
            None => return element_style.clone(),
        };
        match chain.secondary {
            Some(pseudo_element) => {
                get_style_for_pseudo_element(&data, &primary_pseudo_style, pseudo_element)
            }
            None => primary_pseudo_style,
        }
    }

    fn is_shadow_host(&self) -> bool {
        false
    }

    fn is_body_element_of_html_element_root(&self) -> bool {
        self.is_html_element()
            && self.local_name() == &local_name!("body")
            && self.node.parent.is_some_and(|p| {
                let parent = self.node.with(p);
                parent
                    .element()
                    .is_some_and(|e| e.name.ns == ns!(html) && e.name.local == local_name!("html"))
                    && parent
                        .parent
                        .is_some_and(|g| matches!(self.node.with(g).data, NodeData::Document))
            })
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.is_html_element()
    }

    fn is_root(&self) -> bool {
        self.node
            .parent
            .is_some_and(|p| matches!(self.node.with(p).data, NodeData::Document))
    }
}
