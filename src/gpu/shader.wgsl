struct Uniforms {
    viewport_width: f32,
    viewport_height: f32,
    cell_width: f32,
    cell_height: f32,
    atlas_width: f32,
    atlas_height: f32,
    render_phase: f32,
    sharpness: f32,  // gamma exponent for sharpening (0.5-1.0, lower=sharper)
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

// Gamma correction helpers for proper subpixel blending
fn srgb_to_linear(srgb: f32) -> f32 {
    if srgb <= 0.04045 {
        return srgb / 12.92;
    }
    return pow((srgb + 0.055) / 1.055, 2.4);
}

fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        return linear * 12.92;
    }
    return 1.055 * pow(linear, 1.0 / 2.4) - 0.055;
}

fn srgb_to_linear_vec3(srgb: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_to_linear(srgb.r),
        srgb_to_linear(srgb.g),
        srgb_to_linear(srgb.b)
    );
}

fn linear_to_srgb_vec3(linear: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        linear_to_srgb(linear.r),
        linear_to_srgb(linear.g),
        linear_to_srgb(linear.b)
    );
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

    // Composite glyph foreground using atlas alpha with subpixel RGB rendering
    if has_glyph {
        // Glyph size in physical pixels (derived from atlas UV extent)
        let glyph_size = (in.glyph_uv1 - in.glyph_uv0) * vec2<f32>(u.atlas_width, u.atlas_height);

        // Compute atlas UV from pixel position relative to glyph origin
        let rel = in.cell_px_pos - in.glyph_offset;
        let t = clamp(rel / max(glyph_size, vec2<f32>(1.0, 1.0)), vec2<f32>(0.0), vec2<f32>(1.0));
        let uv = in.glyph_uv0 + t * (in.glyph_uv1 - in.glyph_uv0);

        // Optimized subpixel rendering: reduce color fringing by using smaller offsets
        // Use 1/6 pixel offset instead of 1/3 to reduce color separation
        let subpixel_step = 1.0 / (6.0 * u.atlas_width);

        let alpha_r = textureSample(atlas_texture, atlas_sampler, uv - vec2(subpixel_step, 0.0)).r;
        let alpha_g = textureSample(atlas_texture, atlas_sampler, uv).r;
        let alpha_b = textureSample(atlas_texture, atlas_sampler, uv + vec2(subpixel_step, 0.0)).r;

        // Apply adjustable sharpening (controlled by uniform)
        let sharp_r = pow(alpha_r, u.sharpness);
        let sharp_g = pow(alpha_g, u.sharpness);
        let sharp_b = pow(alpha_b, u.sharpness);

        // Only apply glyph where pixel falls within the glyph area
        let in_bounds = step(0.0, rel.x) * step(0.0, rel.y)
                      * step(rel.x, glyph_size.x) * step(rel.y, glyph_size.y);

        let sr = sharp_r * in_bounds;
        let sg = sharp_g * in_bounds;
        let sb = sharp_b * in_bounds;

        if max(sr, max(sg, sb)) > 0.001 {
            // Convert to linear space for proper blending
            let fg_linear = srgb_to_linear_vec3(in.fg_color.rgb);
            let bg_linear = srgb_to_linear_vec3(in.bg_color.rgb);

            // Reduce subpixel effect by averaging with grayscale alpha
            let avg_alpha = (sr + sg + sb) / 3.0;
            let blend_factor = 0.3; // 30% subpixel, 70% grayscale to reduce fringing

            let final_r = mix(avg_alpha, sr, blend_factor);
            let final_g = mix(avg_alpha, sg, blend_factor);
            let final_b = mix(avg_alpha, sb, blend_factor);

            // Blend in linear space with adjusted per-channel alphas
            let blended_linear = vec3<f32>(
                mix(bg_linear.r, fg_linear.r, final_r),
                mix(bg_linear.g, fg_linear.g, final_g),
                mix(bg_linear.b, fg_linear.b, final_b)
            );

            // Convert back to sRGB for display
            color = vec4<f32>(linear_to_srgb_vec3(blended_linear), 1.0);
        }
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
