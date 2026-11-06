// SPDX-License-Identifier: MPL-2.0
//! OT-61 L2 — a static HTML page laid out by Servo's layout with no script
//! engine (`docs/THINK-2026-11-01-ot61-first-page-layout.md` §3 and §5).
//!
//! In Servo, `script` drives layout. Here this crate does: it parses the page
//! into `neo-layout-dom`, makes layout through `LayoutFactoryImpl`, adds the
//! page's `<style>` sheets, and asks for one reflow. Everything layout needs
//! from the rest of the engine is answered in-process:
//!
//! - paint: [`PaintHost`] answers layout's and the font service's key requests
//!   and keeps each display list layout sends (L3 hands them to WebRender);
//! - images: [`NoImages`], a cache that has none (a static first page);
//! - fonts: a `FontContext` over `SystemFontService` (L1's bundled fonts);
//! - script, embedder, network and profiler channels: in-process, and nothing
//!   a static page sends needs an answer.
//!
//! WIP: this is the driver's shape; L2's gate (fragment rectangles of a page
//! checked by hand) comes with it once L0 links.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use euclid::{Scale, Size2D};
use fonts::{
    CspViolationHandler, FontContext, NetworkTimingHandler, SystemFontService,
    WebFontDocumentContext,
};
use layout::LayoutFactoryImpl;
use layout_api::{
    Layout, LayoutConfig, LayoutFactory, ReflowGoal, ReflowRequest, ReflowRequestRestyle,
    RestyleReason,
};
use malloc_size_of_derive::MallocSizeOf;
use neo_layout_dom::Document;
use paint_api::display_list::{PaintDisplayListInfo, PaintTimingInfo};
use paint_api::{CrossProcessPaintApi, PaintMessage, SerializableDisplayListPayload};
use servo_base::Epoch;
use servo_base::generic_channel::{self, GenericCallback, GenericReceiver};
use servo_base::id::{PipelineNamespace, PipelineNamespaceId, TEST_PIPELINE_ID, TEST_WEBVIEW_ID};
use servo_url::ServoUrl;
use style::context::QuirksMode;
use style::media_queries::MediaList;
use style::stylesheets::{AllowImportRules, Origin, Stylesheet, UrlExtraData};
use webrender_api::{
    BuiltDisplayList, DisplayListPayload, FontInstanceKey, FontKey, IdNamespace, ImageKey,
};

mod no_images;
pub mod oracle;
pub use no_images::NoImages;

/// One display list layout sent: its descriptor, and the two channels the
/// rest arrives on. `send_display_list` sends `SendDisplayList` first and the
/// info and data after it, so they can't be read inside the paint callback
/// (it runs during that first send); [`StaticPage::take_display_list`] reads
/// them once the reflow has returned.
type SentDisplayList = (
    webrender_api::BuiltDisplayListDescriptor,
    GenericReceiver<PaintDisplayListInfo>,
    GenericReceiver<SerializableDisplayListPayload>,
);

/// What layout sent to "paint" during a reflow.
#[derive(Default)]
pub struct Painted {
    /// How many display lists layout sent.
    pub display_lists: usize,
    /// The display lists not yet taken, oldest first.
    sent: Vec<SentDisplayList>,
}

/// The in-process stand-in for Servo's paint thread: it hands out WebRender
/// keys from one namespace and records what layout sends.
pub struct PaintHost {
    next_key: u32,
    painted: Arc<Mutex<Painted>>,
}

/// The id namespace this host's keys come from.
const NAMESPACE: IdNamespace = IdNamespace(1);

impl PaintHost {
    fn key(&mut self) -> u32 {
        self.next_key += 1;
        self.next_key
    }

    fn handle(&mut self, msg: PaintMessage) {
        match msg {
            PaintMessage::GenerateFontKeys(fonts, instances, reply, _painter) => {
                let f = (0..fonts)
                    .map(|_| FontKey::new(NAMESPACE, self.key()))
                    .collect();
                let i = (0..instances)
                    .map(|_| FontInstanceKey::new(NAMESPACE, self.key()))
                    .collect();
                let _ = reply.send((f, i));
            }
            PaintMessage::GenerateImageKey(_webview, reply) => {
                let _ = reply.send(ImageKey::new(NAMESPACE, self.key()));
            }
            PaintMessage::SendDisplayList {
                display_list_descriptor,
                display_list_info_receiver,
                display_list_data_receiver,
                ..
            } => {
                if let Ok(mut p) = self.painted.lock() {
                    p.display_lists += 1;
                    p.sent.push((
                        display_list_descriptor,
                        display_list_info_receiver,
                        display_list_data_receiver,
                    ));
                }
            }
            // Fonts added, frames requested, epochs, viewport: nothing to
            // answer for a static page.
            _ => {}
        }
    }
}

