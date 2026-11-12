// SPDX-License-Identifier: MPL-2.0
//! An image cache with no images: the first static page has none, so every
//! request answers "failed to load" and nothing is ever pending.

use std::sync::Arc;

use malloc_size_of::MallocSizeOfOps;
use net_traits::FetchResponseMsg;
use net_traits::image_cache::{
    Image, ImageCache, ImageCacheResponseCallback, ImageCacheResult, ImageLoadListener,
    PendingImageId, VectorImageId,
};
use net_traits::request::CorsSettings;
use pixels::RasterImage;
use profile_traits::mem::Report;
use servo_base::id::PipelineId;
use servo_url::{ImmutableOrigin, ServoUrl};
use uuid::Uuid;
use webrender_api::ImageKey;
use webrender_api::units::DeviceIntSize;

/// The image cache of a page with no images.
pub struct NoImages;

impl ImageCache for NoImages {
    fn memory_reports(&self, _prefix: &str, _ops: &mut MallocSizeOfOps) -> Vec<Report> {
        Vec::new()
    }

    fn get_image_key(&self) -> Option<ImageKey> {
        None
    }

    fn get_image(
        &self,
        _url: ServoUrl,
        _origin: ImmutableOrigin,
        _cors_setting: Option<CorsSettings>,
    ) -> Option<Image> {
        None
    }

    fn get_cached_image_status(
        &self,
        _url: ServoUrl,
        _origin: ImmutableOrigin,
        _cors_setting: Option<CorsSettings>,
    ) -> ImageCacheResult {
        ImageCacheResult::FailedToLoadOrDecode
    }

    fn rasterize_vector_image(
        &self,
        _image_id: VectorImageId,
        _size: DeviceIntSize,
        _svg_id: Option<Uuid>,
    ) -> Option<RasterImage> {
        None
    }

    fn add_rasterization_complete_listener(
        &self,
        _pipeline_id: PipelineId,
        _image_id: VectorImageId,
        _size: DeviceIntSize,
        _callback: ImageCacheResponseCallback,
    ) {
    }

    fn evict_rasterized_image(&self, _svg_id: &Uuid) {}

    fn evict_completed_image(
        &self,
        _url: &ServoUrl,
        _origin: &ImmutableOrigin,
        _cors_setting: &Option<CorsSettings>,
    ) {
    }

    fn get_broken_image_icon(&self) -> Option<Arc<RasterImage>> {
        None
    }

    fn add_listener(&self, _listener: ImageLoadListener) {}

    fn notify_pending_response(&self, _id: PendingImageId, _action: FetchResponseMsg) {}

    fn dispatch_fill_key_cache_with_batch_of_keys(&self, _image_keys: Vec<ImageKey>) {}

    fn clear(&self) {}
}
