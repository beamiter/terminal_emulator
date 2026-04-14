use ab_glyph::{Font, FontVec, GlyphId, PxScale, ScaleFont, point};
use std::collections::HashMap;

/// UV region of a rasterized glyph inside the atlas texture.
#[derive(Clone, Copy, Debug)]
pub struct GlyphRegion {
    /// UV coordinates (0.0..1.0) in the atlas texture
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    /// Glyph pixel dimensions (for centering within cell)
    pub width_px: f32,
    pub height_px: f32,
    /// Bearing offsets from glyph origin
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// Key for the glyph cache: character + style.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AtlasGlyphKey {
    pub ch: char,
    pub bold: bool,
}

/// CPU-side glyph atlas with shelf-packing, uploaded to a GPU R8Unorm texture.
pub struct GlyphAtlas {
    font_regular: FontVec,
    font_bold: Option<FontVec>,
    font_size_px: f32,
    /// CPU-side alpha bitmap
    bitmap: Vec<u8>,
    width: u32,
    height: u32,
    /// Shelf packer state
    shelf_x: u32,
    shelf_y: u32,
    shelf_height: u32,
    /// Glyph cache
    cache: HashMap<AtlasGlyphKey, GlyphRegion>,
    /// Whether the CPU bitmap has been modified since last GPU upload
    dirty: bool,
    /// GPU resources
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl GlyphAtlas {
    const INITIAL_SIZE: u32 = 1024;
    const MAX_SIZE: u32 = 4096;
    const GLYPH_PADDING: u32 = 1;

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_data_regular: &[u8],
        font_data_bold: Option<&[u8]>,
        font_size_px: f32,
    ) -> Self {
        let font_regular =
            FontVec::try_from_vec(font_data_regular.to_vec()).expect("failed to load regular font");
        let font_bold = font_data_bold
            .map(|data| FontVec::try_from_vec(data.to_vec()).expect("failed to load bold font"));

        let width = Self::INITIAL_SIZE;
        let height = Self::INITIAL_SIZE;
        let bitmap = vec![0u8; (width * height) as usize];

        let (texture, view, sampler) = Self::create_gpu_resources(device, width, height);

        // Upload initial empty texture
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let mut atlas = GlyphAtlas {
            font_regular,
            font_bold,
            font_size_px,
            bitmap,
            width,
            height,
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
            cache: HashMap::with_capacity(256),
            dirty: false,
            texture,
            view,
            sampler,
        };

        // Pre-populate ASCII
        atlas.prepopulate_ascii();

        atlas
    }

    fn create_gpu_resources(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_atlas_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        (texture, view, sampler)
    }

    fn prepopulate_ascii(&mut self) {
        for ch in ' '..='~' {
            self.get_or_rasterize(ch, false);
        }
        // Also populate bold ASCII
        for ch in ' '..='~' {
            self.get_or_rasterize(ch, true);
        }
    }

