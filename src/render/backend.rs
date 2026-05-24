use anyhow::Result;

use crate::{
    animation::loop_time::LoopTime,
    project::render_settings::RenderBackendStatus,
    render::{frame_buffer::FrameBuffer, gpu_renderer::GpuRenderer, renderer::RenderParams},
};

pub struct RenderedFrame {
    pub frame: FrameBuffer,
    pub status: RenderBackendStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRenderSpec {
    pub frame_index: u32,
    pub total_frames: u32,
    pub time: LoopTime,
    pub width: u32,
    pub height: u32,
}

impl FrameRenderSpec {
    pub fn new(frame_index: u32, total_frames: u32, width: u32, height: u32) -> Self {
        let total_frames = total_frames.max(1);
        Self {
            frame_index: frame_index % total_frames,
            total_frames,
            time: LoopTime::from_frame(frame_index, total_frames),
            width,
            height,
        }
    }
}

pub struct BackendRenderers {
    gpu_renderer: Option<GpuRenderer>,
    gpu_factory: Box<dyn FnMut() -> Result<GpuRenderer> + Send>,
}

impl BackendRenderers {
    pub fn new() -> Self {
        Self::with_gpu_factory(GpuRenderer::new)
    }

    pub(crate) fn with_gpu_factory(
        gpu_factory: impl FnMut() -> Result<GpuRenderer> + Send + 'static,
    ) -> Self {
        Self {
            gpu_renderer: None,
            gpu_factory: Box::new(gpu_factory),
        }
    }

    pub fn render_frame(
        &mut self,
        params: &RenderParams,
        time: LoopTime,
        width: u32,
        height: u32,
    ) -> Result<RenderedFrame> {
        if self.gpu_renderer.is_none() {
            self.gpu_renderer = Some((self.gpu_factory)()?);
        }

        let renderer = self
            .gpu_renderer
            .as_mut()
            .expect("GPU renderer should be initialized");
        let frame = renderer.try_render_frame(params, time, width, height)?;
        Ok(RenderedFrame {
            frame,
            status: RenderBackendStatus::gpu(),
        })
    }

    pub fn render_indexed_frame(
        &mut self,
        params: &RenderParams,
        spec: FrameRenderSpec,
    ) -> Result<RenderedFrame> {
        self.render_frame(params, spec.time, spec.width, spec.height)
    }
}

impl Default for BackendRenderers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::BackendRenderers;
    use crate::{animation::loop_time::LoopTime, render::renderer::RenderParams};

    #[test]
    fn gpu_factory_failure_is_returned_without_fallback() {
        let mut renderers =
            BackendRenderers::with_gpu_factory(|| Err(anyhow!("forced adapter failure")));

        let error = match renderers.render_frame(
            &RenderParams::default(),
            LoopTime::from_frame(0, 24),
            8,
            8,
        ) {
            Ok(_) => panic!("GPU-only renderer must not synthesize a fallback frame"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("forced adapter failure"));
    }

    #[test]
    fn gpu_render_reports_gpu_backend_when_adapter_is_available() {
        let mut renderers = BackendRenderers::new();

        let Ok(rendered) =
            renderers.render_frame(&RenderParams::default(), LoopTime::from_frame(0, 24), 8, 8)
        else {
            return;
        };

        assert_eq!(rendered.status.short_label(), "GPU");
        assert_eq!(rendered.frame.pixels.len(), 8 * 8 * 4);
    }
}