/// One static page and the layout laying it out.
pub struct StaticPage {
    document: Box<Document>,
    layout: Box<dyn Layout>,
    painted: Arc<Mutex<Painted>>,
    url_for_fonts: ServoUrl,
    /// The font service thread's proxy, to stop it when the page goes
    /// (`Drop`).
    font_service: Arc<fonts::SystemFontServiceProxy>,
    /// The receiving ends of channels nothing here reads: kept so a send on
    /// them is not an error.
    _keep: (
        Box<dyn std::any::Any>,
        Box<dyn std::any::Any>,
        crossbeam_channel::Receiver<embedder_traits::EmbedderMsg>,
    ),
}

impl StaticPage {
    /// Parse `html` and make a layout for it, `width` x `height` CSS px.
    pub fn new(html: &str, width: f32, height: f32) -> StaticPage {
        // This thread is the page's script thread, as far as layout knows:
        // Servo's script thread starts with `initialize(SCRIPT)`
        // (`servo-script` 0.7.0, `event_loop/script_thread.rs:496`), and
        // layout's `ThreadStateRestorer` (`servo-layout-api`, debug builds)
        // swaps SCRIPT for LAYOUT around each reflow and asserts SCRIPT was
        // set. Without it the first reflow panics in `thread_state::exit`
        // (the first on-target L2 run, 2026-11-03).
        style::thread_state::initialize(style::thread_state::ThreadState::SCRIPT);
        PipelineNamespace::install(PipelineNamespaceId(1));
        let url = ServoUrl::parse("about:blank").expect("about:blank");
        let extra = UrlExtraData::from(url.as_url().clone());
        let extra_url = url.clone();
        let document = Document::parse(html, &extra);

        let painted = Arc::new(Mutex::new(Painted::default()));
        let mut host = PaintHost {
            next_key: 0,
            painted: painted.clone(),
        };
        let paint_api = CrossProcessPaintApi::new(
            GenericCallback::new(move |msg| {
                if let Ok(msg) = msg {
                    host.handle(msg);
                }
            })
            .expect("an in-process callback"),
        );

        let (memory_sender, memory_port) = generic_channel::channel().expect("a channel");
        let font_service = SystemFontService::spawn(
            paint_api.clone(),
            profile_traits::mem::ProfilerChan(memory_sender),
        );
        let font_proxy = Arc::new(font_service.to_proxy());
        let font_context = Arc::new(FontContext::new(
            font_proxy.clone(),
            paint_api.clone(),
            net_traits::ResourceThreads::new(generic_channel::channel().expect("a channel").0),
        ));

        let viewport_details = embedder_traits::ViewportDetails {
            size: Size2D::new(width, height),
            hidpi_scale_factor: Scale::new(1.0),
            device_size: Size2D::new(width, height),
        };
        let (script_chan, script_port) = generic_channel::channel().expect("a channel");
        let (embedder_sender, embedder_port) = crossbeam_channel::unbounded();
        let config = LayoutConfig {
            id: TEST_PIPELINE_ID,
            webview_id: TEST_WEBVIEW_ID,
            url,
            is_iframe: false,
            script_chan,
            image_cache: Arc::new(NoImages),
            font_context,
            time_profiler_chan: profile_traits::time::ProfilerChan(None),
            paint_api,
            viewport_details,
            user_stylesheets: Rc::new(Vec::new()),
            theme: embedder_traits::Theme::Light,
            embedder_chan: embedder_traits::ScriptToEmbedderChan::new(
                embedder_sender,
                Box::new(NoWaker),
            ),
        };
        let mut layout = LayoutFactoryImpl().create(config);
        // The page's own `<style>` sheets, in document order, as one author
        // sheet on the document's author lock.
        let css = document.style_texts();
        if !css.is_empty() {
            let lock = &document.shared_style_locks().author;
            let sheet = servo_arc::Arc::new(Stylesheet::from_str(
                &css,
                extra.clone(),
                Origin::Author,
                servo_arc::Arc::new(lock.wrap(MediaList::empty())),
                lock.clone(),
                None,
                None,
                QuirksMode::NoQuirks,
                AllowImportRules::No,
            ));
            layout.add_stylesheet(sheet, None);
        }
        StaticPage {
            document,
            layout,
            painted,
            url_for_fonts: extra_url,
            font_service: font_proxy,
            _keep: (Box::new(script_port), Box::new(memory_port), embedder_port),
        }
    }

