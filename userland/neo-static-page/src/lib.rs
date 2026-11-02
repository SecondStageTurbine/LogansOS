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
use paint_api::display_list::PaintTimingInfo;
use paint_api::{CrossProcessPaintApi, PaintMessage};
use servo_base::Epoch;
use servo_base::generic_channel::{self, GenericCallback};
use servo_base::id::{PipelineNamespace, PipelineNamespaceId, TEST_PIPELINE_ID, TEST_WEBVIEW_ID};
use servo_url::ServoUrl;
use style::stylesheets::UrlExtraData;
use webrender_api::{FontInstanceKey, FontKey, IdNamespace, ImageKey};

mod no_images;
pub use no_images::NoImages;

/// What layout sent to "paint" during a reflow.
#[derive(Default)]
pub struct Painted {
    /// How many display lists layout sent.
    pub display_lists: usize,
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
            PaintMessage::SendDisplayList { .. } => {
                if let Ok(mut p) = self.painted.lock() {
                    p.display_lists += 1;
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
        let font_context = Arc::new(FontContext::new(
            Arc::new(font_service.to_proxy()),
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
        let layout = LayoutFactoryImpl().create(config);
        StaticPage {
            document,
            layout,
            painted,
            url_for_fonts: extra_url,
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
                dirty_root: Some(self.document.address(0)),
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

    /// How many display lists layout has sent.
    pub fn display_lists(&self) -> usize {
        self.painted.lock().map(|p| p.display_lists).unwrap_or(0)
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
