use crate::terminal::Color;
use egui::Color32;

pub fn to_egui_color32(color: Color) -> Color32 {
    match color {
        Color::Black => Color32::from_rgb(50, 50, 50), // 更深的黑色
        Color::Red => Color32::from_rgb(220, 80, 80),  // 更亮的红色
        Color::Green => Color32::from_rgb(80, 220, 100), // 更亮的绿色
        Color::Yellow => Color32::from_rgb(220, 220, 80), // 更亮的黄色
        Color::Blue => Color32::from_rgb(100, 150, 220), // 更亮的蓝色
        Color::Magenta => Color32::from_rgb(220, 100, 200), // 更亮的品红
        Color::Cyan => Color32::from_rgb(100, 220, 220), // 更亮的青色
        Color::White => Color32::from_rgb(240, 240, 240), // 更亮的白色
        Color::BrightBlack => Color32::from_rgb(160, 160, 160), // 更亮的亮黑
        Color::BrightRed => Color32::from_rgb(255, 120, 120), // 纯亮红
        Color::BrightGreen => Color32::from_rgb(150, 255, 150), // 纯亮绿
        Color::BrightYellow => Color32::from_rgb(255, 255, 150), // 纯亮黄
        Color::BrightBlue => Color32::from_rgb(150, 200, 255), // 纯亮蓝
        Color::BrightMagenta => Color32::from_rgb(255, 150, 255), // 纯亮品红
        Color::BrightCyan => Color32::from_rgb(150, 255, 255), // 纯亮青
        Color::BrightWhite => Color32::from_rgb(255, 255, 255), // 纯白
        Color::Indexed(idx) => color_256(idx),
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        Color::Default => Color32::from_rgb(220, 220, 220), // 亮灰色作为默认
    }
}

pub fn color_256(idx: u8) -> Color32 {
    match idx {
        0..=15 => {
            let colors = [
                Color32::from_rgb(0, 0, 0),
                Color32::from_rgb(205, 49, 49),
                Color32::from_rgb(13, 188, 121),
                Color32::from_rgb(229, 229, 16),
                Color32::from_rgb(36, 114, 200),
                Color32::from_rgb(188, 63, 60),
                Color32::from_rgb(17, 168, 205),
                Color32::from_rgb(229, 229, 229),
                Color32::from_rgb(127, 127, 127),
                Color32::from_rgb(255, 85, 85),
                Color32::from_rgb(85, 255, 85),
                Color32::from_rgb(255, 255, 85),
                Color32::from_rgb(85, 85, 255),
                Color32::from_rgb(255, 85, 255),
                Color32::from_rgb(85, 255, 255),
                Color32::from_rgb(255, 255, 255),
            ];
            colors[idx as usize]
        }
        16..=231 => {
            let idx = idx - 16;
            let r = (idx / 36) * 51;
            let g = ((idx % 36) / 6) * 51;
            let b = (idx % 6) * 51;
            Color32::from_rgb(r as u8, g as u8, b as u8)
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            Color32::from_rgb(gray, gray, gray)
        }
    }
}

pub mod defaults {
    use egui::Color32;

    pub const FOREGROUND: Color32 = Color32::from_rgb(220, 220, 220); // 亮灰色
    pub const BACKGROUND: Color32 = Color32::from_rgb(29, 29, 29); // 深灰背景
    pub const CURSOR: Color32 = Color32::from_rgb(80, 80, 80); // 深灰色光标
    pub fn selection() -> Color32 {
        Color32::from_rgba_unmultiplied(200, 200, 200, 100)
    }
}
