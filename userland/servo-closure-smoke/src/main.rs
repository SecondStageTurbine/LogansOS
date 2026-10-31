// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.3b — `servo-closure-smoke`: upstream html5ever and Stylo's
//! `selectors` on-target (ADR-078 §3, the C2 gate's last item;
//! `docs/RESEARCH-2026-10-29-ot61-c2.3b-html5ever-stylo-closure-SPIKE.md`).
//!
//! Every third-party crate here is the vendored closure in
//! `third_party/ot61-browser/`. The binary checks, in order:
//!
//! 1. **Parse and match.** html5ever builds a tree for a fixed page through a
//!    minimal `TreeSink` (the job Servo's DOM does in the real engine), and
//!    `selectors` matches a list of CSS selectors against it. Every expected
//!    count is hand-derived from the HTML5 tree-construction rules (implied
//!    `</p>` and `</li>`, the `<tbody>` the parser inserts, entity decoding),
//!    not from running the parser, so a wrong tree fails.
//! 2. **The parker.** `string_cache`'s atom table sits behind
//!    `parking_lot::Mutex` buckets. Upstream `parking_lot_core` has no
//!    LogansOS arm and would spin in `park()`; the vendored copy parks on
//!    std's futex-based `Mutex`/`Condvar`. More threads than CPUs contend on
//!    one `parking_lot::Mutex` whose holder sleeps, then parse in parallel
//!    (interning per-thread atoms), and a `parking_lot::Condvar` times out
//!    and wakes. A spinning parker starves the holder on an AP with no timer
//!    tick; this one blocks.
//! 3. **memchr's SIMD choice.** memchr picks AVX2 at run time when std
//!    reports it. That is safe only if the kernel saves the YMM state across
//!    switches, and the kernel advertises AVX (CR4.OSXSAVE) only when it does.
//!    Threads search alongside each other and yield between searches, and
//!    every result is compared with a byte loop.
//!
//! std is built for this crate as a known target (xtask sets
//! `RUSTC_BOOTSTRAP_SYNTHETIC_TARGET`), so neither this crate nor the vendored
//! ones need `#![feature(restricted_std)]`.
//!
//! Exit 0 when every check passes on the SSE2 path, 100 when they pass with
//! AVX2 detected (the scenario says which it expects, so the AVX2 twin can't
//! pass vacuously). A failed check prints `FAIL(n)` and exits `n` (1..99).

#![no_main]

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use cssparser::ToCss;
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, LocalName, Namespace, QualName, ns, parse_document};
use neo_std_pal::SyscallHeap;
use precomputed_hash::PrecomputedHash;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::context::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags,
    QuirksMode as SelQuirks, SelectorCaches,
};
use selectors::matching::{ElementSelectorFlags, matches_selector_list};
use selectors::parser::{ParseRelative, SelectorParseErrorKind};
use selectors::{OpaqueElement, SelectorList};

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ servo-closure ]";
const PASS_SSE2: i32 = 0;
const PASS_AVX2: i32 = 100;

fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok: {what}");
    } else {
        println!("{TAG} FAIL({code}): {what}");
        std::process::exit(code);
    }
}

// --- The page and its hand-derived expectations -----------------------------

/// Real-world HTML, written the way pages are: implied end tags, a table
/// without `<tbody>`, entities, a comment, a doctype.
const PAGE: &str = "<!DOCTYPE html>\n\
<html lang=\"en\"><head><title>LogansOS &mdash; C2.3b</title>\n\
<style>p.note { color: red }</style></head>\n\
<body>\n\
<div id=\"main\" class=\"content wide\">\n\
  <h1>Hello &amp; welcome</h1>\n\
  <p class=\"note\">First <b>bold</b> note\n\
  <p>Second paragraph<br>with a break\n\
  <p class=\"note last\">Third, never closed\n\
  <ul><li>one<li>two<li data-x=\"y\">three</ul>\n\
  <table><tr><td>cell</table>\n\
  <a href=\"/x\">link</a>\n\
</div>\n\
<!-- a comment -->\n\
</body></html>\n";

