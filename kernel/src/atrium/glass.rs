// SPDX-License-Identifier: MPL-2.0
//! G1's live frosted-glass draw seam (GPU P1 Slice 6).
//!
//! A glass draw has three intentionally distinct compositor-owned buffers:
//! `capture` (the backdrop snapshot), `blurred` (the GPU/CPU result), and
//! `composite` (the vibrancy-tinted G1 surface).  The separation is not an
//! optimization detail: it makes the pre-existing `dst != self` guard a
//! construction invariant and prevents a client-mapped backdrop from becoming
//! an in-place GPU operand.
//!
//! This module stops at a completed [`DispatchToken`].  It never calls
//! `present`, `arm_flip`, or any scanout API.  The compositor must separately
//! hold `GpuSubmitCap` and pass the existing `scanout_authorized()` gate before
//! the `composite` surface can reach the display.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec;

use super::blur::{self, BlurDims, G1_VIBRANCY_TINT, GlassTint};
use super::buffer::SharedPixelBuffer;
use super::logic::{BufferDescV1, DamageRectV1};
use crate::ai::tensor::Device;
use crate::drivers::accel::{self, AccelDriver, DispatchToken, gpu_compute};
use crate::errno::EINVAL;

/// The trusted compositor's fixed glass material.  Clients never select a
/// pipeline, weights, tint, or GPU address through this interface.
pub const GLASS_RADIUS: usize = blur::BLUR_RADIUS_R8;
pub const GLASS_TINT: GlassTint = G1_VIBRANCY_TINT;

/// Completed compute-fence evidence for one glass draw.  `sequence == 0` means
/// the CPU floor (either GPU-off or an enabled-path degrade); it is never a
/// present epoch and carries no scanout authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlassDraw {
    pub fence: DispatchToken,
    pub output_damage: DamageRectV1,
}

/// One compositor-owned glass material instance.
pub struct GlassSurface {
    capture: Arc<SharedPixelBuffer>,
    blurred: Arc<SharedPixelBuffer>,
    composite: Arc<SharedPixelBuffer>,
}

impl GlassSurface {
    /// Allocate three distinct GPU-reachable compositor surfaces.  Under the
    /// Slice-5 graphics-narrowing switch they route through the graphics arena;
    /// otherwise `alloc_gpu` preserves the old buddy-backed state.
    pub fn create(desc: BufferDescV1) -> Result<Self, i64> {
        Ok(Self {
            capture: SharedPixelBuffer::create_gpu(desc)?,
            blurred: SharedPixelBuffer::create_gpu(desc)?,
            composite: SharedPixelBuffer::create_gpu(desc)?,
        })
    }

    pub fn composite(&self) -> &Arc<SharedPixelBuffer> {
        &self.composite
    }

    /// Execute capture -> blur/floor -> vibrancy tint -> G1 composite-surface
    /// production.  The returned compute token is already complete because
    /// `gpu_compute::blur_backdrop` waits with its CP-18 bound; it is not a
    /// display fence.  `None` means no accelerator driver is available and the
    /// CPU floor stays live.
    pub fn draw(
        &self,
        backdrop: &Arc<SharedPixelBuffer>,
        changed: DamageRectV1,
        driver: Option<&Arc<dyn AccelDriver>>,
    ) -> Result<GlassDraw, i64> {
        self.draw_with_material(
            backdrop,
            changed,
            driver,
            GLASS_RADIUS,
            &blur::WEIGHTS_R8_Q16,
            GLASS_TINT,
        )
    }

