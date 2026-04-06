use egui::Color32;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 终端颜色配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalColors {
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub cursor: [u8; 3],
    pub selection: [u8; 4],  // RGBA
    pub ansi_colors: [[u8; 3]; 16],  // 标准 16 色
}

/// UI 组件颜色
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UIColors {
    pub window_bg: [u8; 3],
    pub panel_bg: [u8; 3],
    pub border: [u8; 3],
    pub text: [u8; 3],
    pub text_disabled: [u8; 3],
}

/// 滚动条颜色
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrollbarColors {
    pub track_normal: [u8; 4],
    pub track_hover: [u8; 4],
    pub track_drag: [u8; 4],
    pub thumb_normal: [u8; 4],
    pub thumb_hover: [u8; 4],
    pub thumb_drag: [u8; 4],
}

/// 标签栏颜色
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabbarColors {
    pub bg: [u8; 3],
    pub border: [u8; 3],
    pub inactive_text: [u8; 3],
    pub active_text: [u8; 3],
    pub active_border: [u8; 3],
    pub close_btn_bg: [u8; 3],
    pub close_btn_hover: [u8; 3],
}

/// 搜索框颜色
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchColors {
    pub bg: [u8; 3],
    pub border: [u8; 3],
    pub text: [u8; 3],
}

/// 命令调板颜色
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandPaletteColors {
    pub session_color: [u8; 3],
    pub edit_color: [u8; 3],
    pub search_color: [u8; 3],
    pub terminal_color: [u8; 3],
    pub window_color: [u8; 3],
}

/// 完整的主题定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub terminal: TerminalColors,
    pub ui: UIColors,
    pub scrollbar: ScrollbarColors,
    pub tabbar: TabbarColors,
    pub search: SearchColors,
    pub palette: CommandPaletteColors,
}

impl Theme {
    /// 获取内置的深色主题
    pub fn builtin_dark() -> Self {
        Theme {
            name: "dark".to_string(),
            terminal: TerminalColors {
                foreground: [220, 220, 220],
                background: [29, 29, 29],
                cursor: [80, 80, 80],
                selection: [200, 200, 200, 100],
                ansi_colors: [
                    [0, 0, 0],           // Black
                    [205, 49, 49],       // Red
                    [13, 177, 47],       // Green
                    [229, 178, 34],      // Yellow
                    [36, 114, 200],      // Blue
                    [188, 63, 60],       // Magenta
                    [17, 168, 205],      // Cyan
                    [230, 230, 230],     // White
                    [80, 80, 80],        // Bright Black
                    [241, 76, 76],       // Bright Red
                    [35, 209, 139],      // Bright Green
                    [245, 245, 67],      // Bright Yellow
                    [59, 142, 234],      // Bright Blue
                    [214, 112, 214],     // Bright Magenta
                    [41, 184, 219],      // Bright Cyan
                    [255, 255, 255],     // Bright White
                ],
            },
            ui: UIColors {
                window_bg: [29, 29, 29],
                panel_bg: [29, 29, 29],
                border: [80, 80, 80],
                text: [220, 220, 220],
                text_disabled: [120, 120, 120],
            },
            scrollbar: ScrollbarColors {
                track_normal: [72, 72, 80, 42],
                track_hover: [84, 84, 92, 64],
                track_drag: [92, 92, 100, 88],
                thumb_normal: [146, 146, 156, 118],
                thumb_hover: [166, 166, 176, 156],
                thumb_drag: [188, 188, 196, 188],
            },
            tabbar: TabbarColors {
                bg: [40, 40, 40],
                border: [120, 120, 130],
                inactive_text: [180, 180, 190],
                active_text: [220, 220, 220],
                active_border: [100, 200, 255],
                close_btn_bg: [100, 50, 50],
                close_btn_hover: [255, 150, 150],
            },
            search: SearchColors {
                bg: [40, 40, 40],
                border: [100, 100, 100],
                text: [220, 220, 220],
            },
            palette: CommandPaletteColors {
                session_color: [100, 150, 255],
                edit_color: [100, 200, 100],
                search_color: [255, 200, 100],
                terminal_color: [150, 150, 255],
                window_color: [200, 100, 200],
            },
        }
    }

    /// 获取内置的浅色主题
    pub fn builtin_light() -> Self {
        Theme {
            name: "light".to_string(),
            terminal: TerminalColors {
                foreground: [20, 20, 20],
                background: [250, 250, 250],
                cursor: [100, 100, 100],
                selection: [200, 220, 255, 150],
                ansi_colors: [
                    [0, 0, 0],
                    [200, 0, 0],
                    [0, 180, 0],
                    [180, 140, 0],
                    [0, 80, 200],
                    [200, 0, 200],
                    [0, 170, 170],
                    [200, 200, 200],
                    [100, 100, 100],
                    [255, 0, 0],
                    [0, 255, 0],
                    [255, 200, 0],
                    [0, 100, 255],
                    [255, 0, 255],
                    [0, 200, 200],
                    [255, 255, 255],
                ],
            },
            ui: UIColors {
                window_bg: [250, 250, 250],
                panel_bg: [240, 240, 240],
                border: [150, 150, 150],
                text: [20, 20, 20],
                text_disabled: [120, 120, 120],
            },
            scrollbar: ScrollbarColors {
                track_normal: [220, 220, 220, 60],
                track_hover: [200, 200, 200, 90],
                track_drag: [180, 180, 180, 120],
                thumb_normal: [150, 150, 150, 150],
                thumb_hover: [120, 120, 120, 180],
                thumb_drag: [100, 100, 100, 200],
            },
            tabbar: TabbarColors {
                bg: [240, 240, 240],
                border: [150, 150, 150],
                inactive_text: [80, 80, 80],
                active_text: [0, 0, 0],
                active_border: [100, 150, 255],
                close_btn_bg: [200, 150, 150],
                close_btn_hover: [255, 100, 100],
            },
            search: SearchColors {
                bg: [240, 240, 240],
                border: [150, 150, 150],
                text: [20, 20, 20],
            },
            palette: CommandPaletteColors {
                session_color: [50, 100, 200],
                edit_color: [50, 150, 50],
                search_color: [200, 150, 0],
                terminal_color: [100, 100, 180],
                window_color: [150, 50, 150],
            },
        }
    }

