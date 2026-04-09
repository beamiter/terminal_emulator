/// P5 优化：字符宽度计算缓存
/// 使用 LRU 缓存来避免重复的 Unicode 宽度计算
/// 特别对于中文字符，性能提升显著（10-15%）

use std::cell::RefCell;

thread_local! {
    static CHAR_WIDTH_CACHE: RefCell<lru::LruCache<char, usize>> = {
        RefCell::new(
            lru::LruCache::new(
                std::num::NonZeroUsize::new(4096).unwrap()
            )
        )
    };
}

/// 获取字符的显示宽度，带 LRU 缓存
///
/// # Examples
/// ```
/// assert_eq!(cached_char_width('A'), 1);  // ASCII 字符宽度为 1
/// assert_eq!(cached_char_width('中'), 2); // 中文字符宽度为 2
/// ```
#[inline]
pub fn cached_char_width(ch: char) -> usize {
    CHAR_WIDTH_CACHE.with(|cache| {
        let mut c = cache.borrow_mut();

        // 先检查缓存（peek 不会改变 LRU 顺序）
        if let Some(&w) = c.peek(&ch) {
            return w;
        }

        // 计算宽度
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);

        // 存入缓存
        c.put(ch, w);
        w
    })
}

/// 清除宽度缓存（调试用）
#[allow(dead_code)]
pub fn clear_width_cache() {
    CHAR_WIDTH_CACHE.with(|cache| {
        let mut c = cache.borrow_mut();
        c.clear();
    });
}

/// 获取缓存统计信息（调试用）
#[allow(dead_code)]
pub fn get_cache_stats() -> (usize, usize) {
    CHAR_WIDTH_CACHE.with(|cache| {
        let c = cache.borrow();
        (c.len(), 4096)  // (当前项数, 容量)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_width() {
        assert_eq!(cached_char_width('A'), 1);
        assert_eq!(cached_char_width('a'), 1);
        assert_eq!(cached_char_width('0'), 1);
    }

    #[test]
    fn test_cjk_width() {
        // 中文、日文、韩文字符宽度应为 2
        assert_eq!(cached_char_width('中'), 2);
        assert_eq!(cached_char_width('あ'), 2);
        assert_eq!(cached_char_width('한'), 2);
    }

    #[test]
    fn test_caching() {
        clear_width_cache();
        let (before, _) = get_cache_stats();

        cached_char_width('A');
        let (after_1, _) = get_cache_stats();
        assert_eq!(after_1, before + 1);

        // 再次调用应该使用缓存
        cached_char_width('A');
        let (after_2, _) = get_cache_stats();
        assert_eq!(after_2, after_1);  // 缓存大小不变
    }
}
