use std::collections::HashMap;
use super::font_backend::{FontBackend, GlyphRegion, AtlasGlyphKey, GLYPH_PADDING, INITIAL_ATLAS_SIZE, MAX_ATLAS_SIZE, create_gpu_resources, upload_bitmap, empty_glyph_region};

pub struct FontdueAtlas {
    font_regular: fontdue::Font,
    font_bold: Option<fontdue::Font>,
    fallback_fonts: Vec<fontdue::Font>,
    font_size_px: f32,
    font_weight: f32,
    bitmap: Vec<u8>,
    width: u32,
    height: u32,
    shelf_x: u32,
    shelf_y: u32,
    shelf_height: u32,
    cache: HashMap<AtlasGlyphKey, GlyphRegion>,
    dirty: bool,
    needs_rebind: bool,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    cached_ascent: f32,
    cached_descent: f32,
}

impl FontdueAtlas {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_data_regular: &[u8],
        font_data_bold: Option<&[u8]>,
        fallback_font_data: &[Vec<u8>],
        font_size_px: f32,
        font_weight: f32,
    ) -> Self {
        let settings = fontdue::FontSettings {
            ..Default::default()
        };
        let font_regular = fontdue::Font::from_bytes(font_data_regular, settings)
            .expect("failed to load regular font");
        let font_bold = font_data_bold.map(|data| {
            fontdue::Font::from_bytes(data, settings).expect("failed to load bold font")
        });
        let fallback_fonts: Vec<fontdue::Font> = fallback_font_data
            .iter()
            .filter_map(|data| fontdue::Font::from_bytes(data.as_slice(), settings).ok())
            .collect();

        let width = INITIAL_ATLAS_SIZE;
        let height = INITIAL_ATLAS_SIZE;
        let bitmap = vec![0u8; (width * height) as usize];

        let (texture, view, sampler) = create_gpu_resources(device, width, height);
        upload_bitmap(queue, &texture, &bitmap, width, height);

        let (cached_ascent, cached_descent) = Self::compute_metrics(&font_regular, font_size_px);

        let mut atlas = FontdueAtlas {
            font_regular,
            font_bold,
            fallback_fonts,
            font_size_px,
            font_weight,
            bitmap,
            width,
            height,
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
            cache: HashMap::with_capacity(256),
            dirty: false,
            needs_rebind: false,
            texture,
            view,
            sampler,
            cached_ascent,
            cached_descent,
        };

        atlas.prepopulate_ascii();
        atlas
    }

    fn compute_metrics(font: &fontdue::Font, font_size_px: f32) -> (f32, f32) {
        if let Some(lm) = font.horizontal_line_metrics(font_size_px) {
            (lm.ascent, lm.descent)
        } else {
            (font_size_px * 0.8, -(font_size_px * 0.2))
        }
    }

    fn prepopulate_ascii(&mut self) {
        for ch in ' '..='~' {
            self.get_or_rasterize(ch, false);
        }
        for ch in ' '..='~' {
            self.get_or_rasterize(ch, true);
        }
    }

    fn allocate_shelf(&mut self, w: u32, h: u32) -> bool {
        if self.shelf_x + w <= self.width && self.shelf_y + h.max(self.shelf_height) <= self.height
        {
            self.shelf_x += w;
            if h > self.shelf_height {
                self.shelf_height = h;
            }
            return true;
        }

        let new_shelf_y = self.shelf_y + self.shelf_height;
        if w <= self.width && new_shelf_y + h <= self.height {
            self.shelf_y = new_shelf_y;
            self.shelf_x = w;
            self.shelf_height = h;
            return true;
        }

        false
    }

    fn grow(&mut self) -> bool {
        let new_size = self.width * 2;
        if new_size > MAX_ATLAS_SIZE {
            return false;
        }

        let mut new_bitmap = vec![0u8; (new_size * new_size) as usize];
        for y in 0..self.height {
            let src_start = (y * self.width) as usize;
            let src_end = src_start + self.width as usize;
            let dst_start = (y * new_size) as usize;
            new_bitmap[dst_start..dst_start + self.width as usize]
                .copy_from_slice(&self.bitmap[src_start..src_end]);
        }

        self.bitmap = new_bitmap;
        let scale_x = self.width as f32 / new_size as f32;
        let scale_y = self.height as f32 / new_size as f32;
        for region in self.cache.values_mut() {
            region.u0 *= scale_x;
            region.u1 *= scale_x;
            region.v0 *= scale_y;
            region.v1 *= scale_y;
        }

        self.width = new_size;
        self.height = new_size;
        self.dirty = true;
        true
    }

    fn rasterize_and_place(
        &mut self,
        metrics: &fontdue::Metrics,
        glyph_bitmap: &[u8],
        bold: bool,
        key: AtlasGlyphKey,
    ) -> GlyphRegion {
        let glyph_w = metrics.width as u32;
        let glyph_h = metrics.height as u32;

        if glyph_w == 0 || glyph_h == 0 {
            let region = empty_glyph_region();
            self.cache.insert(key, region);
            return region;
        }

        let padded_w = glyph_w + GLYPH_PADDING * 2;
        let padded_h = glyph_h + GLYPH_PADDING * 2;

        if !self.allocate_shelf(padded_w, padded_h) {
            if !self.grow() {
                let region = empty_glyph_region();
                self.cache.insert(key, region);
                return region;
            }
            if !self.allocate_shelf(padded_w, padded_h) {
                let region = empty_glyph_region();
                self.cache.insert(key, region);
                return region;
            }
        }

        let atlas_x = self.shelf_x - padded_w;
        let atlas_y = self.shelf_y;
        let bx = atlas_x + GLYPH_PADDING;
        let by = atlas_y + GLYPH_PADDING;

        let weight_boost = if bold { 1.0 } else { self.font_weight };

        // Copy bitmap row by row into atlas
        for gy in 0..glyph_h {
            for gx in 0..glyph_w {
                let src_idx = (gy * glyph_w + gx) as usize;
                let dst_x = bx + gx;
                let dst_y = by + gy;
                if dst_x < self.width && dst_y < self.height {
                    let alpha = glyph_bitmap[src_idx] as f32 / 255.0;
                    let boosted = (alpha * weight_boost).min(1.0);
                    self.bitmap[(dst_y * self.width + dst_x) as usize] = (boosted * 255.0 + 0.5) as u8;
                }
            }
        }

        self.dirty = true;

        // Convert fontdue coordinates to match ab_glyph convention:
        // fontdue ymin: bottom of glyph relative to baseline (positive = above baseline)
        // ab_glyph bearing_y: top of glyph relative to top of cell (ascent-based)
        // bearing_y in screen coords = ascent - (ymin + height)
        let bearing_x = metrics.xmin as f32;
        let bearing_y = self.cached_ascent - (metrics.ymin as f32 + metrics.height as f32);

        let region = GlyphRegion {
            u0: bx as f32 / self.width as f32,
            v0: by as f32 / self.height as f32,
            u1: (bx + glyph_w) as f32 / self.width as f32,
            v1: (by + glyph_h) as f32 / self.height as f32,
            width_px: glyph_w as f32,
            height_px: glyph_h as f32,
            bearing_x,
            bearing_y,
        };
        self.cache.insert(key, region);
        region
    }
}