/// `(selector, expected match count)`. Each count comes from the HTML5
/// tree-construction algorithm, by hand:
/// - a `<p>` start tag closes an open `<p>`, and `<ul>`/`<table>` close one
///   too, so there are three sibling `p`s directly under `#main`;
/// - `<li>` closes the open `<li>`: three `li` children of the `ul`;
/// - a `<tr>` directly in `<table>` gets an implied `<tbody>`;
/// - `<b>` stays inside the first `p`, `<br>` inside the second.
const EXPECT: &[(&str, usize)] = &[
    ("p", 3),
    ("#main > p", 3),
    ("#main > p.note", 2),
    ("p.note.last", 1),
    ("p:not(.note)", 1),
    ("p > b", 1),
    ("p > br", 1),
    ("ul > li", 3),
    ("li:last-child[data-x=\"y\"]", 1),
    ("li:nth-child(2)", 1),
    ("table > tbody > tr > td", 1),
    ("table > tr", 0),
    ("div.content.wide a[href^=\"/\"]", 1),
    ("head > title", 1),
    ("html:root", 1),
    ("[lang|=en]", 1),
    ("h1 + p", 1),
    ("h1 ~ p", 3),
    ("body > div:only-child", 1),
    ("*", 21),
];

/// Text the parser must decode (entities, implied end tags).
const H1_TEXT: &str = "Hello & welcome";
const TITLE_TEXT: &str = "LogansOS \u{2014} C2.3b";

// --- A minimal DOM and TreeSink ---------------------------------------------

#[derive(Debug)]
enum NodeData {
    Document,
    Doctype,
    Text(RefCell<StrTendril>),
    Comment,
    Pi,
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,
        template_contents: Option<Handle>,
    },
}

#[derive(Debug)]
struct Node {
    data: NodeData,
    parent: RefCell<Option<Weak<Node>>>,
    children: RefCell<Vec<Handle>>,
}

type Handle = Rc<Node>;

impl Node {
    fn new(data: NodeData) -> Handle {
        Rc::new(Node {
            data,
            parent: RefCell::new(None),
            children: RefCell::new(Vec::new()),
        })
    }

    fn parent(&self) -> Option<Handle> {
        self.parent.borrow().as_ref().and_then(Weak::upgrade)
    }

    fn is_element(&self) -> bool {
        matches!(self.data, NodeData::Element { .. })
    }

    fn name(&self) -> &QualName {
        match &self.data {
            NodeData::Element { name, .. } => name,
            _ => panic!("not an element"),
        }
    }

    fn attr(&self, local: &str) -> Option<String> {
        match &self.data {
            NodeData::Element { attrs, .. } => attrs
                .borrow()
                .iter()
                .find(|a| a.name.ns == ns!() && &*a.name.local == local)
                .map(|a| a.value.to_string()),
            _ => None,
        }
    }

    fn text_content(&self, out: &mut String) {
        if let NodeData::Text(t) = &self.data {
            out.push_str(&t.borrow());
        }
        for c in self.children.borrow().iter() {
            c.text_content(out);
        }
    }
}

fn append(parent: &Handle, child: Handle) {
    *child.parent.borrow_mut() = Some(Rc::downgrade(parent));
    parent.children.borrow_mut().push(child);
}

fn detach(target: &Handle) {
    if let Some(parent) = target.parent() {
        parent
            .children
            .borrow_mut()
            .retain(|c| !Rc::ptr_eq(c, target));
    }
    *target.parent.borrow_mut() = None;
}

/// Append text, merging with a preceding text node as the spec requires.
fn append_text(parent: &Handle, text: StrTendril) {
    if let Some(last) = parent.children.borrow().last()
        && let NodeData::Text(t) = &last.data
    {
        t.borrow_mut().push_tendril(&text);
        return;
    }
    append(parent, Node::new(NodeData::Text(RefCell::new(text))));
}

struct Sink {
    document: Handle,
    errors: Cell<usize>,
}

impl TreeSink for Sink {
    type Handle = Handle;
    type Output = (Handle, usize);
    type ElemName<'a>
        = &'a QualName
    where
        Self: 'a;

