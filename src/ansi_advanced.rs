use serde::{Deserialize, Serialize};

/// 超链接（OSC 8）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hyperlink {
    pub url: String,
    pub text: String,
    pub id: Option<String>,
}

impl Hyperlink {
    pub fn to_ansi_string(&self) -> String {
        let id = self.id.as_deref().unwrap_or("");
        format!("\x1b]8;{};{}\x1b\\{}\x1b]8;;\x1b\\", id, self.url, self.text)
    }

    pub fn from_ansi_string(s: &str) -> Option<Self> {
        // 简化解析：\x1b]8;id;url\x1b\text\x1b]8;;\x1b\
        if !s.contains("\x1b]8;") {
            return None;
        }

        // 提取 URL 和文本
        let parts: Vec<&str> = s.split("\x1b\\").collect();
        if parts.len() >= 2 {
            let url_part = parts[0];
            let text = parts[1];

            if let Some(url_start) = url_part.find(';') {
                let url = &url_part[url_start + 1..];
                return Some(Hyperlink {
                    url: url.to_string(),
                    text: text.to_string(),
                    id: None,
                });
            }
        }

        None
    }
}

/// 256 色及 TrueColor 支持
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum ColorMode {
    Standard16,   // 16 种基本色
    Color256,     // 256 色
    TrueColor,    // 24 位 RGB
}

impl ColorMode {
    pub fn supports_256(&self) -> bool {
        matches!(self, ColorMode::Color256 | ColorMode::TrueColor)
    }

    pub fn supports_truecolor(&self) -> bool {
        matches!(self, ColorMode::TrueColor)
    }

    /// 获取 COLORTERM 环境变量值
    pub fn colorterm_value(&self) -> &'static str {
        match self {
            ColorMode::Standard16 => "8color",
            ColorMode::Color256 => "256color",
            ColorMode::TrueColor => "truecolor",
        }
    }
}

/// 扩展的 ANSI 序列生成器
pub struct AnsiBuilder;

impl AnsiBuilder {
    /// 生成 256 色前景色序列
    pub fn color256_fg(color: u8) -> String {
        format!("\x1b[38;5;{}m", color)
    }

    /// 生成 256 色背景色序列
    pub fn color256_bg(color: u8) -> String {
        format!("\x1b[48;5;{}m", color)
    }

    /// 生成 TrueColor（24位）前景色序列
    pub fn truecolor_fg(r: u8, g: u8, b: u8) -> String {
        format!("\x1b[38;2;{};{};{}m", r, g, b)
    }

    /// 生成 TrueColor（24位）背景色序列
    pub fn truecolor_bg(r: u8, g: u8, b: u8) -> String {
        format!("\x1b[48;2;{};{};{}m", r, g, b)
    }

    /// 生成下划线样式（各种类型）
    pub fn underline(style: UnderlineStyle) -> String {
        match style {
            UnderlineStyle::Single => "\x1b[4m".to_string(),
            UnderlineStyle::Double => "\x1b[21m".to_string(),
            UnderlineStyle::Curly => "\x1b[4:3m".to_string(),
            UnderlineStyle::Dotted => "\x1b[4:4m".to_string(),
            UnderlineStyle::Dashed => "\x1b[4:5m".to_string(),
        }
    }

    /// 生成重置序列
    pub fn reset() -> &'static str {
        "\x1b[0m"
    }

    /// 生成粗体序列
    pub fn bold() -> &'static str {
        "\x1b[1m"
    }

    /// 生成斜体序列
    pub fn italic() -> &'static str {
        "\x1b[3m"
    }

    /// 生成反转色序列
    pub fn invert() -> &'static str {
        "\x1b[7m"
    }

    /// 生成删除线序列
    pub fn strikethrough() -> &'static str {
        "\x1b[9m"
    }

    /// 生成光标形状序列
    pub fn cursor_shape(shape: CursorShape) -> String {
        match shape {
            CursorShape::Block => "\x1b[2 q".to_string(),
            CursorShape::Line => "\x1b[6 q".to_string(),
            CursorShape::Underline => "\x1b[4 q".to_string(),
        }
    }

    /// 生成标题序列（OSC 0）
    pub fn set_title(title: &str) -> String {
        format!("\x1b]0;{}\x07", title)
    }

    /// 生成通知序列（OSC 9）
    pub fn notification(title: &str, message: &str) -> String {
        format!("\x1b]9;4;1;notify-send '{}' '{}'\x07", title, message)
    }
}

#[derive(Clone, Debug, Copy)]
pub enum UnderlineStyle {
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Clone, Debug, Copy)]
pub enum CursorShape {
    Block,
    Line,
    Underline,
}

/// VT 功能检测
pub struct VtCapabilities {
    pub supports_256color: bool,
    pub supports_truecolor: bool,
    pub supports_hyperlink: bool,
    pub supports_mouse: bool,
    pub supports_focus_events: bool,
}

impl VtCapabilities {
    pub fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();

        VtCapabilities {
            supports_256color: term.contains("256") || colorterm == "256color",
            supports_truecolor: colorterm == "truecolor",
            supports_hyperlink: term != "dumb" && !term.contains("cygwin"),
            supports_mouse: !term.contains("dumb"),
            supports_focus_events: !term.contains("dumb"),
        }
    }

    /// 获取推荐的颜色模式
    pub fn recommended_color_mode(&self) -> ColorMode {
        if self.supports_truecolor {
            ColorMode::TrueColor
        } else if self.supports_256color {
            ColorMode::Color256
        } else {
            ColorMode::Standard16
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truecolor_fg() {
        let ansi = AnsiBuilder::truecolor_fg(255, 0, 0);
        assert_eq!(ansi, "\x1b[38;2;255;0;0m");
    }

    #[test]
    fn test_hyperlink() {
        let link = Hyperlink {
            url: "https://example.com".to_string(),
            text: "Example".to_string(),
            id: None,
        };
        let ansi = link.to_ansi_string();
        assert!(ansi.contains("https://example.com"));
    }

    #[test]
    fn test_cursor_shape() {
        let ansi = AnsiBuilder::cursor_shape(CursorShape::Block);
        assert_eq!(ansi, "\x1b[2 q");
    }
}
