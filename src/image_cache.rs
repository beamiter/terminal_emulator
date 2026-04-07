use std::collections::VecDeque;
use crate::kitty_graphics::KittyImage;

/// GPU 纹理缓存管理器
pub struct ImageCache {
    /// 内存中缓存的图像 (ID -> 图像)
    cache: std::collections::HashMap<u32, KittyImage>,
    /// LRU 队列 (最近使用的在后面)
    lru_queue: VecDeque<u32>,
    /// 当前内存占用 (字节)
    memory_used: usize,
    /// 内存限制 (100MB)
    memory_limit: usize,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
            lru_queue: VecDeque::new(),
            memory_used: 0,
            memory_limit: 100 * 1024 * 1024, // 100MB
        }
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            memory_limit: limit,
            ..Default::default()
        }
    }

    /// 插入图像，自动进行 LRU 管理
    pub fn insert(&mut self, image: KittyImage) {
        let image_id = image.id;
        let image_size = self.estimate_image_size(&image);

        // 如果图像已存在，删除旧的
        if let Some(old_image) = self.cache.remove(&image_id) {
            self.memory_used -= self.estimate_image_size(&old_image);
        }

        // 删除 LRU 队列中的该 ID（如果存在）
        if let Some(pos) = self.lru_queue.iter().position(|&id| id == image_id) {
            self.lru_queue.remove(pos);
        }

        // 检查是否需要清除缓存
        while self.memory_used + image_size > self.memory_limit && !self.cache.is_empty() {
            self.evict_lru();
        }

        // 插入新图像
        self.memory_used += image_size;
        self.cache.insert(image_id, image);
        self.lru_queue.push_back(image_id);

        log::info!(
            "[IMAGE_CACHE] Inserted image {} (size: {:.2}MB, total: {:.2}MB)",
            image_id,
            image_size as f64 / 1024.0 / 1024.0,
            self.memory_used as f64 / 1024.0 / 1024.0
        );
    }

    /// 获取图像（更新 LRU）
    pub fn get(&mut self, id: u32) -> Option<&KittyImage> {
        if self.cache.contains_key(&id) {
            // 移动到 LRU 队列的末尾（最近使用）
            if let Some(pos) = self.lru_queue.iter().position(|&img_id| img_id == id) {
                self.lru_queue.remove(pos);
            }
            self.lru_queue.push_back(id);
        }
        self.cache.get(&id)
    }

    /// 获取图像（不更新 LRU）
    pub fn get_ref(&self, id: u32) -> Option<&KittyImage> {
        self.cache.get(&id)
    }

    /// 删除图像
    pub fn remove(&mut self, id: u32) -> Option<KittyImage> {
        if let Some(pos) = self.lru_queue.iter().position(|&img_id| img_id == id) {
            self.lru_queue.remove(pos);
        }
        self.cache.remove(&id).map(|img| {
            self.memory_used -= self.estimate_image_size(&img);
            log::info!("[IMAGE_CACHE] Removed image {}", id);
            img
        })
    }

    /// 清除所有缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_queue.clear();
        self.memory_used = 0;
        log::info!("[IMAGE_CACHE] Cleared all images");
    }

    /// 获取当前内存占用
    pub fn memory_used(&self) -> usize {
        self.memory_used
    }

    /// 获取缓存大小
    pub fn size(&self) -> usize {
        self.cache.len()
    }

    /// LRU 清除：删除最久未使用的项
    fn evict_lru(&mut self) {
        if let Some(oldest_id) = self.lru_queue.pop_front() {
            if let Some(image) = self.cache.remove(&oldest_id) {
                self.memory_used -= self.estimate_image_size(&image);
                log::warn!(
                    "[IMAGE_CACHE] Evicted image {} due to memory limit",
                    oldest_id
                );
            }
        }
    }

    /// 估计图像占用的内存大小
    fn estimate_image_size(&self, image: &KittyImage) -> usize {
        // 基础大小 + 图像数据大小
        std::mem::size_of::<KittyImage>() + image.data.len()
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kitty_graphics::ImageFormat;

    #[test]
    fn test_cache_insertion() {
        let mut cache = ImageCache::with_limit(1024 * 1024); // 1MB limit
        let image = KittyImage {
            id: 1,
            format: ImageFormat::Png,
            width: 100,
            height: 100,
            data: vec![0u8; 1000],
        };

        cache.insert(image);
        assert!(cache.get_ref(1).is_some());
        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = ImageCache::with_limit(500); // Very small limit

        for i in 1..=5 {
            let image = KittyImage {
                id: i,
                format: ImageFormat::Png,
                width: 10,
                height: 10,
                data: vec![0u8; 100],
            };
            cache.insert(image);
        }

        // Due to memory limit, older images should be evicted
        assert!(cache.size() < 5);
    }

    #[test]
    fn test_get_updates_lru() {
        let mut cache = ImageCache::new();

        let image1 = KittyImage {
            id: 1,
            format: ImageFormat::Png,
            width: 10,
            height: 10,
            data: vec![0u8; 100],
        };
        cache.insert(image1);

        let image2 = KittyImage {
            id: 2,
            format: ImageFormat::Png,
            width: 10,
            height: 10,
            data: vec![0u8; 100],
        };
        cache.insert(image2);

        // Access image 1, making it recently used
        cache.get(1);

        // Queue should now be: 2, 1
        assert_eq!(*cache.lru_queue.back().unwrap(), 1);
    }
}
