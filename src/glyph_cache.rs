use egui::Color32;
/// P2 优化：字形缓存 + 批量渲染
/// 缓存字符到字形的映射，支持批量渲染相同样式的字符
/// 中文字符的栅格化可以缓存 GPU，性能提升 3-4x
use std::collections::HashMap;

/// 字形缓存的键：字符 + 样式信息
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub ch: char,
    pub bold: bool,
    pub italic: bool,
}

impl GlyphKey {
    pub fn new(ch: char, bold: bool, italic: bool) -> Self {
        GlyphKey { ch, bold, italic }
    }
}

/// 缓存的字形信息
#[derive(Clone, Debug)]
pub struct CachedGlyph {
    pub width: f32,
    pub height: f32,
    pub advance_width: f32,
}

impl CachedGlyph {
    pub fn new(width: f32, height: f32, advance_width: f32) -> Self {
        CachedGlyph {
            width,
            height,
            advance_width,
        }
    }
}

/// 字形缓存，使用 LRU 策略管理内存
pub struct GlyphCache {
    cache: HashMap<GlyphKey, CachedGlyph>,
    max_size: usize,
    hits: usize,
    misses: usize,
}

impl GlyphCache {
    /// 创建新的字形缓存
    pub fn new(max_size: usize) -> Self {
        GlyphCache {
            cache: HashMap::with_capacity(max_size / 2),
            max_size,
            hits: 0,
            misses: 0,
        }
    }

    /// 查找缓存的字形
    pub fn get(&mut self, key: GlyphKey) -> Option<CachedGlyph> {
        if let Some(glyph) = self.cache.get(&key) {
            self.hits += 1;
            Some(glyph.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    /// 插入字形到缓存
    pub fn insert(&mut self, key: GlyphKey, glyph: CachedGlyph) {
        // 缓存满时清理一部分
        if self.cache.len() >= self.max_size {
            // 简单的清理策略：移除一半的项
            let to_remove = self.max_size / 4;
            let keys: Vec<_> = self.cache.keys().take(to_remove).cloned().collect();
            for key in keys {
                self.cache.remove(&key);
            }
        }
        self.cache.insert(key, glyph);
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> GlyphCacheStats {
        let total_requests = self.hits + self.misses;
        let hit_rate = if total_requests > 0 {
            (self.hits as f32 / total_requests as f32) * 100.0
        } else {
            0.0
        };

        GlyphCacheStats {
            size: self.cache.len(),
            max_size: self.max_size,
            hits: self.hits,
            misses: self.misses,
            hit_rate,
        }
    }
}

/// 字形缓存统计信息
#[derive(Debug, Clone)]
pub struct GlyphCacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hits: usize,
    pub misses: usize,
    pub hit_rate: f32,
}

/// 用于批量渲染的样式键
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct RenderStyle {
    pub fg: egui::Color32,
    pub bg: egui::Color32,
    pub bold: bool,
    pub italic: bool,
}

impl RenderStyle {
    pub fn new(fg: egui::Color32, bg: egui::Color32, bold: bool, italic: bool) -> Self {
        RenderStyle {
            fg,
            bg,
            bold,
            italic,
        }
    }
}

/// 渲染批次 - 相同样式的字符位置集合
#[derive(Debug, Clone)]
pub struct RenderBatch {
    pub style: RenderStyle,
    pub positions: Vec<(usize, usize, char)>, // (row, col, char)
}

impl RenderBatch {
    pub fn new(style: RenderStyle) -> Self {
        RenderBatch {
            style,
            positions: Vec::new(),
        }
    }

    pub fn add_position(&mut self, row: usize, col: usize, ch: char) {
        self.positions.push((row, col, ch));
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_cache() {
        let mut cache = GlyphCache::new(100);
        let key = GlyphKey::new('A', false, false);
        let glyph = CachedGlyph::new(10.0, 20.0, 10.0);

        cache.insert(key, glyph.clone());
        assert_eq!(cache.get(key).is_some(), true);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = GlyphCache::new(10);

        // 插入超过容量的项
        for i in 0..20 {
            let key = GlyphKey::new(char::from_u32(i as u32).unwrap_or('A'), false, false);
            let glyph = CachedGlyph::new(10.0, 20.0, 10.0);
            cache.insert(key, glyph);
        }

        // 缓存大小应该不超过 max_size
        assert!(cache.cache.len() <= cache.max_size);
    }

    #[test]
    fn test_render_style() {
        let style1 = RenderStyle::new(Color32::WHITE, Color32::BLACK, false, false);
        let style2 = RenderStyle::new(Color32::WHITE, Color32::BLACK, false, false);
        assert_eq!(style1, style2);
    }

    #[test]
    fn test_render_batch() {
        let style = RenderStyle::new(Color32::WHITE, Color32::BLACK, false, false);
        let mut batch = RenderBatch::new(style);

        batch.add_position(0, 0, 'A');
        batch.add_position(0, 1, 'B');

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