    fn finish(self) -> Self::Output {
        (self.document, self.errors.get())
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {
        self.errors.set(self.errors.get() + 1);
    }

    fn get_document(&self) -> Handle {
        self.document.clone()
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> &'a QualName {
        target.name()
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        let template_contents = flags.template.then(|| Node::new(NodeData::Document));
        Node::new(NodeData::Element {
            name,
            attrs: RefCell::new(attrs),
            template_contents,
        })
    }

    fn create_comment(&self, _text: StrTendril) -> Handle {
        Node::new(NodeData::Comment)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        Node::new(NodeData::Pi)
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        match child {
            NodeOrText::AppendNode(node) => append(parent, node),
            NodeOrText::AppendText(text) => append_text(parent, text),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if element.parent().is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public: StrTendril,
        _system: StrTendril,
    ) {
        append(&self.document, Node::new(NodeData::Doctype));
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        match &target.data {
            NodeData::Element {
                template_contents: Some(c),
                ..
            } => c.clone(),
            _ => panic!("not a template"),
        }
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        let parent = sibling.parent().expect("sibling has a parent");
        let idx = parent
            .children
            .borrow()
            .iter()
            .position(|c| Rc::ptr_eq(c, sibling))
            .expect("sibling is a child of its parent");
        let node = match new_node {
            NodeOrText::AppendNode(node) => {
                detach(&node);
                node
            }
            NodeOrText::AppendText(text) => {
                if idx > 0
                    && let NodeData::Text(t) = &parent.children.borrow()[idx - 1].data
                {
                    t.borrow_mut().push_tendril(&text);
                    return;
                }
                Node::new(NodeData::Text(RefCell::new(text)))
            }
        };
        *node.parent.borrow_mut() = Some(Rc::downgrade(&parent));
        parent.children.borrow_mut().insert(idx, node);
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        if let NodeData::Element {
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

    fn remove_from_parent(&self, target: &Handle) {
        detach(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let children = std::mem::take(&mut *node.children.borrow_mut());
        for c in children {
            append(new_parent, c);
        }
    }
}

fn parse(html: &str) -> (Handle, usize) {
    let sink = Sink {
        document: Node::new(NodeData::Document),
        errors: Cell::new(0),
    };
    parse_document(sink, Default::default()).one(StrTendril::from(html))
}

fn elements(node: &Handle, out: &mut Vec<Handle>) {
    if node.is_element() {
        out.push(node.clone());
    }
    for c in node.children.borrow().iter() {
        elements(c, out);
    }
}

// --- The selectors embedding ------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct Impl;

/// An identifier or local name, interned through `string_cache` (so matching
/// shares the atom table html5ever fills).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct Ident(LocalName);

impl<'a> From<&'a str> for Ident {
    fn from(s: &'a str) -> Self {
        Ident(LocalName::from(s))
    }
}

impl ToCss for Ident {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        cssparser::serialize_identifier(&self.0, dest)
    }
}

impl PrecomputedHash for Ident {
    fn precomputed_hash(&self) -> u32 {
        self.0.precomputed_hash()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Value(String);

impl<'a> From<&'a str> for Value {
    fn from(s: &'a str) -> Self {
        Value(s.to_owned())
    }
}

impl AsRef<str> for Value {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToCss for Value {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        cssparser::serialize_string(&self.0, dest)
    }
}

/// No pseudo-classes beyond the tree-structural ones `selectors` handles
/// itself (`:not`, `:nth-child`, `:root`, ...): an uninhabited type.
#[derive(Clone, Debug, PartialEq, Eq)]
enum NoPseudoClass {}

impl ToCss for NoPseudoClass {
    fn to_css<W: fmt::Write>(&self, _dest: &mut W) -> fmt::Result {
        match *self {}
    }
}

impl selectors::parser::NonTSPseudoClass for NoPseudoClass {
    fn is_active_or_hover(&self) -> bool {
        match *self {}
    }
    fn is_user_action_state(&self) -> bool {
        match *self {}
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum NoPseudoElement {}

impl ToCss for NoPseudoElement {
    fn to_css<W: fmt::Write>(&self, _dest: &mut W) -> fmt::Result {
        match *self {}
    }
}

impl selectors::parser::PseudoElement for NoPseudoElement {}

impl selectors::parser::SelectorImpl for Impl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = Value;
    type Identifier = Ident;
    type LocalName = Ident;
    type NamespaceUrl = Namespace;
    type NamespacePrefix = Ident;
    type BorrowedLocalName = Ident;
    type BorrowedNamespaceUrl = Namespace;
    type NonTSPseudoClass = NoPseudoClass;
    type PseudoElement = NoPseudoElement;
}

struct SelParser;

impl<'i> selectors::parser::Parser<'i> for SelParser {
    type Impl = Impl;
    type Error = SelectorParseErrorKind;
}

#[derive(Clone, Debug)]
struct El(Handle);

impl El {
    fn siblings(&self) -> Option<(Handle, usize)> {
        let parent = self.0.parent()?;
        let idx = parent
            .children
            .borrow()
            .iter()
            .position(|c| Rc::ptr_eq(c, &self.0))?;
        Some((parent, idx))
    }
}

impl selectors::Element for El {
    type Impl = Impl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(&*self.0)
    }

    fn parent_element(&self) -> Option<Self> {
        self.0.parent().filter(|p| p.is_element()).map(El)
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
        let (parent, idx) = self.siblings()?;
        let children = parent.children.borrow();
        children[..idx]
            .iter()
            .rev()
            .find(|c| c.is_element())
            .cloned()
            .map(El)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let (parent, idx) = self.siblings()?;
        let children = parent.children.borrow();
        children[idx + 1..]
            .iter()
            .find(|c| c.is_element())
            .cloned()
            .map(El)
    }

    fn first_element_child(&self) -> Option<Self> {
        self.0
            .children
            .borrow()
            .iter()
            .find(|c| c.is_element())
            .cloned()
            .map(El)
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.0.name().ns == ns!(html)
    }

    fn has_local_name(&self, local_name: &Ident) -> bool {
        self.0.name().local == local_name.0
    }

    fn has_namespace(&self, ns: &Namespace) -> bool {
        &self.0.name().ns == ns
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.0.name().local == other.0.name().local && self.0.name().ns == other.0.name().ns
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&Namespace>,
        local_name: &Ident,
        operation: &AttrSelectorOperation<&Value>,
    ) -> bool {
        let NodeData::Element { attrs, .. } = &self.0.data else {
            return false;
        };
        attrs.borrow().iter().any(|a| {
            let ns_ok = match ns {
                NamespaceConstraint::Any => true,
                NamespaceConstraint::Specific(url) => &a.name.ns == *url,
            };
            ns_ok && a.name.local == local_name.0 && operation.eval_str(&a.value)
        })
    }

    fn match_non_ts_pseudo_class(
        &self,
        pc: &NoPseudoClass,
        _context: &mut MatchingContext<Impl>,
    ) -> bool {
        match *pc {}
    }

    fn match_pseudo_element(
        &self,
        pe: &NoPseudoElement,
        _context: &mut MatchingContext<Impl>,
    ) -> bool {
        match *pe {}
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        self.0.name().ns == ns!(html)
            && matches!(&*self.0.name().local, "a" | "area" | "link")
            && self.0.attr("href").is_some()
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &Ident, case_sensitivity: CaseSensitivity) -> bool {
        self.0
            .attr("id")
            .is_some_and(|v| case_sensitivity.eq(v.as_bytes(), id.0.as_bytes()))
    }

    fn has_class(&self, name: &Ident, case_sensitivity: CaseSensitivity) -> bool {
        self.0.attr("class").is_some_and(|v| {
            v.split_ascii_whitespace()
                .any(|c| case_sensitivity.eq(c.as_bytes(), name.0.as_bytes()))
        })
    }

    fn has_custom_state(&self, _name: &Ident) -> bool {
        false
    }

    fn imported_part(&self, _name: &Ident) -> Option<Ident> {
        None
    }

    fn is_part(&self, _name: &Ident) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        self.0.children.borrow().iter().all(|c| match &c.data {
            NodeData::Element { .. } => false,
            NodeData::Text(t) => t.borrow().is_empty(),
            _ => true,
        })
    }

    fn is_root(&self) -> bool {
        self.0
            .parent()
            .is_some_and(|p| matches!(p.data, NodeData::Document))
    }

    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        false
    }
}

fn selector_list(css: &str) -> Option<SelectorList<Impl>> {
    let mut parser = cssparser::Parser::new(css);
    SelectorList::parse(&SelParser, &mut parser, ParseRelative::No).ok()
}

/// Every element of `doc` that `css` matches.
fn query(doc: &Handle, css: &str) -> Option<Vec<Handle>> {
    let list = selector_list(css)?;
    let mut all = Vec::new();
    elements(doc, &mut all);
    let mut caches = SelectorCaches::default();
    let mut ctx = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        SelQuirks::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    Some(
        all.into_iter()
            .filter(|e| matches_selector_list(&list, &El(e.clone()), &mut ctx))
            .collect(),
    )
}

fn text_of(doc: &Handle, css: &str) -> String {
    let mut s = String::new();
    if let Some(first) = query(doc, css).and_then(|m| m.into_iter().next()) {
        first.text_content(&mut s);
    }
    s
}

/// Parse `PAGE` and run every expectation. Returns the first mismatch.
fn parse_and_match() -> Result<(), String> {
    let (doc, _errors) = parse(PAGE);
    for &(css, want) in EXPECT {
        match query(&doc, css) {
            None => return Err(format!("selector `{css}` did not parse")),
            Some(m) if m.len() != want => {
                return Err(format!("`{css}` matched {} elements, want {want}", m.len()));
            }
            Some(_) => {}
        }
    }
    let h1 = text_of(&doc, "h1");
    if h1 != H1_TEXT {
        return Err(format!("h1 text {h1:?}, want {H1_TEXT:?}"));
    }
    let title = text_of(&doc, "title");
    if title != TITLE_TEXT {
        return Err(format!("title text {title:?}, want {TITLE_TEXT:?}"));
    }
    if selector_list("p:::bad").is_some() {
        return Err("a malformed selector parsed".into());
    }
    Ok(())
}

// --- The parker -------------------------------------------------------------

const CONTENDERS: usize = 8;
/// Pages each thread parses in `parallel_parse`.
const PARSES: usize = 4;
/// `memchr_threads`: haystack bytes and searches per thread. Sized for an
/// unoptimized build under TCG; the byte-loop reference dominates the cost.
const HAYSTACK: usize = 16 * 1024;
const SEARCHES: usize = 32;
const ROUNDS: u64 = 40;

/// More threads than CPUs on one `parking_lot::Mutex`, and each holder sleeps
/// while holding it, so the others must park.
fn parker_contention() -> bool {
    let lock = Arc::new(parking_lot::Mutex::new(0u64));
    let handles: Vec<_> = (0..CONTENDERS)
        .map(|i| {
            let lock = lock.clone();
            thread::spawn(move || {
                for r in 0..ROUNDS {
                    let mut g = lock.lock();
                    *g += 1;
                    if r % 8 == i as u64 % 8 {
                        thread::sleep(Duration::from_millis(2));
                    }
                }
            })
        })
        .collect();
    let joined = handles.into_iter().all(|h| h.join().is_ok());
    joined && *lock.lock() == CONTENDERS as u64 * ROUNDS
}

/// Threads the hold phase queues (the kernel harness requires this many in
/// `SYS_FUTEX_WAIT` at once).
const HOLD_WAITERS: usize = 6;

/// The hold phase: this thread holds one `parking_lot::Mutex` across a 500 ms
/// sleep while `HOLD_WAITERS` threads queue on it. With a parker that blocks,
/// all of them sit in `SYS_FUTEX_WAIT` meanwhile, which the kernel harness
/// counts; a spinning parker leaves them running, and the harness fails it.
fn parker_hold() -> bool {
    static GOT: AtomicUsize = AtomicUsize::new(0);
    let lock = Arc::new(parking_lot::Mutex::new(()));
    let guard = lock.lock();
    let handles: Vec<_> = (0..HOLD_WAITERS)
        .map(|_| {
            let lock = lock.clone();
            thread::spawn(move || {
                drop(lock.lock());
                GOT.fetch_add(1, Ordering::SeqCst);
            })
        })
        .collect();
    thread::sleep(Duration::from_millis(500));
    let none_before = GOT.load(Ordering::SeqCst) == 0;
    drop(guard);
    let joined = handles.into_iter().all(|h| h.join().is_ok());
    none_before && joined && GOT.load(Ordering::SeqCst) == HOLD_WAITERS
}

/// `parking_lot::Condvar`: a timed wait times out, a notified wait wakes.
fn parker_condvar() -> bool {
    let pair = Arc::new((parking_lot::Mutex::new(false), parking_lot::Condvar::new()));
    {
        let mut g = pair.0.lock();
        let t0 = Instant::now();
        let r = pair.1.wait_for(&mut g, Duration::from_millis(20));
        if !r.timed_out() || t0.elapsed() < Duration::from_millis(20) {
            return false;
        }
    }
    let p2 = pair.clone();
    let waker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        *p2.0.lock() = true;
        p2.1.notify_one();
    });
    let mut g = pair.0.lock();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !*g {
        if pair.1.wait_until(&mut g, deadline).timed_out() {
            return false;
        }
    }
    drop(g);
    waker.join().is_ok()
}

/// Threads parse at once, each interning its own custom element names, so the
/// atom table's buckets are contended. Every thread must see the same tree.
fn parallel_parse() -> bool {
    static INTERNED: AtomicUsize = AtomicUsize::new(0);
    let handles: Vec<_> = (0..CONTENDERS)
        .map(|i| {
            thread::spawn(move || {
                for j in 0..PARSES {
                    let page =
                        format!("{PAGE}<x-seat-d-{i}-{j} data-t{i}=\"{j}\"></x-seat-d-{i}-{j}>");
                    let (doc, _) = parse(&page);
                    let css = format!("x-seat-d-{i}-{j}[data-t{i}=\"{j}\"]");
                    if query(&doc, &css).map(|m| m.len()) != Some(1)
                        || query(&doc, "p").map(|m| m.len()) != Some(3)
                    {
                        return false;
                    }
                    INTERNED.fetch_add(1, Ordering::Relaxed);
                }
                true
            })
        })
        .collect();
    let ok = handles.into_iter().all(|h| h.join().unwrap_or(false));
    ok && INTERNED.load(Ordering::Relaxed) == CONTENDERS * PARSES
}

// --- memchr -----------------------------------------------------------------

fn naive_find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Searches on several threads that yield between searches, each checked
/// against a byte loop. Returns the number of checked searches, or 0 on a
/// mismatch.
fn memchr_threads() -> u64 {
    static CHECKED: AtomicU64 = AtomicU64::new(0);
    let handles: Vec<_> = (0..CONTENDERS)
        .map(|i| {
            thread::spawn(move || {
                let mut hay = vec![0u8; HAYSTACK];
                let mut x = 0x9e37_79b9u32.wrapping_mul(i as u32 + 1);
                for b in hay.iter_mut() {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    *b = b'a' + (x % 26) as u8;
                }
                for k in 0..SEARCHES {
                    let at = (k * 997 + i * 31) % (hay.len() - 16);
                    let needle: Vec<u8> = hay[at..at + 12].to_vec();
                    let byte = hay[at];
                    if memchr::memchr(byte, &hay) != hay.iter().position(|&b| b == byte)
                        || memchr::memrchr(byte, &hay) != hay.iter().rposition(|&b| b == byte)
                        || memchr::memmem::find(&hay, &needle) != naive_find(&hay, &needle)
                    {
                        return false;
                    }
                    CHECKED.fetch_add(1, Ordering::Relaxed);
                    thread::yield_now();
                }
                true
            })
        })
        .collect();
    let ok = handles.into_iter().all(|h| h.join().unwrap_or(false));
    if ok {
        CHECKED.load(Ordering::Relaxed)
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("{TAG} start: html5ever 0.40.1, selectors 0.41.0, cssparser 0.38.0 (vendored)");

    match parse_and_match() {
        Ok(()) => check(1, "parse + match: every hand-derived count and text", true),
        Err(why) => {
            println!("{TAG} {why}");
            check(1, "parse + match: every hand-derived count and text", false);
        }
    }
    check(
        2,
        "parking_lot::Mutex: 8 contenders, sleeping holders",
        parker_contention(),
    );
    check(
        3,
        "parking_lot::Condvar: timeout and wake",
        parker_condvar(),
    );
    check(
        6,
        "parking_lot::Mutex hold: 6 waiters queue behind a sleeping holder",
        parker_hold(),
    );
    check(
        4,
        "parallel parse: 8 threads interning atoms",
        parallel_parse(),
    );

    let avx2 = std::arch::is_x86_feature_detected!("avx2");
    println!("{TAG} memchr: avx2 detected = {}", avx2 as u8);
    let checked = memchr_threads();
    check(
        5,
        "memchr/memrchr/memmem on 8 yielding threads == byte loop",
        checked == (CONTENDERS * SEARCHES) as u64,
    );

    let code = if avx2 { PASS_AVX2 } else { PASS_SSE2 };
    println!("{TAG} pass (exit {code})");
    std::process::exit(code)
}