    /// 获取内置的 Solarized Dark 主题
    pub fn builtin_solarized_dark() -> Self {
        Theme {
            name: "solarized-dark".to_string(),
            terminal: TerminalColors {
                foreground: [131, 148, 150],
                background: [0, 43, 54],
                cursor: [133, 153, 0],
                selection: [7, 54, 66, 100],
                ansi_colors: [
                    [7, 54, 66],         // Black
                    [220, 50, 47],       // Red
                    [133, 153, 0],       // Green
                    [181, 137, 0],       // Yellow
                    [38, 139, 210],      // Blue
                    [108, 113, 196],     // Magenta
                    [42, 161, 152],      // Cyan
                    [131, 148, 150],     // White
                    [101, 123, 131],     // Bright Black
                    [203, 75, 75],       // Bright Red
                    [88, 110, 117],      // Bright Green
                    [101, 123, 131],     // Bright Yellow
                    [131, 148, 150],     // Bright Blue
                    [108, 113, 196],     // Bright Magenta
                    [147, 161, 161],     // Bright Cyan
                    [253, 246, 227],     // Bright White
                ],
            },
            ui: UIColors {
                window_bg: [0, 43, 54],
                panel_bg: [7, 54, 66],
                border: [101, 123, 131],
                text: [131, 148, 150],
                text_disabled: [88, 110, 117],
            },
            scrollbar: ScrollbarColors {
                track_normal: [7, 54, 66, 100],
                track_hover: [7, 54, 66, 150],
                track_drag: [101, 123, 131, 180],
                thumb_normal: [42, 161, 152, 150],
                thumb_hover: [42, 161, 152, 180],
                thumb_drag: [38, 139, 210, 200],
            },
            tabbar: TabbarColors {
                bg: [7, 54, 66],
                border: [101, 123, 131],
                inactive_text: [88, 110, 117],
                active_text: [131, 148, 150],
                active_border: [38, 139, 210],
                close_btn_bg: [220, 50, 47],
                close_btn_hover: [220, 50, 47],
            },
            search: SearchColors {
                bg: [7, 54, 66],
                border: [101, 123, 131],
                text: [131, 148, 150],
            },
            palette: CommandPaletteColors {
                session_color: [38, 139, 210],
                edit_color: [133, 153, 0],
                search_color: [181, 137, 0],
                terminal_color: [42, 161, 152],
                window_color: [108, 113, 196],
            },
        }
    }

    /// 从文件加载主题
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = toml::from_str(&content)?;
        Ok(theme)
    }

    /// 保存主题到文件
    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 将 RGB 数组转换为 Color32
    pub fn rgb_to_color32(rgb: [u8; 3]) -> Color32 {
        Color32::from_rgb(rgb[0], rgb[1], rgb[2])
    }

    /// 将 RGBA 数组转换为 Color32
    pub fn rgba_to_color32(rgba: [u8; 4]) -> Color32 {
        Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
    }

    /// 获取终端前景色
    pub fn terminal_foreground(&self) -> Color32 {
        Self::rgb_to_color32(self.terminal.foreground)
    }

    /// 获取终端背景色
    pub fn terminal_background(&self) -> Color32 {
        Self::rgb_to_color32(self.terminal.background)
    }

    /// 获取光标颜色
    pub fn cursor_color(&self) -> Color32 {
        Self::rgb_to_color32(self.terminal.cursor)
    }

    /// 获取选择颜色
    pub fn selection_color(&self) -> Color32 {
        Self::rgba_to_color32(self.terminal.selection)
    }

    /// 获取 ANSI 颜色
    pub fn ansi_color(&self, index: usize) -> Color32 {
        if index < 16 {
            Self::rgb_to_color32(self.terminal.ansi_colors[index])
        } else {
            Self::rgb_to_color32(self.terminal.foreground)
        }
    }

    /// 获取可用的主题列表
    pub fn available_themes() -> Vec<&'static str> {
        vec!["dark", "light", "solarized-dark"]
    }

    /// 根据名称获取内置主题
    pub fn get_builtin(name: &str) -> Option<Self> {
        match name {
            "dark" => Some(Self::builtin_dark()),
            "light" => Some(Self::builtin_light()),
            "solarized-dark" => Some(Self::builtin_solarized_dark()),
            _ => None,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::builtin_dark()
    }
}