    /// The fixed public material above is the production G1 path.  Keeping
    /// this crate-visible variant lets the QEMU byte oracle use r=1 without
    /// giving a client any material-selection ABI.
    pub(crate) fn draw_with_material(
        &self,
        backdrop: &Arc<SharedPixelBuffer>,
        changed: DamageRectV1,
        driver: Option<&Arc<dyn AccelDriver>>,
        radius: usize,
        weights: &[i32],
        tint: GlassTint,
    ) -> Result<GlassDraw, i64> {
        let desc = self.capture.desc();
        if Arc::ptr_eq(backdrop, &self.capture)
            || Arc::ptr_eq(backdrop, &self.blurred)
            || Arc::ptr_eq(backdrop, &self.composite)
            || Arc::ptr_eq(&self.capture, &self.blurred)
            || Arc::ptr_eq(&self.capture, &self.composite)
            || Arc::ptr_eq(&self.blurred, &self.composite)
            || backdrop.desc() != desc
            || !super::logic::validate_damage(changed, &desc)
        {
            return Err(EINVAL);
        }

        // Capture the client/composited backdrop before handing an arena-owned
        // snapshot to compute.  The current CPU floor recomputes full bounds;
        // the committed output damage below is still radius-expanded and the
        // pinned source-halo rule remains the future incremental boundary.
        backdrop.copy_pixels_to(&self.capture)?;
        let dims = BlurDims {
            width: desc.width as usize,
            height: desc.height as usize,
            stride: desc.stride as usize,
            radius,
        };
        let blur_enabled = super::glass_blur_enabled();
        let fence = if blur_enabled {
            let src = self.capture.contiguous_pixels()?;
            let mut dst = self.blurred.contiguous_pixels_mut()?;
            let mut mid = vec![0u8; desc.bytes as usize];
            if let Some(driver) = driver {
                gpu_compute::blur_backdrop(
                    driver,
                    src.as_slice(),
                    &mut mid,
                    dst.as_mut_slice(),
                    dims,
                    weights,
                )
                .map_err(|_| EINVAL)?
            } else {
                blur::blur_pass_h(src.as_slice(), &mut mid, dims, weights).map_err(|_| EINVAL)?;
                blur::blur_pass_v(&mid, dst.as_mut_slice(), dims, weights).map_err(|_| EINVAL)?;
                DispatchToken {
                    device: Device::Cpu,
                    sequence: 0,
                }
            }
        } else {
            // Reduce Transparency: do not submit; use the unblurred solid-floor
            // backdrop and keep the damage exactly scoped to the source change.
            let src = self.capture.contiguous_pixels()?;
            let mut dst = self.blurred.contiguous_pixels_mut()?;
            blur::copy_backdrop(src.as_slice(), dst.as_mut_slice(), dims).map_err(|_| EINVAL)?;
            DispatchToken {
                device: driver.map_or(Device::Cpu, |d| d.descriptor().device),
                sequence: 0,
            }
        };

        let src = self.blurred.contiguous_pixels()?;
        let mut dst = self.composite.contiguous_pixels_mut()?;
        blur::apply_vibrancy_tint(src.as_slice(), dst.as_mut_slice(), dims, tint)
            .map_err(|_| EINVAL)?;

        let output_damage = if blur_enabled {
            let out = blur::expand_output_rect(
                blur::BlurRect {
                    x: changed.x as u32,
                    y: changed.y as u32,
                    width: changed.width as u32,
                    height: changed.height as u32,
                },
                radius as u32,
                desc.width,
                desc.height,
            );
            DamageRectV1 {
                x: u16::try_from(out.x).map_err(|_| EINVAL)?,
                y: u16::try_from(out.y).map_err(|_| EINVAL)?,
                width: u16::try_from(out.width).map_err(|_| EINVAL)?,
                height: u16::try_from(out.height).map_err(|_| EINVAL)?,
            }
        } else {
            changed
        };
        self.composite.commit_damage(output_damage)?;
        Ok(GlassDraw {
            fence,
            output_damage,
        })
    }

    /// Resolve the QEMU compute substrate if it is registered.  Production
    /// compositor policy may provide a different trusted driver; no user input
    /// selects this value.
    pub fn qemu_driver() -> Option<Arc<dyn AccelDriver>> {
        accel::find_device_by_driver("accel_sim").map(|(_, driver)| driver)
    }
}
