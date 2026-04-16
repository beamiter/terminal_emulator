use super::atlas::GlyphAtlas;
use super::instance::{CellInstance, GridUniforms};
use super::pipeline::GridPipeline;
use egui_wgpu::CallbackResources;

/// GPU resources stored in egui_wgpu's CallbackResources (TypeMap).
pub struct GpuResources {
    pub atlas: GlyphAtlas,
    pub pipeline: GridPipeline,
    /// Track atlas texture generation so we know when to rebuild the bind group
    atlas_gen: u64,
}

impl GpuResources {
    pub fn new(atlas: GlyphAtlas, pipeline: GridPipeline) -> Self {
        GpuResources {
            atlas,
            pipeline,
            atlas_gen: 0,
        }
    }
}

/// Per-frame callback carrying the instance data to render.
pub struct GridRenderCallback {
    pub instances: Vec<CellInstance>,
    pub uniforms: GridUniforms,
    pub instance_count: u32,
}

impl egui_wgpu::CallbackTrait for GridRenderCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let res = callback_resources.get_mut::<GpuResources>().unwrap();

        // Upload atlas if dirty (rasterized new glyphs this frame)
        let old_tex_size = (res.atlas.atlas_width(), res.atlas.atlas_height());
        res.atlas.ensure_uploaded(device, queue);
        let new_tex_size = (res.atlas.atlas_width(), res.atlas.atlas_height());

        // Rebuild bind group if atlas texture was recreated (resized or reset)
        if old_tex_size != new_tex_size || res.atlas.take_needs_rebind() {
            res.atlas_gen += 1;
            res.pipeline
                .rebuild_bind_group(device, &res.atlas.view, &res.atlas.sampler);
        }

        // Upload uniforms and instances
        res.pipeline.update_uniforms(queue, &self.uniforms);
        res.pipeline
            .update_instances(device, queue, &self.instances);

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &CallbackResources,
    ) {
        if self.instance_count == 0 {
            return;
        }
        let res = callback_resources.get::<GpuResources>().unwrap();
        render_pass.set_pipeline(res.pipeline.pipeline());
        render_pass.set_bind_group(
            0,
            res.pipeline
                .bind_group_for_phase(self.uniforms.render_phase),
            &[],
        );
        render_pass.set_vertex_buffer(0, res.pipeline.instance_buffer().slice(..));
        render_pass.draw(0..6, 0..self.instance_count);
    }
}