impl FontBackend for FontdueAtlas {
    fn get_or_rasterize(&mut self, ch: char, bold: bool) -> GlyphRegion {
        let key = AtlasGlyphKey { ch, bold };
        if let Some(&region) = self.cache.get(&key) {
            return region;
        }

        // Try primary font first (bold or regular)
        let font = if bold {
            self.font_bold.as_ref().unwrap_or(&self.font_regular)
        } else {
            &self.font_regular
        };

        // Check if glyph exists in primary font
        let glyph_index = font.lookup_glyph_index(ch);
        let has_glyph = glyph_index != 0;

        // If no glyph in primary font (or we want fallback for missing chars), try fallback fonts
        if !has_glyph && ch != ' ' && !ch.is_control() {
            for fb in &self.fallback_fonts {
                let fb_glyph_index = fb.lookup_glyph_index(ch);
                if fb_glyph_index != 0 {
                    let (fb_metrics, fb_bitmap) = fb.rasterize(ch, self.font_size_px);
                    return self.rasterize_and_place(&fb_metrics, &fb_bitmap, bold, key);
                }
            }
            // Glyph not found in any font, use .notdef
            let (metrics, glyph_bitmap) = font.rasterize(ch, self.font_size_px);
            return self.rasterize_and_place(&metrics, &glyph_bitmap, bold, key);
        }

        // Glyph exists (or is space/control), rasterize from primary font
        let (metrics, glyph_bitmap) = font.rasterize(ch, self.font_size_px);

        if glyph_bitmap.is_empty() || metrics.width == 0 || metrics.height == 0 {
            // Space, control chars, etc. — just return advance width
            let region = GlyphRegion {
                u0: 0.0,
                v0: 0.0,
                u1: 0.0,
                v1: 0.0,
                width_px: metrics.advance_width,
                height_px: 0.0,
                bearing_x: 0.0,
                bearing_y: 0.0,
            };
            self.cache.insert(key, region);
            return region;
        }

        self.rasterize_and_place(&metrics, &glyph_bitmap, bold, key)
    }

    fn reset(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.cache.clear();
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_height = 0;

        let w = INITIAL_ATLAS_SIZE;
        let h = INITIAL_ATLAS_SIZE;
        self.bitmap = vec![0u8; (w * h) as usize];
        self.width = w;
        self.height = h;

        let (texture, view, sampler) = create_gpu_resources(device, w, h);
        self.texture = texture;
        self.view = view;
        self.sampler = sampler;

        let (asc, desc) = Self::compute_metrics(&self.font_regular, self.font_size_px);
        self.cached_ascent = asc;
        self.cached_descent = desc;

        self.prepopulate_ascii();
        self.ensure_uploaded(device, queue);
        self.needs_rebind = true;
    }

    fn font_metrics(&self) -> (f32, f32, f32) {
        let m = self.font_regular.metrics('0', self.font_size_px);
        (self.cached_ascent, self.cached_descent, m.advance_width)
    }

    fn ensure_uploaded(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.dirty {
            return;
        }

        let tex_size = self.texture.size();
        if tex_size.width != self.width || tex_size.height != self.height {
            let (texture, view, sampler) = create_gpu_resources(device, self.width, self.height);
            self.texture = texture;
            self.view = view;
            self.sampler = sampler;
        }

        upload_bitmap(queue, &self.texture, &self.bitmap, self.width, self.height);
        self.dirty = false;
    }

    fn backend_name(&self) -> &'static str {
        "fontdue"
    }

    fn gpu_resources(&self) -> (&wgpu::TextureView, &wgpu::Sampler) {
        (&self.view, &self.sampler)
    }

    fn atlas_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn take_needs_rebind(&mut self) -> bool {
        let v = self.needs_rebind;
        self.needs_rebind = false;
        v
    }
}
