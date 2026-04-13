struct Uniforms {
    screen_width: f32,
    screen_height: f32,
    cell_width: f32,
    cell_height: f32,
    atlas_width: f32,
    atlas_height: f32,
    content_origin_x: f32,
    content_origin_y: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) @interpolate(flat) flags: u32,
    @location(4) local_pos: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    // Per-instance attributes
    @location(0) col_row: vec2<u32>,
    @location(1) glyph_uv0: vec2<f32>,
    @location(2) glyph_uv1: vec2<f32>,
    @location(3) fg_color: vec4<f32>,
    @location(4) bg_color: vec4<f32>,
    @location(5) flags: u32,
    @location(6) glyph_offset: vec2<f32>,
) -> VertexOutput {
    // Generate quad vertices: two triangles
    var quad_x = array<f32, 6>(0.0, 1.0, 0.0, 0.0, 1.0, 1.0);
    var quad_y = array<f32, 6>(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);

    let qx = quad_x[vertex_index];
    let qy = quad_y[vertex_index];

    let is_wide = (flags & 2u) != 0u;
    var cell_w = u.cell_width;
    if is_wide {
        cell_w = u.cell_width * 2.0;
    }

    // Cell position in physical pixels
    let px = u.content_origin_x + f32(col_row.x) * u.cell_width + qx * cell_w;
    let py = u.content_origin_y + f32(col_row.y) * u.cell_height + qy * u.cell_height;

    // Convert to NDC
    let ndc_x = (px / u.screen_width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / u.screen_height) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = mix(glyph_uv0, glyph_uv1, vec2<f32>(qx, qy));
    out.fg_color = fg_color;
    out.bg_color = bg_color;
    out.flags = flags;
    out.local_pos = vec2<f32>(qx, qy);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let has_glyph = (in.flags & 1u) != 0u;
    let has_underline = (in.flags & 4u) != 0u;
    let has_strikethrough = (in.flags & 8u) != 0u;

    // Start with background
    var color = in.bg_color;

    // Composite glyph foreground using atlas alpha
    if has_glyph {
        let alpha = textureSample(atlas_texture, atlas_sampler, in.uv).r;
        color = mix(color, vec4<f32>(in.fg_color.rgb, 1.0), alpha);
    }

    // Underline: 1-2px line at bottom of cell
    if has_underline {
        let bottom_band = 1.0 - in.local_pos.y;
        if bottom_band < 0.08 {
            color = vec4<f32>(in.fg_color.rgb, 1.0);
        }
    }

    // Strikethrough: 1-2px line at middle of cell
    if has_strikethrough {
        let mid = abs(in.local_pos.y - 0.5);
        if mid < 0.04 {
            color = vec4<f32>(in.fg_color.rgb, 1.0);
        }
    }

    return color;
}