    /// The parsed document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Style and lay out the whole page once. `true` if layout produced a
    /// result.
    pub fn reflow(&mut self, width: f32, height: f32) -> bool {
        let root = self.document.address(0);
        let url = self.url_for_fonts.clone();
        let request = ReflowRequest {
            document: root,
            epoch: Epoch(1),
            restyle: Some(ReflowRequestRestyle {
                reason: RestyleReason::empty(),
                // An element (layout unwraps `as_element()` on it): the
                // document element, as Servo's script passes for a first
                // reflow. The document node itself panicked layout on
                // target (layout_impl.rs:1297, 2026-11-03).
                dirty_root: Some(
                    self.document.address(
                        self.document
                            .document_element()
                            .expect("html5ever always makes <html>"),
                    ),
                ),
                stylesheets_changed: true,
                pending_restyles: Vec::new(),
            }),
            viewport_details: embedder_traits::ViewportDetails {
                size: Size2D::new(width, height),
                hidpi_scale_factor: Scale::new(1.0),
                device_size: Size2D::new(width, height),
            },
            reflow_goal: ReflowGoal::UpdateTheRendering,
            origin: url.origin(),
            animation_timeline_value: 0.0,
            animations: Default::default(),
            animating_images: Arc::new(parking_lot::RwLock::new(Default::default())),
            highlighted_dom_node: None,
            frame_focused: false,
            halt_lcp: true,
            paint_timing_eligible: false,
            paint_timing_info: PaintTimingInfo::now(),
            document_context: WebFontDocumentContext {
                policy_container: Default::default(),
                request_client: net_traits::request::RequestClient {
                    preloaded_resources: Default::default(),
                    policy_container: Default::default(),
                    origin: net_traits::request::Origin::Origin(url.origin()),
                    is_nested_browsing_context: false,
                    insecure_requests_policy:
                        net_traits::request::InsecureRequestsPolicy::DoNotUpgrade,
                    has_trustworthy_ancestor_origin: false,
                },
                document_url: url,
                csp_handler: Box::new(NoCsp),
                network_timing_handler: Box::new(NoTiming),
            },
            accessibility_damage: None,
            rooted_nodes_for_accessibility_integrity_check: None,
        };
        self.layout.reflow(request).is_some()
    }

    /// The border box of the element with `id`, in CSS px (x, y, width,
    /// height), rounded to whole px, after a reflow.
    pub fn border_box(&self, id: &str) -> Option<[i32; 4]> {
        let node = self.document.element_by_id(id)?;
        let rect = self.layout.query_box_area(
            self.document.address(node),
            layout_api::BoxAreaType::Border,
            false,
        )?;
        Some([
            rect.origin.x.to_px(),
            rect.origin.y.to_px(),
            rect.size.width.to_px(),
            rect.size.height.to_px(),
        ])
    }

    /// The newest display list layout sent, rebuilt as WebRender takes it
    /// (`BuiltDisplayList::from_data`, as Servo's paint thread does), with
    /// layout's info for it (pipeline, epoch, viewport). `None` if layout
    /// sent none, or its channels closed. Older ones are dropped.
    pub fn take_display_list(&self) -> Option<(PaintDisplayListInfo, BuiltDisplayList)> {
        let (descriptor, info, data) = {
            let mut p = self.painted.lock().ok()?;
            let last = p.sent.pop()?;
            p.sent.clear();
            last
        };
        let info = info.recv().ok()?;
        let data = data.recv().ok()?;
        let payload = DisplayListPayload {
            items_data: data.items_data,
            spatial_tree: data.spatial_tree,
        };
        Some((info, BuiltDisplayList::from_data(payload, descriptor)))
    }

    /// How many display lists layout has sent.
    pub fn display_lists(&self) -> usize {
        self.painted.lock().map(|p| p.display_lists).unwrap_or(0)
    }
}

impl Drop for StaticPage {
    /// Stop the font service thread before its channels go: Servo's shutdown
    /// sends it `Exit` and waits for the reply
    /// (`SystemFontServiceProxy::exit`). Without that, dropping the page
    /// disconnects the service's channel and its loop's `recv().unwrap()`
    /// panics (`system_font_service.rs:123`); with `panic = "abort"` that ends
    /// the process (the fourth on-target L2 run, 2026-11-03, after every
    /// border box had matched).
    fn drop(&mut self) {
        self.font_service.exit();
    }
}

/// Never woken: this host runs one reflow on its own thread.
struct NoWaker;

impl embedder_traits::EventLoopWaker for NoWaker {
    fn clone_box(&self) -> Box<dyn embedder_traits::EventLoopWaker> {
        Box::new(NoWaker)
    }

    fn wake(&self) {}
}

/// A static page fetches no web fonts, so it has no CSP violations to report.
#[derive(Debug, MallocSizeOf)]
struct NoCsp;

impl CspViolationHandler for NoCsp {
    fn process_violations(&self, _violations: Vec<content_security_policy::Violation>) {}

    fn clone(&self) -> Box<dyn CspViolationHandler> {
        Box::new(NoCsp)
    }
}

/// ... and no fetch timings.
#[derive(Debug, MallocSizeOf)]
struct NoTiming;

impl NetworkTimingHandler for NoTiming {
    fn submit_timing(&self, _url: ServoUrl, _response: net_traits::ResourceFetchTiming) {}

    fn clone(&self) -> Box<dyn NetworkTimingHandler> {
        Box::new(NoTiming)
    }
}
