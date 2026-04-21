# egui vs jterm2 Font Rendering: Detailed Comparison

## Architecture Overview

| Aspect | egui (epaint) | jterm2 |
|--------|---------------|--------|
| **Atlas Format** | RGBA8 (TextureFormat::Rgba8Unorm) | R8 grayscale (TextureFormat::R8Unorm) |
| **Initial Atlas Size** | 32px tall × 2048px wide (grows by doubling height) | 1024×1024 (both dims, doubles to 4096×4096) |
| **Packing Strategy** | Shelf packer with 1px padding | Shelf packer with 2px padding |
| **Cache Invalidation** | Full rebuild at 80% full | Per-row dirty tracking + incremental growth |
| **Rasterizer Backend** | skrifa (outline) + vello_cpu (coverage) | fontdue OR ab_glyph (both coverage-based) |
| **Subpixel Bins** | 4 (0.00, 0.25, 0.50, 0.75px) | 4 (0.00, 0.25, 0.50, 0.75px) |
| **CJK Special Case** | Skip subpixel binning (always bin 0) | No special case documented |

---

## 1. Glyph Rasterization

### egui
- **Font parsing**: skrifa (Google's Rust font library)
- **Hinting**: Optional, uses skrifa's HintingInstance with SmoothMode::Normal
- **Coverage rendering**: vello_cpu fills a BezPath into a Pixmap
- **Subpixel application**: Applied as an x-offset in the outline (VelloPen translates x-coords)
- **Artifact handling**: CJK glyphs always use bin 0 to avoid 4× atlas copies
- **Output**: Single alpha channel → Color32 via `AlphaFromCoverage` (default: `TwoCoverageMinusCoverageSq` gamma)

### jterm2
- **Font parsing**: fontdue or ab_glyph crate
- **Hinting**: None (fontdue's rasterizer has no hinting pipeline)
- **Coverage rendering**: Direct bitmap rasterization (fontdue) or outline→coverage (ab_glyph)
- **Subpixel application**: Embedded into `bearing_x` offset during rasterization
- **Artifact handling**: No documented special case; all 4 bins always cached per glyph
- **Weight boost**: User-configurable `font_weight` multiplier applied to alpha during storage
- **Output**: Single alpha channel (R8Unorm), stored directly as-is

**Key Difference**: egui rasterizes at full precision then applies subpixel at outline-level; jterm2 applies subpixel as a position offset after rasterization.

---

## 2. Atlas Texture Format & Storage

### egui
```
Color32 (sRGB with premultiplied alpha)
per-pixel: R, G, B stored as gamma-encoded + alpha
Stored in: RGBA8Unorm
```

**Coverage-to-Color conversion** (`AlphaFromCoverage`):
- Linear: `alpha = coverage`
- Gamma: `alpha = coverage^g` (e.g., g=0.5)
- **Default TwoCoverageMinusCoverageSq**: `alpha = 2c - c²` (gamma-aware blending)

**Rationale**: Different color spaces impact text rendering at small sizes. The "two coverage minus coverage squared" function provides a middle ground for dark text on light backgrounds (light mode) AND light text on dark backgrounds (dark mode).

### jterm2
```
R8 grayscale (raw coverage alpha)
per-pixel: stored as-is, no color transformation
Format: R8Unorm
```

**No coverage transformation** — the raw alpha is stored directly.

**Rationale**: Simplicity; color blending happens entirely in the fragment shader (linear-space per-channel blending of foreground/background).

**Trade-off**:
- **egui**: Bakes gamma choices at rasterization time; simpler shader math but less flexible.
- **jterm2**: Defers all color math to the shader; more shader complexity but full control per-pixel per-channel.

---

## 3. Subpixel Rendering Strategy

### egui
**Horizontal subpixel binning (4 levels)**:
- Outline x-coordinates are shifted by `bin.as_float()` (0.0, 0.25, 0.5, 0.75) before rasterization.
- Each glyph is rasterized at 4 fractional phases.
- Cache key: `(glyph_id, pixels_per_point, px_scale_factor, bin)`.
- **Result in atlas**: 4 separate bitmaps per glyph (unless CJK → always bin 0).

**Fragment shader**: Linear texture sampling at normalized UV coordinates. No explicit subpixel logic in the shader—the 4 different rasterizations effectively give 4×sub-pixel precision for horizontal placement.

### jterm2
**Horizontal subpixel binning (4 levels)**:
- `quantize_subpixel(cell_x.fract())` bins into 0, 1, 2, or 3 based on fractional pixel position.
- Each bin value is converted to a pixel offset: `bin * 0.25px`.
- Cache key: `(char, bold, subpixel_bin)`.
- **Result in atlas**: 4 separate rasterizations per glyph (same as egui).

**Fragment shader** (`fs_main`):
```wgsl
// Sample at 3 positions: left, center, right
let uv_l = uv - vec2(1.0 / (6.0 * atlas_width), 0.0);
let uv_c = uv;
let uv_r = uv + vec2(1.0 / (6.0 * atlas_width), 0.0);

let alpha_r = textureSample(atlas, sampler, uv_l).r;
let alpha_g = textureSample(atlas, sampler, uv_c).r;
let alpha_b = textureSample(atlas, sampler, uv_r).r;

// Sharpening: raise to power
let sharp_r = pow(alpha_r, sharpness);
let sharp_g = pow(alpha_g, sharpness);
let sharp_b = pow(alpha_b, sharpness);

// Blend: 30% subpixel, 70% grayscale
let avg = (alpha_r + alpha_g + alpha_b) / 3.0;
let blended = mix(avg, vec3(sharp_r, sharp_g, sharp_b), 0.3);
```

**Unique features**:
- **ClearType-like RGB sampling**: Samples ±1/6 pixel (not ±1/3 like true ClearType) to reduce color fringing.
- **Sharpening**: Raises alpha to a power; `sharpness < 1.0` increases contrast.
- **Hybrid blending**: 30% subpixel RGB + 70% grayscale average for a balanced appearance.
- **Linear-space blending**: Applies full sRGB encoding/decoding per-channel for perceptually correct blending.

**Trade-off**:
- **egui**: Passive subpixel (4 rasterizations in atlas, linear sampling in shader).
- **jterm2**: Active subpixel in shader + sharpening + blending logic. More GPU cost but visual quality tuning via `sharpness` uniform.

---

## 4. Vertex Format & Mesh Layout

### egui
```rust
Vertex {
    pos: Pos2,       // 2×f32 — logical pixel coordinates (points)
    uv: Pos2,        // 2×f32 — normalized [0, 1] texture coordinates
    color: Color32,  // 4 bytes — sRGB RGBA
}
// 20 bytes per vertex
```

**Mesh generation**:
- `add_rect_with_uv(rect, uv, color)` emits 4 vertices + 6 indices (two triangles).
- Indices are `u32` (separate index buffer).
- Italics: horizontal shear applied to top two vertices.

### jterm2
```rust
CellInstance {
    col, row: u32 × 2                // cell grid position
    glyph_u0/v0/u1/v1: f32 × 4      // atlas UV bounds (not normalized; normalized in shader)
    fg_color, bg_color: [u8; 4] × 2 // sRGB foreground + background
    flags: u32                        // HAS_GLYPH, WIDE, UNDERLINE, STRIKETHROUGH
    glyph_offset_x/y: f32 × 2        // pixel offsets within cell
    _pad: u32
}
// 48 bytes per instance
```

**Mesh generation**:
- Instanced rendering: 1 instance = 1 cell.
- Vertex shader generates 6 vertices (2 triangles) from hardcoded `quad_x[6]`, `quad_y[6]` arrays.
- No separate index buffer; `draw(0..6, 0..instance_count)`.
- Two passes: background (`render_phase=0`) fills cell with `bg_color`; foreground (`render_phase=1`) draws glyph + decorations.

**Trade-off**:
- **egui**: Traditional indexed mesh; flexible for arbitrary geometry (rotated text, italics).
- **jterm2**: Instanced rendering; rigid but efficient for the grid-layout use case (no repeated data per instance).

---

## 5. Texture Upload & Dirty Tracking

### egui
- **Delta tracking**: `TextureAtlas::take_delta()` returns `Option<ImageDelta>`.
  - Full delta (entire atlas) on resize or init.
  - Partial delta (dirty rectangle) on incremental glyph adds.
- **Upload**: `queue.write_texture` with the delta region.
- **Cache invalidation**: Full atlas rebuild when 80% full (`Fonts::begin_pass`).

### jterm2
- **Dirty flag**: `FontBackend::dirty` boolean (no sub-rectangle tracking).
- **Upload**: Full atlas via `queue.write_texture` if `dirty == true`.
- **Caching**: HashMap lookup with `(char, bold, subpixel_bin)` key.
- **Growth**: On shelf-packing failure, atlas dimensions double; existing bitmap is copied to upper-left; old UV coordinates remain valid (halved).
- **CPU bitmap**: Stored as a `Vec<u8>` and reallocated on each growth.

**Efficiency trade-offs**:
- **egui**: Partial delta uploads minimize bandwidth; full rebuild at 80% limits fragmentation.
- **jterm2**: Full uploads are simpler but potentially wasteful; relies on growth strategy (exponential → ~1-2 uploads/session) to amortize the cost.

---

## 6. Fragment Shader Rendering

### egui (`egui.wgsl`)
```wgsl
fn fs_main_linear_framebuffer(in: VertexOutput) -> vec4<f32> {
    let tex_gamma = textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
    var out_color_gamma = in.color * tex_gamma;           // multiply in gamma space
    let out_color_linear = linear_from_gamma_rgb(out_color_gamma.rgb);
    return vec4<f32>(out_color_linear, out_color_gamma.a);
}
```

**Key design**: Multiplication happens in gamma space because "that's the only way to get text to look right."

- Vertex color (gamma-encoded sRGB) × atlas color (gamma-encoded) in gamma space.
- Result converted to linear for sRGB framebuffer output.
- Alpha channel remains in gamma space.

### jterm2 (`shader.wgsl`)
```wgsl
fn fs_main() -> @location(0) vec4<f32> {
    // Background pass
    if render_phase < 0.5 {
        return bg_color_linear;
    }
    
    // Foreground pass
    let uv = glyph_uv_from_cell_pos(...);
    
    // ClearType-like 3-sample RGB
    let alpha_r = textureSample(atlas, sampler, uv - vec2(1.0 / (6.0 * atlas_width), 0.0)).r;
    let alpha_g = textureSample(atlas, sampler, uv).r;
    let alpha_b = textureSample(atlas, sampler, uv + vec2(1.0 / (6.0 * atlas_width), 0.0)).r;
    
    let avg = (alpha_r + alpha_g + alpha_b) / 3.0;
    let sharp_r = pow(alpha_r, sharpness);
    let sharp_g = pow(alpha_g, sharpness);
    let sharp_b = pow(alpha_b, sharpness);
    let blended = mix(avg, vec3(sharp_r, sharp_g, sharp_b), 0.3);
    
    // Per-channel linear blending
    let fg_linear = srgb_to_linear(fg_color);
    let bg_linear = srgb_to_linear(bg_color);
    var out_linear = mix(bg_linear, fg_linear, blended);
    
    // Underline / strikethrough
    if should_draw_underline(...) {
        out_linear = mix(out_linear, srgb_to_linear(fg_color), 1.0);
    }
    
    return linear_to_srgb(out_linear);
}
```

**Key differences**:
- **Per-channel blending**: Each RGB channel has its own alpha from the 3-sample subpixel RGB, enabling ClearType-like edge sharpening.
- **Sharpening**: Optional power-based contrast enhancement.
- **Hybrid approach**: 30% subpixel RGB + 70% grayscale to balance sharpness vs. color fringing.
- **All linear**: Final blending happens entirely in linear space.

---

## 7. Color Space & Blending

### egui
| Stage | Space | Notes |
|-------|-------|-------|
| Rasterization | Gamma | Alpha baked via `AlphaFromCoverage` |
| Vertex color | Gamma | User-provided sRGB color |
| Atlas sampling | Gamma | Multiply in gamma space |
| Fragment output | Linear | Convert RGB to linear; keep alpha in gamma |

**Rationale**: Gamma-space multiply is claimed to "look right" for text. The final linear conversion is for framebuffer compatibility.

### jterm2
| Stage | Space | Notes |
|-------|-------|-------|
| Rasterization | Grayscale | Raw alpha, no color transform |
| Vertex colors | sRGB | Packed as [u8; 4] per instance |
| Atlas sampling | Grayscale + ClearType RGB | 3-tap subpixel sampling |
| Per-channel blend | Linear | Each RGB channel uses its own alpha |
| Fragment output | sRGB | Convert linear to sRGB |

**Rationale**: Deferred color blending allows per-channel control via subpixel RGB alphas.

---

## 8. Two-Pass Rendering

### egui
**Single pass**: All primitives (backgrounds, text, UI shapes) rendered in one pass.
- Clipped via scissors and `clip_rect` in the fragment shader.
- Depth test: disabled (no Z ordering beyond paint order).

### jterm2
**Two-pass rendering**:
1. **Background pass** (`render_phase=0.0`): Fills cell background with `bg_color`.
2. **Foreground pass** (`render_phase=1.0`): Draws glyph texture + underline/strikethrough with `fg_color`.

**Rationale**: Separates cell fills from glyph rendering, allowing independent visual stacking without geometry duplication.

---

## Summary of Key Differences

| Feature | egui | jterm2 | Implication |
|---------|------|--------|-------------|
| **Atlas texture format** | RGBA8 | R8 | jterm2: simpler but requires shader blending logic |
| **Color math baked** | Yes (AlphaFromCoverage) | No (shader-deferred) | egui: simpler shader; jterm2: more flexibility |
| **Subpixel in rasterizer** | Yes (outline offset) | Partial (position offset) | egui: higher precision; jterm2 may have slight smear |
| **Shader subpixel logic** | Passive (linear sampling) | Active (3-tap RGB) | jterm2: more visual control (sharpness tuning) |
| **Blending space** | Gamma × gamma | Linear per-channel | egui: traditional; jterm2: subpixel-aware |
| **Atlas growth strategy** | Size-triggered rebuild | Exponential doubling | egui: preemptive; jterm2: on-demand |
| **Rendering strategy** | Single pass | Two pass | jterm2: grid-specific optimization |
| **Mesh geometry** | Indexed triangles | Instanced quads | jterm2: efficient for uniform cells |

---

## Alignment Recommendations

### To adopt egui's approach:
1. **Switch atlas to RGBA8** and bake color via `AlphaFromCoverage`.
   - Pro: Simpler shader, proven by egui.
   - Con: Lose per-channel subpixel control; need to pick a single gamma function.

2. **Apply subpixel at outline-rasterization time** (requires skrifa + vello_cpu).
   - Pro: Higher precision rasterization.
   - Con: Must replace fontdue/ab_glyph.

3. **Simplify shader** to `gamma_color * gamma_atlas` multiply.
   - Pro: Fewer GPU cycles, easier to debug.
   - Con: Lose sharpness tuning and ClearType RGB benefits.

### To enhance jterm2 toward egui:
1. **Keep R8 atlas but improve coverage-to-alpha** by implementing `AlphaFromCoverage`.
   - Apply a gamma function (e.g., TwoCoverageMinusCoverageSq) at rasterization.
   - Might reduce sharpness tuning flexibility but improve visual consistency.

2. **Add hinting support** (skrifa HintingInstance) to fontdue or ab_glyph.
   - egui's hinting improves small-size rendering quality.

3. **Implement partial-delta texture uploads** (track dirty rectangles).
   - Would reduce per-frame upload bandwidth for sparse updates.

4. **Consider keeping the 3-tap subpixel RGB** as an optional mode.
   - egui doesn't do per-channel subpixel; jterm2's approach is a differentiator.
   - Could add a `use_rgb_subpixel` flag in config to toggle.

---

## Code Reference Locations

**egui**:
- Font rasterization: `~/.cargo/registry/src/.../epaint-0.34.1/src/text/font.rs`
- Atlas management: `~/.cargo/registry/src/.../epaint-0.34.1/src/texture_atlas.rs`
- Tessellation: `~/.cargo/registry/src/.../epaint-0.34.1/src/tessellator.rs`
- Shader: `~/.cargo/registry/src/.../egui-wgpu-0.34.1/src/egui.wgsl`

**jterm2**:
- Font backend trait: `src/gpu/font_backend.rs`
- Fontdue backend: `src/gpu/fontdue_backend.rs`
- Ab_glyph backend: `src/gpu/ab_glyph_backend.rs`
- Vertex/instance format: `src/gpu/instance.rs`
- Shader: `src/gpu/shader.wgsl`
- Layout & instance building: `src/ui.rs`
- Callback integration: `src/gpu/callback.rs`
