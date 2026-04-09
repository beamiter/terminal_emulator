use crate::config::Config;
use egui::{Color32, RichText};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigTab {
    Font,
    Appearance,
    Advanced,
}

pub enum ConfigAction {
    None,
    FontSizeChanged(f32),
    LineSpacingChanged(f32),
    FontFamilyChanged(String),
    ThemeChanged(String),
    PaddingChanged(f32),
    ScrollbackLinesChanged(usize),
    DebugPanelToggled(bool),
    SaveRequested,
    ResetToDefaults,
}

pub struct ConfigPanel {
    pub is_open: bool,
    active_tab: ConfigTab,
    // 编辑中的临时值
    edit_font_size: f32,
    edit_line_spacing: f32,
    edit_padding: f32,
    edit_scrollback_lines: usize,
    edit_font_family: String,
    edit_theme: String,
    edit_restore_session: bool,
    pub edit_debug_overlay: bool,
    // 系统字体缓存
    monospace_fonts: Vec<String>,
    all_fonts: Vec<String>,
    available_themes: Vec<String>,
    fonts_loaded: bool,
    // Font filter
    font_filter: String,
    show_all_fonts: bool,
    // 保存编辑状态
    has_changes: bool,
}

impl ConfigPanel {
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: ConfigTab::Font,
            edit_font_size: 14.0,
            edit_line_spacing: 1.3,
            edit_padding: 2.0,
            edit_scrollback_lines: 10000,
            edit_font_family: String::new(),
            edit_theme: "dark".to_string(),
            edit_restore_session: false,
            edit_debug_overlay: false,
            monospace_fonts: Vec::new(),
            all_fonts: Vec::new(),
            available_themes: crate::theme::Theme::available_themes()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            fonts_loaded: false,
            font_filter: String::new(),
            show_all_fonts: false,
            has_changes: false,
        }
    }

    pub fn open(&mut self, config: &Config) {
        self.is_open = true;
        self.has_changes = false;
        // 从当前 config 拷贝值到编辑字段
        self.edit_font_size = config.font_size;
        self.edit_line_spacing = config.line_spacing;
        self.edit_padding = config.padding;
        self.edit_scrollback_lines = config.scrollback_lines;
        self.edit_font_family = config.font_family.clone();
        self.edit_theme = config.theme.clone();
        self.edit_restore_session = config.restore_session;

        // 缓存系统字体列表（只加载一次）
        if !self.fonts_loaded {
            self.monospace_fonts = Config::get_monospace_fonts();
            self.all_fonts = Config::get_all_fonts();
            if self.monospace_fonts.is_empty() {
                self.monospace_fonts = vec![
                    "SauceCodePro Nerd Font".to_string(),
                    "JetBrains Mono".to_string(),
                    "Fira Code".to_string(),
                    "Monospace".to_string(),
                ];
            }
            self.fonts_loaded = true;
        }
        self.font_filter.clear();
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn toggle(&mut self, config: &Config) {
        if self.is_open {
            self.close();
        } else {
            self.open(config);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Vec<ConfigAction> {
        let mut actions = Vec::new();

        if !self.is_open {
            return actions;
        }

        let screen_rect = ctx.viewport_rect();
        let panel_width = 550.0;
        let panel_height = 500.0;
        let panel_pos = egui::pos2(
            (screen_rect.width() - panel_width) / 2.0,
            (screen_rect.height() - panel_height) / 3.0,
        );

        egui::Window::new("Settings")
            .title_bar(false)
            .resizable(false)
            .movable(true)
            .default_pos(panel_pos)
            .fixed_size([panel_width, panel_height])
            .frame(egui::Frame {
                fill: Color32::from_rgb(45, 45, 48),
                stroke: egui::Stroke::new(1.0, Color32::from_rgb(120, 120, 120)),
                inner_margin: egui::Margin::same(12),
                corner_radius: egui::CornerRadius::same(4),
                ..Default::default()
            })
            .show(ctx, |ui| {
                // 标题栏
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("⚙ Settings").size(18.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            self.is_open = false;
                        }
                    });
                });
                ui.separator();

                // Tab 栏
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.active_tab, ConfigTab::Font, "🔤 Font");
                    ui.selectable_value(&mut self.active_tab, ConfigTab::Appearance, "🎨 Appearance");
                    ui.selectable_value(&mut self.active_tab, ConfigTab::Advanced, "⚙ Advanced");
                });
                ui.separator();

                // 内容区域
                egui::ScrollArea::vertical()
                    .max_height(panel_height - 140.0)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        match self.active_tab {
                            ConfigTab::Font => {
                                self.render_font_tab(ui, &mut actions);
                            }
                            ConfigTab::Appearance => {
                                self.render_appearance_tab(ui, &mut actions);
                            }
                            ConfigTab::Advanced => {
                                self.render_advanced_tab(ui, &mut actions);
                            }
                        }
                    });

                // 底部按钮栏
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("↺ Reset to Defaults").clicked() {
                        actions.push(ConfigAction::ResetToDefaults);
                        self.has_changes = true;
                    }
                    if self.has_changes {
                        ui.label(RichText::new("●").color(Color32::from_rgb(200, 150, 0)).size(12.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✓ Save").clicked() {
                            actions.push(ConfigAction::SaveRequested);
                            self.has_changes = false;
                        }
                    });
                });
            });

        actions
    }

    fn render_font_tab(&mut self, ui: &mut egui::Ui, actions: &mut Vec<ConfigAction>) {
        ui.label(RichText::new("Font Settings").strong().size(14.0));
        ui.separator();

        // Font Size slider
        ui.horizontal(|ui| {
            ui.label("Size:");
            if ui
                .add(
                    egui::Slider::new(&mut self.edit_font_size, 8.0..=72.0)
                        .step_by(1.0)
                        .show_value(true),
                )
                .changed()
            {
                actions.push(ConfigAction::FontSizeChanged(self.edit_font_size));
                self.has_changes = true;
            }
            ui.label("px");
        });

        ui.separator();

        // Line Spacing slider
        ui.horizontal(|ui| {
            ui.label("Line Spacing:");
            if ui
                .add(
                    egui::Slider::new(&mut self.edit_line_spacing, 0.8..=3.0)
                        .step_by(0.05)
                        .show_value(true),
                )
                .changed()
            {
                actions.push(ConfigAction::LineSpacingChanged(self.edit_line_spacing));
                self.has_changes = true;
            }
        });

        ui.separator();

        // Current font display
        let current_display = if self.edit_font_family.is_empty() {
            "Default".to_string()
        } else {
            self.edit_font_family.clone()
        };
        ui.horizontal(|ui| {
            ui.label("Current:");
            ui.label(RichText::new(&current_display).strong().color(Color32::from_rgb(100, 200, 255)));
        });

        ui.add_space(4.0);

        // Search / filter input
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.add(
                egui::TextEdit::singleline(&mut self.font_filter)
                    .desired_width(200.0)
                    .hint_text("Filter fonts..."),
            );
            if !self.font_filter.is_empty() {
                if ui.small_button("x").clicked() {
                    self.font_filter.clear();
                }
            }
        });

        // Toggle: show all fonts vs monospace only
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_all_fonts, "Show all fonts");
            let count = if self.show_all_fonts {
                self.all_fonts.len()
            } else {
                self.monospace_fonts.len()
            };
            ui.label(
                RichText::new(format!("({} fonts)", count))
                    .size(11.0)
                    .color(Color32::from_rgb(140, 140, 140)),
            );
        });

        ui.add_space(4.0);

        // Font list
        let filter_lower = self.font_filter.to_lowercase();
        let fonts: &Vec<String> = if self.show_all_fonts {
            &self.all_fonts
        } else {
            &self.monospace_fonts
        };

        // "Default" option at top
        let show_default = self.font_filter.is_empty() || "default".contains(&filter_lower);
        let matched_fonts: Vec<&String> = fonts
            .iter()
            .filter(|f| filter_lower.is_empty() || f.to_lowercase().contains(&filter_lower))
            .collect();

        let total = matched_fonts.len() + if show_default { 1 } else { 0 };

        if total == 0 {
            ui.label(
                RichText::new("No matching fonts")
                    .italics()
                    .color(Color32::from_rgb(140, 140, 140)),
            );
        } else {
            let row_height = 22.0;
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .auto_shrink([false; 2])
                .show_rows(ui, row_height, total, |ui, row_range| {
                    for row_idx in row_range {
                        if row_idx == 0 && show_default {
                            let is_selected = self.edit_font_family.is_empty();
                            let resp = ui.selectable_label(is_selected, "Default");
                            if resp.clicked() && !is_selected {
                                self.edit_font_family.clear();
                                actions.push(ConfigAction::FontFamilyChanged(String::new()));
                                self.has_changes = true;
                            }
                            continue;
                        }
                        let font_idx = if show_default { row_idx - 1 } else { row_idx };
                        if let Some(font_name) = matched_fonts.get(font_idx) {
                            let is_selected = self.edit_font_family == **font_name;
                            let label = RichText::new(font_name.as_str());
                            let resp = ui.selectable_label(is_selected, label);
                            if resp.clicked() && !is_selected {
                                self.edit_font_family = (*font_name).clone();
                                actions.push(ConfigAction::FontFamilyChanged(self.edit_font_family.clone()));
                                self.has_changes = true;
                            }
                        }
                    }
                });
        }

        // Warning if font not found
        if !self.edit_font_family.is_empty()
            && !self.monospace_fonts.iter().any(|f| f == &self.edit_font_family)
            && !self.all_fonts.iter().any(|f| f == &self.edit_font_family)
        {
            ui.colored_label(Color32::YELLOW, "⚠ Font not found in system");
        }

        ui.add_space(2.0);
        ui.label(
            RichText::new("Note: font change requires restart")
                .size(10.0)
                .color(Color32::from_rgb(140, 140, 140)),
        );
    }

    fn render_appearance_tab(&mut self, ui: &mut egui::Ui, actions: &mut Vec<ConfigAction>) {
        ui.label(RichText::new("Appearance Settings").strong().size(14.0));
        ui.separator();

        // Theme ComboBox
        ui.horizontal(|ui| {
            ui.label("Theme:");
            if egui::ComboBox::from_label("")
                .selected_text(self.edit_theme.clone())
                .show_ui(ui, |ui| {
                    let mut changed = false;
                    for theme in &self.available_themes {
                        if ui.selectable_value(&mut self.edit_theme, theme.clone(), theme).changed() {
                            changed = true;
                        }
                    }
                    changed
                })
                .inner
                .unwrap_or(false)
            {
                actions.push(ConfigAction::ThemeChanged(self.edit_theme.clone()));
                self.has_changes = true;
            }
        });

        ui.separator();

        // Padding slider
        ui.horizontal(|ui| {
            ui.label("Padding:");
            if ui
                .add(
                    egui::Slider::new(&mut self.edit_padding, 0.0..=20.0)
                        .step_by(0.5)
                        .show_value(true),
                )
                .changed()
            {
                actions.push(ConfigAction::PaddingChanged(self.edit_padding));
                self.has_changes = true;
            }
            ui.label("px");
        });
    }

    fn render_advanced_tab(&mut self, ui: &mut egui::Ui, actions: &mut Vec<ConfigAction>) {
        ui.label(RichText::new("Advanced Settings").strong().size(14.0));
        ui.separator();

        // Scrollback lines slider (logarithmic for large range)
        ui.horizontal(|ui| {
            ui.label("Scrollback Lines:");
            if ui
                .add(
                    egui::Slider::new(&mut self.edit_scrollback_lines, 100..=100_000)
                        .logarithmic(true)
                        .show_value(true),
                )
                .changed()
            {
                actions.push(ConfigAction::ScrollbackLinesChanged(self.edit_scrollback_lines));
                self.has_changes = true;
            }
        });

        ui.separator();

        // Restore session checkbox
        if ui
            .checkbox(&mut self.edit_restore_session, "Restore sessions on startup")
            .changed()
        {
            self.has_changes = true;
        }

        ui.separator();

        // Debug overlay toggle
        if ui
            .checkbox(&mut self.edit_debug_overlay, "Show debug overlay (F12)")
            .changed()
        {
            actions.push(ConfigAction::DebugPanelToggled(self.edit_debug_overlay));
        }
    }
}

