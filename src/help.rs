//! 快捷键帮助面板模块

use egui::{Color32, RichText, Window};

/// 帮助面板状态
#[derive(Debug, Clone)]
pub struct HelpPanel {
    pub is_open: bool,
}

impl HelpPanel {
    pub fn new() -> Self {
        HelpPanel { is_open: false }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn show(&self, ctx: &egui::Context, open: &mut bool) {
        if !*open {
            return;
        }

        Window::new("快捷键帮助 (Keybindings Help)")
            .open(open)
            .default_size([600.0, 500.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // 标题
                    ui.heading(RichText::new("⌨️ JTerm2 快捷键").size(18.0));
                    ui.separator();

                    // 分屏操作
                    ui.collapsing("🖥️ 分屏操作 (Split Panes)", |ui| {
                        Self::add_keybinding(ui, "Ctrl+Shift+D", "垂直分割 (Left/Right)");
                        Self::add_keybinding(ui, "Ctrl+Shift+E", "水平分割 (Top/Bottom)");
                        Self::add_keybinding(ui, "Ctrl+Shift+W", "关闭焦点窗格");
                        Self::add_keybinding(ui, "Alt+Tab", "切换窗格焦点");
                    });

                    ui.separator();

                    // 会话管理
                    ui.collapsing("💬 会话管理 (Sessions)", |ui| {
                        Self::add_keybinding(ui, "Ctrl+T", "新建会话");
                        Self::add_keybinding(ui, "Ctrl+Tab", "下一个会话");
                        Self::add_keybinding(ui, "Ctrl+Shift+Tab", "上一个会话");
                        Self::add_keybinding(ui, "Ctrl+1..9", "快速跳转到第 1-9 个会话");
                    });

                    ui.separator();

                    // 输入和编辑
                    ui.collapsing("✏️ 输入和编辑 (Input & Edit)", |ui| {
                        Self::add_keybinding(ui, "Ctrl+Shift+C", "复制选中文本");
                        Self::add_keybinding(ui, "Ctrl+Shift+V", "粘贴剪贴板");
                        Self::add_keybinding(ui, "Ctrl+V", "粘贴 (备选)");
                        Self::add_keybinding(ui, "F2", "复制 (备选)");
                        Self::add_keybinding(ui, "F3", "粘贴 (备选)");
                        Self::add_keybinding(ui, "拖拽", "鼠标框选文本");
                    });

                    ui.separator();

                    // 滚动和导航
                    ui.collapsing("📜 滚动和导航 (Scroll & Navigation)", |ui| {
                        Self::add_keybinding(ui, "Page Up", "向上滚动");
                        Self::add_keybinding(ui, "Page Down", "向下滚动");
                        Self::add_keybinding(ui, "拖拽滚动条", "自由滚动历史");
                        Self::add_keybinding(ui, "点击滚动条轨道", "分页滚动");
                    });

                    ui.separator();

                    // 搜索和查找
                    ui.collapsing("🔍 搜索 (Search)", |ui| {
                        Self::add_keybinding(ui, "Ctrl+F", "打开搜索面板");
                        Self::add_keybinding(ui, "Escape", "关闭搜索");
                        Self::add_keybinding(ui, "Enter / ↑↓", "导航搜索结果");
                    });

                    ui.separator();

                    // 终端操作
                    ui.collapsing("🖨️ 终端操作 (Terminal)", |ui| {
                        Self::add_keybinding(ui, "Ctrl+L", "清屏");
                        Self::add_keybinding(ui, "Ctrl+D", "发送 EOF (退出 Shell)");
                        Self::add_keybinding(ui, "Ctrl+C", "中断当前命令");
                    });

                    ui.separator();

                    // 命令和工具
                    ui.collapsing("⚙️ 命令和工具 (Commands)", |ui| {
                        Self::add_keybinding(ui, "Ctrl+Shift+P", "命令调色板");
                        Self::add_keybinding(ui, "Ctrl+?", "显示/隐藏帮助");
                    });

                    ui.separator();

                    // 提示信息
                    ui.label(RichText::new("💡 提示: 用鼠标拖拽分隔线调整窗格大小，用鼠标点击标签页关闭会话")
                        .size(11.0)
                        .color(Color32::from_rgb(150, 150, 150)));
                });
            });
    }

    fn add_keybinding(ui: &mut egui::Ui, key: &str, description: &str) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(key).monospace().color(Color32::from_rgb(100, 200, 255)));
            ui.label(RichText::new(description).color(Color32::from_rgb(200, 200, 200)));
        });
    }
}

impl Default for HelpPanel {
    fn default() -> Self {
        Self::new()
    }
}