    /// Get the glyph region for a character, rasterizing it on cache miss.
    pub fn get_or_rasterize(&mut self, ch: char, bold: bool) -> GlyphRegion {
        let key = AtlasGlyphKey { ch, bold };
        if let Some(&region) = self.cache.get(&key) {
            return region;
        }

        let font = if bold {
            self.font_bold.as_ref().unwrap_or(&self.font_regular)
        } else {
            &self.font_regular
        };

        let scale = PxScale::from(self.font_size_px);
        let scaled_font = font.as_scaled(scale);

        let glyph_id = font.glyph_id(ch);
        // Fallback: if glyph not found, try regular font for bold, or use tofu
        let (glyph_id, used_font): (GlyphId, &FontVec) = if glyph_id.0 == 0 && bold {
            let fallback_id = self.font_regular.glyph_id(ch);
            (fallback_id, &self.font_regular)
        } else {
            (glyph_id, font)
        };

        let scaled_used = used_font.as_scaled(scale);
        let glyph = glyph_id.with_scale_and_position(scale, point(0.0, scaled_used.ascent()));

        if let Some(outlined) = used_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let glyph_w = (bounds.max.x - bounds.min.x).ceil() as u32;
            let glyph_h = (bounds.max.y - bounds.min.y).ceil() as u32;

            if glyph_w == 0 || glyph_h == 0 {
                let region = GlyphRegion {
                    u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0,
                    width_px: 0.0, height_px: 0.0,
                    bearing_x: 0.0, bearing_y: 0.0,
                };
                self.cache.insert(key, region);
                return region;
            }

            let padded_w = glyph_w + Self::GLYPH_PADDING * 2;
            let padded_h = glyph_h + Self::GLYPH_PADDING * 2;

            // Allocate space in the atlas (shelf packing)
            if !self.allocate_shelf(padded_w, padded_h) {
                // Atlas full — try to grow
                if !self.grow() {
                    // Can't grow, return empty region
                    let region = GlyphRegion {
                        u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0,
                        width_px: 0.0, height_px: 0.0,
                        bearing_x: 0.0, bearing_y: 0.0,
                    };
                    self.cache.insert(key, region);
                    return region;
                }
                // Retry after grow
                if !self.allocate_shelf(padded_w, padded_h) {
                    let region = GlyphRegion {
                        u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0,
                        width_px: 0.0, height_px: 0.0,
                        bearing_x: 0.0, bearing_y: 0.0,
                    };
                    self.cache.insert(key, region);
                    return region;
                }
            }

            let atlas_x = self.shelf_x - padded_w;
            let atlas_y = self.shelf_y;

            // Rasterize into the bitmap
            let bx = atlas_x + Self::GLYPH_PADDING;
            let by = atlas_y + Self::GLYPH_PADDING;
            outlined.draw(|x, y, alpha| {
                let px = bx + x;
                let py = by + y;
                if px < self.width && py < self.height {
                    self.bitmap[(py * self.width + px) as usize] = (alpha * 255.0) as u8;
                }
            });

            self.dirty = true;

            // Compute UV region (glyph area without padding)
            let region = GlyphRegion {
                u0: bx as f32 / self.width as f32,
                v0: by as f32 / self.height as f32,
                u1: (bx + glyph_w) as f32 / self.width as f32,
                v1: (by + glyph_h) as f32 / self.height as f32,
                width_px: glyph_w as f32,
                height_px: glyph_h as f32,
                bearing_x: bounds.min.x,
                bearing_y: bounds.min.y,
            };
            self.cache.insert(key, region);
            region
        } else {
            // No outline (space, control char, etc)
            let h_advance = scaled_font.h_advance(glyph_id);
            let region = GlyphRegion {
                u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0,
                width_px: h_advance, height_px: 0.0,
                bearing_x: 0.0, bearing_y: 0.0,
            };
            self.cache.insert(key, region);
            region
        }
    }

    /// Try to allocate space in the current shelf. Returns true if successful.
    fn allocate_shelf(&mut self, w: u32, h: u32) -> bool {
        // Does it fit on the current shelf?
        if self.shelf_x + w <= self.width && self.shelf_y + h.max(self.shelf_height) <= self.height {
            self.shelf_x += w;
            if h > self.shelf_height {
                self.shelf_height = h;
            }
            return true;
        }

        // Try starting a new shelf
        let new_shelf_y = self.shelf_y + self.shelf_height;
        if w <= self.width && new_shelf_y + h <= self.height {
            self.shelf_y = new_shelf_y;
            self.shelf_x = w;
            self.shelf_height = h;
            return true;
        }

        false
    }

    /// Double the atlas size, preserving existing content.
    fn grow(&mut self) -> bool {
        let new_size = self.width * 2;
        if new_size > Self::MAX_SIZE {
            return false;
        }

        let mut new_bitmap = vec![0u8; (new_size * new_size) as usize];
        // Copy old bitmap row by row
        for y in 0..self.height {
            let src_start = (y * self.width) as usize;
            let src_end = src_start + self.width as usize;
            let dst_start = (y * new_size) as usize;
            new_bitmap[dst_start..dst_start + self.width as usize]
                .copy_from_slice(&self.bitmap[src_start..src_end]);
        }

        self.bitmap = new_bitmap;
        // Update UV coords for existing cached glyphs
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

    /// Upload the CPU bitmap to the GPU texture if dirty.
    /// May recreate the texture if the atlas was resized.
    pub fn ensure_uploaded(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.dirty {
            return;
        }

        // Check if texture size matches
        let tex_size = self.texture.size();
        if tex_size.width != self.width || tex_size.height != self.height {
            let (texture, view, sampler) =
                Self::create_gpu_resources(device, self.width, self.height);
            self.texture = texture;
            self.view = view;
            self.sampler = sampler;
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.dirty = false;
    }

    /// Clear the atlas and rebuild from scratch. Call on font size/family change.
    pub fn reset(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, font_size_px: f32) {
        self.font_size_px = font_size_px;
        self.cache.clear();
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_height = 0;

        // Reset bitmap
        let w = Self::INITIAL_SIZE;
        let h = Self::INITIAL_SIZE;
        self.bitmap = vec![0u8; (w * h) as usize];
        self.width = w;
        self.height = h;

        let (texture, view, sampler) = Self::create_gpu_resources(device, w, h);
        self.texture = texture;
        self.view = view;
        self.sampler = sampler;

        self.prepopulate_ascii();
        self.ensure_uploaded(device, queue);
    }

    pub fn atlas_width(&self) -> u32 {
        self.width
    }

    pub fn atlas_height(&self) -> u32 {
        self.height
    }
}
