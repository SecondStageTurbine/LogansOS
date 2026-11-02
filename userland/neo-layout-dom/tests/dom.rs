// SPDX-License-Identifier: MPL-2.0
//! Host checks of the static DOM through the traits layout uses. Each expected
//! value is read off the page below by hand.

use html5ever::{local_name, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode, LayoutNodeType};
use neo_layout_dom::{Document, ServoLayoutNode};
use style::dom::NodeInfo;
use style::stylesheets::UrlExtraData;

const PAGE: &str = "<!DOCTYPE html><html><head><style>p { color: red }</style></head>\
<body><p id=a class='x y'>Hi</p><table><tr><td colspan=2 rowspan=3>c</td></tr></table></body></html>";

fn doc() -> Box<Document> {
    let url = UrlExtraData::from(url::Url::parse("about:blank").expect("about:blank"));
    Document::parse(PAGE, &url)
}

fn node(doc: &Document, id: usize) -> ServoLayoutNode<'_> {
    // SAFETY: the address is of a node of `doc`, which outlives the handle.
    unsafe { ServoLayoutNode::new(&doc.address(id)) }
}

fn element_named<'a>(doc: &'a Document, name: &str) -> ServoLayoutNode<'a> {
    (0..doc.len())
        .map(|i| node(doc, i))
        .find(|n| n.as_element().is_some_and(|e| &**e.local_name() == name))
        .unwrap_or_else(|| panic!("<{name}> is in the page"))
}

#[test]
fn the_page_has_the_shape_html5ever_gives_it() {
    let d = doc();
    // document, doctype, html, head, style, "p { color: red }", body, p, "Hi",
    // table, tbody (inserted by the parser), tr, td, "c".
    assert_eq!(d.len(), 14);
    let html = element_named(&d, "html");
    let kids: Vec<_> = html
        .dom_children()
        .filter_map(|c| c.as_element().map(|e| e.local_name().to_string()))
        .collect();
    assert_eq!(kids, ["head", "body"]);
    assert_eq!(d.style_texts(), "p { color: red }");
}

#[test]
fn type_ids_name_the_elements_layout_special_cases() {
    let d = doc();
    let t = |name| element_named(&d, name).type_id();
    use LayoutElementType as E;
    assert_eq!(t("html"), Some(LayoutNodeType::Element(E::HTMLHtmlElement)));
    assert_eq!(t("body"), Some(LayoutNodeType::Element(E::HTMLBodyElement)));
    assert_eq!(
        t("p"),
        Some(LayoutNodeType::Element(E::HTMLParagraphElement))
    );
    assert_eq!(
        t("table"),
        Some(LayoutNodeType::Element(E::HTMLTableElement))
    );
    assert_eq!(
        t("tbody"),
        Some(LayoutNodeType::Element(E::HTMLTableSectionElement))
    );
    assert_eq!(
        t("tr"),
        Some(LayoutNodeType::Element(E::HTMLTableRowElement))
    );
    assert_eq!(
        t("td"),
        Some(LayoutNodeType::Element(E::HTMLTableCellElement))
    );
    assert_eq!(t("head"), Some(LayoutNodeType::Element(E::Element)));
}

#[test]
fn text_attributes_and_spans() {
    let d = doc();
    let p = element_named(&d, "p");
    let text = p.dom_children().next().expect("<p> has its text");
    assert!(text.is_text_node());
    assert_eq!(text.type_id(), Some(LayoutNodeType::Text));
    assert_eq!(&*text.text_content(), "Hi");
    // An element has no text of its own.
    assert_eq!(&*p.text_content(), "");

    let e = p.as_element().expect("an element");
    assert_eq!(e.attribute_as_str(&ns!(), &local_name!("id")), Some("a"));
    assert_eq!(
        e.attribute_as_str(&ns!(), &local_name!("class")),
        Some("x y")
    );
    assert_eq!(e.attribute_as_str(&ns!(), &local_name!("title")), None);

    let td = element_named(&d, "td");
    assert_eq!(td.table_colspan(), Some(2));
    assert_eq!(td.table_rowspan(), Some(3));
    assert_eq!(td.table_span(), None);
}

#[test]
fn root_and_body_are_recognised() {
    let d = doc();
    let html = element_named(&d, "html").as_element().expect("element");
    let body = element_named(&d, "body").as_element().expect("element");
    let p = element_named(&d, "p").as_element().expect("element");
    assert!(html.is_root());
    assert!(!body.is_root());
    assert!(body.is_body_element_of_html_element_root());
    assert!(!p.is_body_element_of_html_element_root());
    assert!(p.is_html_element_in_html_document());
}

#[test]
fn parents_follow_the_tree() {
    let d = doc();
    let td = element_named(&d, "td");
    // SAFETY: test-only, nothing else touches the tree.
    let tr = unsafe { td.dangerous_dom_parent() }.expect("<td> has a parent");
    assert_eq!(
        tr.as_element()
            .map(|e| e.local_name().to_string())
            .as_deref(),
        Some("tr")
    );
    let flat = unsafe { td.dangerous_flat_tree_parent() }.expect("and a flat-tree parent");
    assert_eq!(flat, tr);
}
