use bytemuck::{Pod, Zeroable};

/// Per-cell instance data for GPU rendering.
/// Each visible terminal cell becomes one instance in the draw call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CellInstance {
    /// Grid column
    pub col: u32,
    /// Grid row
    pub row: u32,
    /// Atlas UV: left
    pub glyph_u0: f32,
    /// Atlas UV: top
    pub glyph_v0: f32,
    /// Atlas UV: right
    pub glyph_u1: f32,
    /// Atlas UV: bottom
    pub glyph_v1: f32,
    /// Foreground color RGBA (sRGB)
    pub fg_color: [u8; 4],
    /// Background color RGBA (sRGB)
    pub bg_color: [u8; 4],
    /// Bit flags:
    ///   bit 0: has_glyph (character is not space)
    ///   bit 1: wide (CJK double-width)
    ///   bit 2: underline
    ///   bit 3: strikethrough
    pub flags: u32,
    /// Horizontal offset within cell for glyph centering (physical pixels)
    pub glyph_offset_x: f32,
    /// Vertical offset within cell for glyph centering (physical pixels)
    pub glyph_offset_y: f32,
    pub _pad: u32,
}

impl CellInstance {
    pub const FLAG_HAS_GLYPH: u32 = 1;
    pub const FLAG_WIDE: u32 = 2;
    pub const FLAG_UNDERLINE: u32 = 4;
    pub const FLAG_STRIKETHROUGH: u32 = 8;

    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // col_row: vec2<u32> at location 0
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // glyph_uv0: vec2<f32> at location 1
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                // glyph_uv1: vec2<f32> at location 2
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 2,
                },
                // fg_color: vec4<u8norm> at location 3
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 24,
                    shader_location: 3,
                },
                // bg_color: vec4<u8norm> at location 4
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 28,
                    shader_location: 4,
                },
                // flags: u32 at location 5
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 32,
                    shader_location: 5,
                },
                // glyph_offset: vec2<f32> at location 6
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 36,
                    shader_location: 6,
                },
            ],
        }
    }
}

/// Uniform data passed to the shader each frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GridUniforms {
    /// Viewport (content_rect) width in physical pixels
    pub viewport_width: f32,
    /// Viewport (content_rect) height in physical pixels
    pub viewport_height: f32,
    /// Cell width in physical pixels
    pub cell_width: f32,
    /// Cell height in physical pixels
    pub cell_height: f32,
    /// Atlas texture width in pixels
    pub atlas_width: f32,
    /// Atlas texture height in pixels
    pub atlas_height: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}
