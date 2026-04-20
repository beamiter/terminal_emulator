struct Uniforms {
    viewport_width: f32,
    viewport_height: f32,
    cell_width: f32,
    cell_height: f32,
    atlas_width: f32,
    atlas_height: f32,
    render_phase: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) glyph_uv0: vec2<f32>,
    @location(1) @interpolate(flat) glyph_uv1: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
    @location(4) @interpolate(flat) flags: u32,
    @location(5) cell_local_pos: vec2<f32>,
    @location(6) @interpolate(flat) glyph_offset: vec2<f32>,
    @location(7) cell_px_pos: vec2<f32>,
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

    let has_glyph = (flags & 1u) != 0u;
    let glyph_size = (glyph_uv1 - glyph_uv0) * vec2<f32>(u.atlas_width, u.atlas_height);
    let foreground_pass = u.render_phase >= 0.5;

    var quad_min = vec2<f32>(0.0, 0.0);
    var quad_max = vec2<f32>(cell_w, u.cell_height);
    if foreground_pass && has_glyph {
        quad_min = min(quad_min, glyph_offset);
        quad_max = max(quad_max, glyph_offset + glyph_size);
    }

    let px_in_cell = vec2<f32>(
        mix(quad_min.x, quad_max.x, qx),
        mix(quad_min.y, quad_max.y, qy),
    );

    // Cell position in physical pixels relative to viewport origin
    let px = f32(col_row.x) * u.cell_width + px_in_cell.x;
    let py = f32(col_row.y) * u.cell_height + px_in_cell.y;

    // Convert to NDC (viewport is set to content_rect by egui-wgpu)
    let ndc_x = (px / u.viewport_width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / u.viewport_height) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.glyph_uv0 = glyph_uv0;
    out.glyph_uv1 = glyph_uv1;
    out.fg_color = fg_color;
    out.bg_color = bg_color;
    out.flags = flags;
    out.cell_local_pos = px_in_cell / vec2<f32>(cell_w, u.cell_height);
    out.glyph_offset = glyph_offset;
    out.cell_px_pos = px_in_cell;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let has_glyph = (in.flags & 1u) != 0u;
    let has_underline = (in.flags & 4u) != 0u;
    let has_strikethrough = (in.flags & 8u) != 0u;
    let background_pass = u.render_phase < 0.5;

    let is_wide = (in.flags & 2u) != 0u;
    var cell_w = u.cell_width;
    if is_wide {
        cell_w = u.cell_width * 2.0;
    }

    // Background pass draws only cell fills. Foreground pass starts transparent.
    var color = select(vec4<f32>(0.0, 0.0, 0.0, 0.0), in.bg_color, background_pass);

    if background_pass {
        return color;
    }

    // Composite glyph foreground using atlas alpha
    if has_glyph {
        // Glyph size in physical pixels (derived from atlas UV extent)
        let glyph_size = (in.glyph_uv1 - in.glyph_uv0) * vec2<f32>(u.atlas_width, u.atlas_height);

        // Compute atlas UV from pixel position relative to glyph origin
        let rel = in.cell_px_pos - in.glyph_offset;
        let t = clamp(rel / max(glyph_size, vec2<f32>(1.0, 1.0)), vec2<f32>(0.0), vec2<f32>(1.0));
        let uv = in.glyph_uv0 + t * (in.glyph_uv1 - in.glyph_uv0);

        var alpha = textureSample(atlas_texture, atlas_sampler, uv).r;

        // More aggressive alpha sharpening for crisp text rendering
        // Using pow < 1.0 boosts mid-range values, making edges sharper
        // 0.75 exponent provides much better clarity than the previous 0.92
        alpha = pow(alpha, 0.75);

        // Optional: clip very faint edges (< 2%) to pure transparent
        // This helps prevent ghosting artifacts
        if (alpha < 0.02) {
            alpha = 0.0;
        }

        // Only apply glyph where pixel falls within the glyph area
        let in_bounds = step(0.0, rel.x) * step(0.0, rel.y)
                      * step(rel.x, glyph_size.x) * step(rel.y, glyph_size.y);
        color = mix(color, vec4<f32>(in.fg_color.rgb, 1.0), alpha * in_bounds);
    }

    // Underline: 1-2px line at bottom of cell
    let within_cell = in.cell_local_pos.x >= 0.0 && in.cell_local_pos.x <= 1.0
        && in.cell_local_pos.y >= 0.0 && in.cell_local_pos.y <= 1.0;

    if has_underline && within_cell {
        let bottom_band = 1.0 - in.cell_local_pos.y;
        if bottom_band < 0.08 {
            color = vec4<f32>(in.fg_color.rgb, 1.0);
        }
    }

    // Strikethrough: 1-2px line at middle of cell
    if has_strikethrough && within_cell {
        let mid = abs(in.cell_local_pos.y - 0.5);
        if mid < 0.04 {
            color = vec4<f32>(in.fg_color.rgb, 1.0);
        }
    }

    return color;
}
