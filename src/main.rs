mod color;
mod debug;
mod terminal;
mod ui;
mod clipboard;
mod pty;
mod shell;
mod config;
mod session;
mod session_manager;
mod search;
mod link;
mod keybindings;
mod command_palette;

use eframe::egui;
use std::sync::Arc;
use std::time::Duration;
use terminal::TerminalState;
use ui::TerminalRenderer;
use clipboard::ClipboardManager;
use parking_lot::Mutex as ParkingMutex;
use shell::{ShellSession, ShellEvent};
use session_manager::SessionManager;
use session::Session;

fn main() -> Result<(), eframe::Error> {
    // Load configuration
    let cfg = config::Config::load();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([cfg.initial_width, cfg.initial_height]),
        ..Default::default()
    };

    let cfg = std::sync::Arc::new(cfg);

    eframe::run_native(
        "Terminal Emulator",
        options,
        Box::new(move |cc| {
            let cfg_clone = cfg.clone();
            let mut fonts = egui::FontDefinitions::default();

            // Try to load system CJK fonts
            let cjk_font_paths = [
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJKsc-Regular.otf",
                "/usr/share/fonts/wenquanyi/wqy-zenhei.ttc",
            ];

            for path in &cjk_font_paths {
                if let Ok(font_data) = std::fs::read(path) {
                    fonts.font_data.insert(
                        "cjk".to_owned(),
                        std::sync::Arc::new(egui::FontData::from_owned(font_data)),
                    );
                    // 添加到所有字体族，确保 TextEdit 也能显示
                    fonts.families
                        .get_mut(&egui::FontFamily::Monospace)
                        .unwrap()
                        .push("cjk".to_owned());
                    fonts.families
                        .get_mut(&egui::FontFamily::Proportional)
                        .unwrap()
                        .push("cjk".to_owned());
                    break;
                }
            }

            cc.egui_ctx.set_fonts(fonts);

            // 设置暗色主题，避免浮夸的亮色背景
            let mut visuals = egui::Visuals::dark();
            visuals.window_fill = egui::Color32::from_rgb(29, 29, 29);
            visuals.panel_fill = egui::Color32::from_rgb(29, 29, 29);
            visuals.extreme_bg_color = egui::Color32::from_rgb(20, 20, 20);
            cc.egui_ctx.set_visuals(visuals);

            Ok(Box::new(TerminalApp::new(&cfg_clone, cc.egui_ctx.clone())))
        }),
    )
}

struct TerminalApp {
    session_manager: SessionManager,
    renderer: TerminalRenderer,
    input_queue: Arc<ParkingMutex<Vec<u8>>>,
    clipboard: Option<ClipboardManager>,
    cols: usize,
    rows: usize,
    last_cursor_blink: std::time::Instant,
    cursor_visible: bool,
    status_message: String,
    last_window_title: String,
    // Tab UI state
    hovered_tab_index: Option<usize>,
    dragging_tab: Option<usize>,
    drag_start_pos: Option<f32>,
    current_mouse_x: f32,
    // Search state
    search_state: search::SearchState,
    // Link detection
    link_detector: link::LinkDetector,
    hovered_link: Option<link::Link>,
    // Keybindings
    keybindings: keybindings::KeyBindings,
    // Command palette
    command_palette: command_palette::CommandPalette,
}

fn should_restore_terminal_shortcut_event(ctx: &egui::Context, modifiers: egui::Modifiers) -> bool {
    !ctx.text_edit_focused() && modifiers.command && !modifiers.alt
}

fn shortcut_event_to_key_event(event: egui::Event, modifiers: egui::Modifiers) -> Option<egui::Event> {
    let key = match event {
        egui::Event::Copy => egui::Key::C,
        egui::Event::Cut => egui::Key::X,
        egui::Event::Paste(_) => egui::Key::V,
        _ => return None,
    };

    Some(egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    })
}

fn normalize_terminal_shortcut_events(
    events: &mut Vec<egui::Event>,
    modifiers: egui::Modifiers,
    restore_shortcuts: bool,
) {
    let mut normalized_events = Vec::with_capacity(events.len());

    for event in events.drain(..) {
        if restore_shortcuts {
            if let Some(key_event) = shortcut_event_to_key_event(event.clone(), modifiers) {
                normalized_events.push(key_event);
                continue;
            }
        }

        if !matches!(event, egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)) {
            normalized_events.push(event);
        }
    }

    *events = normalized_events;
}

/// 将 egui::Key 转换为字符串表示
fn key_to_string(key: egui::Key) -> Option<String> {
    match key {
        egui::Key::Enter => Some("return".to_string()),
        egui::Key::Escape => Some("escape".to_string()),
        egui::Key::Backspace => Some("backspace".to_string()),
        egui::Key::Tab => Some("tab".to_string()),
        egui::Key::ArrowUp => Some("up".to_string()),
        egui::Key::ArrowDown => Some("down".to_string()),
        egui::Key::ArrowLeft => Some("left".to_string()),
        egui::Key::ArrowRight => Some("right".to_string()),
        egui::Key::Home => Some("home".to_string()),
        egui::Key::End => Some("end".to_string()),
        egui::Key::Insert => Some("insert".to_string()),
        egui::Key::Delete => Some("delete".to_string()),
        egui::Key::PageUp => Some("pageup".to_string()),
        egui::Key::PageDown => Some("pagedown".to_string()),
        egui::Key::F1 => Some("f1".to_string()),
        egui::Key::F2 => Some("f2".to_string()),
        egui::Key::F3 => Some("f3".to_string()),
        egui::Key::F4 => Some("f4".to_string()),
        egui::Key::F5 => Some("f5".to_string()),
        egui::Key::F6 => Some("f6".to_string()),
        egui::Key::F7 => Some("f7".to_string()),
        egui::Key::F8 => Some("f8".to_string()),
        egui::Key::F9 => Some("f9".to_string()),
        egui::Key::F10 => Some("f10".to_string()),
        egui::Key::F11 => Some("f11".to_string()),
        egui::Key::F12 => Some("f12".to_string()),
        egui::Key::A => Some("a".to_string()),
        egui::Key::B => Some("b".to_string()),
        egui::Key::C => Some("c".to_string()),
        egui::Key::D => Some("d".to_string()),
        egui::Key::E => Some("e".to_string()),
        egui::Key::F => Some("f".to_string()),
        egui::Key::G => Some("g".to_string()),
        egui::Key::H => Some("h".to_string()),
        egui::Key::I => Some("i".to_string()),
        egui::Key::J => Some("j".to_string()),
        egui::Key::K => Some("k".to_string()),
        egui::Key::L => Some("l".to_string()),
        egui::Key::M => Some("m".to_string()),
        egui::Key::N => Some("n".to_string()),
        egui::Key::O => Some("o".to_string()),
        egui::Key::P => Some("p".to_string()),
        egui::Key::Q => Some("q".to_string()),
        egui::Key::R => Some("r".to_string()),
        egui::Key::S => Some("s".to_string()),
        egui::Key::T => Some("t".to_string()),
        egui::Key::U => Some("u".to_string()),
        egui::Key::V => Some("v".to_string()),
        egui::Key::W => Some("w".to_string()),
        egui::Key::X => Some("x".to_string()),
        egui::Key::Y => Some("y".to_string()),
        egui::Key::Z => Some("z".to_string()),
        egui::Key::Num0 => Some("0".to_string()),
        egui::Key::Num1 => Some("1".to_string()),
        egui::Key::Num2 => Some("2".to_string()),
        egui::Key::Num3 => Some("3".to_string()),
        egui::Key::Num4 => Some("4".to_string()),
        egui::Key::Num5 => Some("5".to_string()),
        egui::Key::Num6 => Some("6".to_string()),
        egui::Key::Num7 => Some("7".to_string()),
        egui::Key::Num8 => Some("8".to_string()),
        egui::Key::Num9 => Some("9".to_string()),
        _ => None,
    }
}

/// 从 egui 的 Key 和 Modifiers 构建快捷键字符串（用于查询快捷键配置）
fn build_keybinding_string(key: egui::Key, modifiers: egui::Modifiers) -> Option<String> {
    let key_str = key_to_string(key)?;
    let mut parts = Vec::new();

    if modifiers.ctrl {
        parts.push("ctrl");
    }
    if modifiers.shift {
        parts.push("shift");
    }
    if modifiers.alt {
        parts.push("alt");
    }
    if modifiers.mac_cmd || modifiers.command_only() {
        parts.push("super");
    }

    parts.push(&key_str);
    Some(parts.join("+"))
}

impl TerminalApp {
    fn new(cfg: &config::Config, repaint_ctx: egui::Context) -> Self {
        let cols = cfg.cols;
        let rows = cfg.rows;

        // 创建首个会话
        let terminal = TerminalState::new(cols, rows);

        // 尝试启动 shell
        let (shell, _) = match ShellSession::new(cols, rows, repaint_ctx.clone()) {
            Ok(session) => {
                eprintln!("✓ Shell session started successfully");
                (Some(session), Some(()))
            }
            Err(e) => {
                eprintln!("✗ Failed to start shell: {}", e);
                (None, None)
            }
        };

        let session = if let Some(shell) = shell {
            Session::with_default_name(0, Arc::new(ParkingMutex::new(terminal)), shell)
        } else {
            // 创建一个没有 shell 的 dummy session（应该很少见）
            let dummy_shell = ShellSession::new(cols, rows, repaint_ctx.clone()).unwrap_or_else(|e| {
                panic!("Cannot create even a dummy shell session: {}", e)
            });
            Session::with_default_name(0, Arc::new(ParkingMutex::new(terminal)), dummy_shell)
        };

        let session_manager = SessionManager::new(session, repaint_ctx);

        let renderer = TerminalRenderer::new(
            cfg.font_size,
            cfg.padding,
            cfg.scrollbar_visibility.clone(),
        );
        let clipboard = ClipboardManager::new().ok();

        TerminalApp {
            session_manager,
            input_queue: Arc::new(ParkingMutex::new(Vec::new())),
            renderer,
            clipboard,
            cols,
            rows,
            last_cursor_blink: std::time::Instant::now(),
            cursor_visible: true,
            status_message: String::new(),
            last_window_title: String::new(),
            hovered_tab_index: None,
            dragging_tab: None,
            drag_start_pos: None,
            current_mouse_x: 0.0,
            search_state: search::SearchState::new(),
            link_detector: link::LinkDetector::new(link::LinkDetectionConfig::default()),
            hovered_link: None,
            keybindings: keybindings::KeyBindings::load().unwrap_or_default(),
            command_palette: command_palette::CommandPalette::new(),
        }
    }

    #[allow(deprecated)]
    fn render_ui(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::NONE
            .inner_margin(0.0);

        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| {
                // 渲染会话标签栏
                let tab_height = 30.0;
                let close_btn_size = 14.0;

                // Tab 栏 - 绘制标签和按钮
                {
                    let tab_rect = egui::Rect::from_min_size(
                        ui.cursor().left_top(),
                        egui::vec2(ui.available_width(), tab_height),
                    );

                    let painter = ui.painter();

                    // 背景
                    painter.rect_filled(tab_rect, 0.0, egui::Color32::from_rgb(40, 40, 40));

                    // 检测悬停位置（在绘制之前）
                    let hover_pos = ctx.input(|i| i.pointer.hover_pos());
                    self.hovered_tab_index = None;

                    // 更新当前鼠标x位置（用于拖拽动画）
                    if let Some(pos) = hover_pos {
                        self.current_mouse_x = pos.x;
                    }

                    // 检测鼠标释放（点击完成或拖拽结束）
                    let mouse_released = ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
                    let mouse_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));

                    // 处理拖拽结束或点击
                    if mouse_released {
                        if let Some(hover_pos) = hover_pos {
                            if let Some(from_idx) = self.dragging_tab {
                                // 拖拽结束 - 计算drop目标并执行重排
                                if tab_rect.contains(hover_pos) {
                                    let relative_x = hover_pos.x - tab_rect.left();
                                    let mut x_offset = 5.0;
                                    let sessions_count = self.session_manager.sessions().len();
                                    let mut target_idx = from_idx;

                                    for idx in 0..sessions_count {
                                        let session = &self.session_manager.sessions()[idx];
                                        let galley = painter.layout_no_wrap(
                                            session.metadata.name.clone(),
                                            egui::FontId::monospace(12.0),
                                            egui::Color32::WHITE,
                                        );
                                        let tab_width = galley.rect.width() + 20.0 + close_btn_size + 4.0;

                                        if relative_x >= x_offset && relative_x < x_offset + tab_width {
                                            target_idx = idx;
                                            break;
                                        }

                                        x_offset += tab_width + 2.0;
                                        if x_offset > tab_rect.right() - tab_rect.left() - 50.0 {
                                            break;
                                        }
                                    }

                                    // 执行重排
                                    if target_idx != from_idx {
                                        self.session_manager.reorder_sessions(from_idx, target_idx);
                                    }
                                }
                                self.dragging_tab = None;
                                self.drag_start_pos = None;
                            } else if let Some(click_pos) = ctx.input(|i| i.pointer.latest_pos()) {
                                // 检查是否点击了Tab或按钮（拖拽未进行）
                                let mut x_offset = tab_rect.left() + 5.0;
                                for (idx, session) in self.session_manager.sessions().iter().enumerate() {
                                    let galley = painter.layout_no_wrap(
                                        session.metadata.name.clone(),
                                        egui::FontId::monospace(12.0),
                                        egui::Color32::WHITE,
                                    );
                                    let tab_width = galley.rect.width() + 20.0 + close_btn_size + 4.0;
                                    let tab_rect_item = egui::Rect::from_min_size(
                                        egui::pos2(x_offset, tab_rect.top() + 5.0),
                                        egui::vec2(tab_width, tab_height - 10.0),
                                    );

                                    let close_btn_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            tab_rect_item.right() - close_btn_size - 3.0,
                                            tab_rect_item.center().y - close_btn_size / 2.0,
                                        ),
                                        egui::vec2(close_btn_size, close_btn_size),
                                    );

                                    if close_btn_rect.contains(click_pos) && self.session_manager.len() > 1 {
                                        self.session_manager.close_session(idx);
                                        break;
                                    } else if tab_rect_item.contains(click_pos) {
                                        self.session_manager.switch_session(idx);
                                        break;
                                    }

                                    x_offset += tab_width + 2.0;
                                    if x_offset > tab_rect.right() - 50.0 {
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // 检测拖拽开始（鼠标按下且移动）
                    if mouse_pressed {
                        if let Some(press_pos) = ctx.input(|i| i.pointer.press_origin()) {
                            if self.dragging_tab.is_none() {
                                // 检查是否在某个Tab上按下
                                let mut x_offset = tab_rect.left() + 5.0;
                                for (idx, session) in self.session_manager.sessions().iter().enumerate() {
                                    let galley = painter.layout_no_wrap(
                                        session.metadata.name.clone(),
                                        egui::FontId::monospace(12.0),
                                        egui::Color32::WHITE,
                                    );
                                    let tab_width = galley.rect.width() + 20.0 + close_btn_size + 4.0;
                                    let tab_rect_item = egui::Rect::from_min_size(
                                        egui::pos2(x_offset, tab_rect.top() + 5.0),
                                        egui::vec2(tab_width, tab_height - 10.0),
                                    );

                                    let close_btn_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            tab_rect_item.right() - close_btn_size - 3.0,
                                            tab_rect_item.center().y - close_btn_size / 2.0,
                                        ),
                                        egui::vec2(close_btn_size, close_btn_size),
                                    );

                                    if tab_rect_item.contains(press_pos) && !close_btn_rect.contains(press_pos) {
                                        // 标记开始拖拽（但还没有移动足够距离）
                                        self.dragging_tab = Some(idx);
                                        self.drag_start_pos = Some(press_pos.x);
                                        break;
                                    }

                                    x_offset += tab_width + 2.0;
                                    if x_offset > tab_rect.right() - 50.0 {
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // 检测拖拽过程中的移动 - 如果有拖拽且鼠标移动超过阈值，就认为正在拖拽
                    let is_actively_dragging = if let (Some(_), Some(start_x)) = (self.dragging_tab, self.drag_start_pos) {
                        if let Some(current_pos) = ctx.input(|i| i.pointer.latest_pos()) {
                            let distance = (current_pos.x - start_x).abs();
                            distance > 5.0  // 5px拖拽阈值
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    // 计算拖拽过程中的动画效果
                    let mut drag_target_idx: Option<usize> = None;
                    if is_actively_dragging {
                        if let Some(hover_pos) = hover_pos {
                            if let Some(_from_idx) = self.dragging_tab {
                                let relative_x = hover_pos.x - tab_rect.left();
                                let mut x_offset = 5.0;
                                let sessions_count = self.session_manager.sessions().len();

                                for idx in 0..sessions_count {
                                    let session = &self.session_manager.sessions()[idx];
                                    let galley = painter.layout_no_wrap(
                                        session.metadata.name.clone(),
                                        egui::FontId::monospace(12.0),
                                        egui::Color32::WHITE,
                                    );
                                    let tab_width = galley.rect.width() + 20.0 + close_btn_size + 4.0;

                                    if relative_x >= x_offset && relative_x < x_offset + tab_width {
                                        drag_target_idx = Some(idx);
                                        break;
                                    }

                                    x_offset += tab_width + 2.0;
                                    if x_offset > tab_rect.right() - tab_rect.left() - 50.0 {
                                        break;
                                    }
                                }
                            }
                        }
                        // 请求持续重绘以显示动画
                        ctx.request_repaint();
                    }

                    let mut x_offset = tab_rect.left() + 5.0;
                    let active_idx = self.session_manager.active_index();

                    // 预先收集会话信息，避免借用冲突
                    let sessions_info: Vec<_> = self.session_manager.sessions()
                        .iter()
                        .enumerate()
                        .map(|(idx, session)| (idx, session.metadata.name.clone()))
                        .collect();

                    // 绘制每个标签
                    for (idx, tab_text) in &sessions_info {
                        let galley = painter.layout_no_wrap(
                            tab_text.clone(),
                            egui::FontId::monospace(12.0),
                            egui::Color32::WHITE,
                        );

                        let tab_width = galley.rect.width() + 20.0 + close_btn_size + 4.0;
                        let mut tab_rect_item = egui::Rect::from_min_size(
                            egui::pos2(x_offset, tab_rect.top() + 5.0),
                            egui::vec2(tab_width, tab_height - 10.0),
                        );

                        let is_active = *idx == active_idx;
                        let is_dragging = self.dragging_tab == Some(*idx);
                        let is_drag_target = drag_target_idx == Some(*idx);

                        // 计算拖拽过程中的动画位移
                        if is_actively_dragging {
                            if is_dragging {
                                // 被拖拽的Tab跟随鼠标移动
                                if let Some(start_x) = self.drag_start_pos {
                                    let offset = self.current_mouse_x - start_x;
                                    tab_rect_item = tab_rect_item.translate(egui::vec2(offset, 0.0));
                                }
                            } else if let Some(from_idx) = self.dragging_tab {
                                // 其他Tabs根据拖拽目标位置进行动画插入
                                let drag_to_left = is_drag_target && drag_target_idx.map(|t| t < from_idx).unwrap_or(false);
                                let drag_to_right = is_drag_target && drag_target_idx.map(|t| t > from_idx).unwrap_or(false);

                                if drag_to_left {
                                    // 目标在左边，右侧的tabs应该向右推移
                                    if *idx > from_idx {
                                        let push_offset = tab_width + 2.0;
                                        tab_rect_item = tab_rect_item.translate(egui::vec2(push_offset, 0.0));
                                    }
                                } else if drag_to_right {
                                    // 目标在右边，左侧的tabs应该向左推移
                                    if *idx < from_idx {
                                        let push_offset = -(tab_width + 2.0);
                                        tab_rect_item = tab_rect_item.translate(egui::vec2(push_offset, 0.0));
                                    }
                                }
                            }
                        }

                        // 检测悬停
                        let is_hovered = if let Some(hover_pos) = hover_pos {
                            tab_rect_item.contains(hover_pos)
                        } else {
                            false
                        };

                        if is_hovered && !is_actively_dragging {
                            self.hovered_tab_index = Some(*idx);
                        }

                        // 背景色：激活、拖拽中或悬停时改变
                        let mut bg_color = if is_active {
                            egui::Color32::from_rgb(70, 70, 80)
                        } else {
                            egui::Color32::from_rgb(50, 50, 60)
                        };

                        if is_hovered || is_dragging {
                            // 增亮
                            bg_color = egui::Color32::from_rgb(
                                (bg_color.r() + 25).min(255),
                                (bg_color.g() + 25).min(255),
                                (bg_color.b() + 25).min(255),
                            );
                        }

                        // 绘制Tab背景和边框
                        if is_dragging && is_actively_dragging {
                            // 拖拽中的Tab：半透明+虚线边框
                            painter.rect_filled(tab_rect_item, 1.0, egui::Color32::from_rgba_premultiplied(
                                bg_color.r(), bg_color.g(), bg_color.b(), 140
                            ));
                            // 虚线边框表示拖拽中
                            painter.hline(
                                tab_rect_item.left()..=tab_rect_item.right(),
                                tab_rect_item.top(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 150, 160)),
                            );
                            painter.hline(
                                tab_rect_item.left()..=tab_rect_item.right(),
                                tab_rect_item.bottom(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 150, 160)),
                            );
                            painter.vline(
                                tab_rect_item.left(),
                                tab_rect_item.top()..=tab_rect_item.bottom(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 150, 160)),
                            );
                            painter.vline(
                                tab_rect_item.right(),
                                tab_rect_item.top()..=tab_rect_item.bottom(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 150, 160)),
                            );
                        } else {
                            painter.rect_filled(tab_rect_item, 1.0, bg_color);
                            // 绘制边框线
                            painter.hline(
                                tab_rect_item.left()..=tab_rect_item.right(),
                                tab_rect_item.top(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                            );
                            painter.hline(
                                tab_rect_item.left()..=tab_rect_item.right(),
                                tab_rect_item.bottom(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                            );
                            painter.vline(
                                tab_rect_item.left(),
                                tab_rect_item.top()..=tab_rect_item.bottom(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                            );
                            painter.vline(
                                tab_rect_item.right(),
                                tab_rect_item.top()..=tab_rect_item.bottom(),
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                            );

                            // 拖拽过程中，在目标Tab位置显示插入指示线
                            if is_drag_target && is_actively_dragging {
                                let insert_line_x = if self.current_mouse_x - tab_rect_item.center().x < 0.0 {
                                    tab_rect_item.left()
                                } else {
                                    tab_rect_item.right()
                                };
                                painter.vline(
                                    insert_line_x,
                                    tab_rect_item.top()..=tab_rect_item.bottom(),
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255)),
                                );
                            }
                        }

                        // 绘制文本
                        painter.text(
                            egui::pos2(
                                tab_rect_item.left() + 10.0,
                                tab_rect_item.center().y,
                            ),
                            egui::Align2::LEFT_CENTER,
                            tab_text,
                            egui::FontId::monospace(12.0),
                            if is_active {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(180, 180, 190)
                            },
                        );

                        // 绘制关闭按钮
                        let close_btn_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                tab_rect_item.right() - close_btn_size - 3.0,
                                tab_rect_item.center().y - close_btn_size / 2.0,
                            ),
                            egui::vec2(close_btn_size, close_btn_size),
                        );

                        let close_btn_hovered = if let Some(hover_pos) = hover_pos {
                            close_btn_rect.contains(hover_pos)
                        } else {
                            false
                        };

                        // 绘制关闭按钮背景（悬停时显示）
                        if close_btn_hovered && !is_dragging {
                            painter.circle_filled(close_btn_rect.center(), close_btn_size / 2.0 + 2.0, egui::Color32::from_rgb(100, 50, 50));
                        }

                        // 绘制X符号
                        let close_x_color = if close_btn_hovered && !is_dragging {
                            egui::Color32::from_rgb(255, 150, 150)
                        } else {
                            egui::Color32::from_rgb(150, 150, 150)
                        };

                        let cross_offset = close_btn_size / 3.0;
                        painter.line_segment(
                            [
                                egui::pos2(close_btn_rect.center().x - cross_offset, close_btn_rect.center().y - cross_offset),
                                egui::pos2(close_btn_rect.center().x + cross_offset, close_btn_rect.center().y + cross_offset),
                            ],
                            egui::Stroke::new(1.5, close_x_color),
                        );
                        painter.line_segment(
                            [
                                egui::pos2(close_btn_rect.center().x + cross_offset, close_btn_rect.center().y - cross_offset),
                                egui::pos2(close_btn_rect.center().x - cross_offset, close_btn_rect.center().y + cross_offset),
                            ],
                            egui::Stroke::new(1.5, close_x_color),
                        );

                        x_offset += tab_width + 2.0;

                        // 防止超出屏幕
                        if x_offset > tab_rect.right() - 50.0 {
                            break;
                        }
                    }

                    // "+" 按钮 - 新建会话
                    let plus_btn_rect = egui::Rect::from_min_size(
                        egui::pos2(tab_rect.right() - 30.0, tab_rect.top() + 5.0),
                        egui::vec2(25.0, tab_height - 10.0),
                    );

                    // 检测"+"按钮悬停
                    let plus_btn_hovered = if let Some(hover_pos) = hover_pos {
                        plus_btn_rect.contains(hover_pos)
                    } else {
                        false
                    };

                    let plus_btn_color = if plus_btn_hovered {
                        egui::Color32::from_rgb(75, 75, 85)
                    } else {
                        egui::Color32::from_rgb(50, 50, 60)
                    };

                    painter.rect_filled(plus_btn_rect, 1.0, plus_btn_color);
                    // 绘制边框
                    painter.hline(
                        plus_btn_rect.left()..=plus_btn_rect.right(),
                        plus_btn_rect.top(),
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                    );
                    painter.hline(
                        plus_btn_rect.left()..=plus_btn_rect.right(),
                        plus_btn_rect.bottom(),
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                    );
                    painter.vline(
                        plus_btn_rect.left(),
                        plus_btn_rect.top()..=plus_btn_rect.bottom(),
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                    );
                    painter.vline(
                        plus_btn_rect.right(),
                        plus_btn_rect.top()..=plus_btn_rect.bottom(),
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 130)),
                    );

                    let plus_text_color = if plus_btn_hovered {
                        egui::Color32::from_rgb(220, 220, 220)
                    } else {
                        egui::Color32::from_rgb(180, 180, 190)
                    };

                    painter.text(
                        plus_btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "+",
                        egui::FontId::monospace(14.0),
                        plus_text_color,
                    );

                    // 检测 "+" 按钮点击（在鼠标释放时）
                    if mouse_released {
                        if let Some(click_pos) = ctx.input(|i| i.pointer.latest_pos()) {
                            if plus_btn_rect.contains(click_pos) {
                                self.session_manager.new_session(None, None);
                            }
                        }
                    }

                    // 向下移动光标
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), tab_height), egui::Sense::hover());
                }

                // 终端显示区域
                self.renderer.sync_font_metrics(ctx);
                let (cols, rows) = self.renderer.grid_dimensions(ui.available_size());

                if cols != self.cols || rows != self.rows {
                    let session = self.session_manager.get_active_session_mut();
                    let _ = session.shell.resize(cols, rows);
                    let mut terminal = session.terminal.lock();
                    terminal.on_resize(cols, rows);
                    self.cols = cols;
                    self.rows = rows;
                }

                let session = self.session_manager.get_active_session_mut();
                let mut terminal_guard = session.terminal.lock();

                // 获取链接列表用于渲染
                let links = self.link_detector.detect_all_links(&terminal_guard.grid);
                self.renderer.render(ui, &mut terminal_guard, self.cursor_visible, &self.search_state, &links, &self.hovered_link);
            });

        // 搜索面板 UI（浮动窗口，右上角）
        if self.search_state.is_open {
            egui::Window::new("Search")
                .title_bar(false)
                .resizable(false)
                .default_pos(egui::pos2(ctx.available_rect().right() - 350.0, 60.0))
                .default_size([340.0, 50.0])
                .fixed_size([340.0, 50.0])
                .frame(egui::Frame {
                    fill: egui::Color32::from_rgb(40, 40, 40),
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        // 搜索输入框
                        ui.label("Search:");
                        let search_response = ui.text_edit_singleline(&mut self.search_state.query);

                        // 自动 focus 搜索框
                        if self.search_state.search_focused {
                            ui.memory_mut(|mem| mem.request_focus(search_response.id));
                            self.search_state.search_focused = false;
                        }

                        if search_response.changed() {
                            // 重新搜索
                            let session = self.session_manager.get_active_session_mut();
                            let terminal = session.terminal.lock();
                            let (matches, error) = search::SearchEngine::search(
                                &terminal.grid,
                                &self.search_state.query,
                                self.search_state.use_regex,
                                self.search_state.case_sensitive,
                            );
                            drop(terminal);
                            self.search_state.matches = matches;
                            self.search_state.error_message = error;
                            self.search_state.current_match_index = 0;
                        }

                        // 显示匹配计数
                        if !self.search_state.matches.is_empty() {
                            ui.label(format!(
                                "{}/{}",
                                self.search_state.current_match_index + 1,
                                self.search_state.matches.len()
                            ));
                        } else if !self.search_state.query.is_empty() {
                            ui.label("No matches");
                        }

                        // 上一个/下一个 按钮
                        if ui.button("↑").clicked() {
                            self.search_state.prev_match();
                        }
                        if ui.button("↓").clicked() {
                            self.search_state.next_match();
                        }

                        // 关闭按钮
                        if ui.button("✕").clicked() {
                            self.search_state.close();
                        }
                    });

                    // 显示错误信息（如正则表达式错误）
                    if let Some(error) = &self.search_state.error_message {
                        ui.label(egui::RichText::new(error).color(egui::Color32::RED));
                    }
                });
        }

        // 命令调色板 UI（中央弹窗）
        if self.command_palette.is_open {
            let screen_rect = ctx.screen_rect();
            let palette_width = 600.0;
            let palette_height = 400.0;
            let palette_pos = egui::pos2(
                (screen_rect.width() - palette_width) / 2.0,
                (screen_rect.height() - palette_height) / 3.0,
            );

            egui::Window::new("Command Palette")
                .title_bar(false)
                .resizable(false)
                .movable(false)
                .default_pos(palette_pos)
                .default_size([palette_width, palette_height])
                .fixed_size([palette_width, palette_height])
                .frame(egui::Frame {
                    fill: egui::Color32::from_rgb(40, 40, 40),
                    stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 100)),
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    // 搜索输入框
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        let search_response = ui.text_edit_singleline(&mut self.command_palette.search_query);
                        if search_response.changed() {
                            self.command_palette.update_search_results();
                        }
                        if search_response.has_focus() && self.command_palette.search_query.is_empty() {
                            ui.label("Search commands...");
                        }
                    });

                    ui.separator();

                    // 命令列表
                    let results = self.command_palette.get_results();
                    let max_visible = self.command_palette.max_visible_results();

                    egui::ScrollArea::vertical()
                        .max_height(palette_height - 100.0)
                        .show(ui, |ui| {
                            for (idx, (cmd_info, _score)) in results.iter().take(max_visible).enumerate() {
                                let is_selected = idx == self.command_palette.selected_index;

                                let bg_color = if is_selected {
                                    egui::Color32::from_rgb(70, 70, 80)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };

                                let item_rect = ui.available_rect_before_wrap();
                                ui.painter().rect_filled(item_rect, 2.0, bg_color);

                                ui.horizontal(|ui| {
                                    // 分类标签
                                    let category_color = match cmd_info.category {
                                        command_palette::CommandCategory::Session => egui::Color32::from_rgb(100, 150, 255),
                                        command_palette::CommandCategory::Edit => egui::Color32::from_rgb(100, 200, 100),
                                        command_palette::CommandCategory::Search => egui::Color32::from_rgb(255, 200, 100),
                                        command_palette::CommandCategory::Terminal => egui::Color32::from_rgb(150, 150, 255),
                                        command_palette::CommandCategory::Window => egui::Color32::from_rgb(200, 100, 200),
                                    };

                                    ui.colored_label(category_color, format!("[{}]", cmd_info.category));

                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&cmd_info.name).strong());
                                        ui.label(
                                            egui::RichText::new(&cmd_info.description)
                                                .size(10.0)
                                                .color(egui::Color32::from_rgb(150, 150, 150)),
                                        );
                                    });

                                    // 快捷键显示
                                    let keybinding_str = self
                                        .keybindings
                                        .bindings
                                        .iter()
                                        .find(|(_, cmd)| {
                                            if let Ok(parsed_cmd) = cmd.parse::<keybindings::Command>() {
                                                parsed_cmd == cmd_info.command
                                            } else {
                                                false
                                            }
                                        })
                                        .map(|(binding, _)| binding.clone())
                                        .unwrap_or_else(|| "No binding".to_string());

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(keybinding_str)
                                                    .size(10.0)
                                                    .color(egui::Color32::from_rgb(100, 150, 200)),
                                            );
                                        },
                                    );
                                });

                                ui.separator();
                            }

                            // 如果没有结果
                            if results.is_empty() {
                                ui.label(
                                    egui::RichText::new("No commands found")
                                        .color(egui::Color32::from_rgb(150, 150, 150)),
                                );
                            }
                        });

                    // 底部提示
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("↑↓ Navigate  Enter Execute  Esc Cancel")
                                .size(10.0)
                                .color(egui::Color32::from_rgb(100, 100, 100)),
                        );
                    });
                });
        }
    }
}

impl eframe::App for TerminalApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // UI handled in update()
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // egui-winit turns Ctrl/Cmd+C/X/V into semantic clipboard events and skips the
        // corresponding Key press. Restore those as Key events so the terminal can receive
        // control bytes, while still preventing egui's default text-edit shortcut behavior.
        let restore_shortcuts = should_restore_terminal_shortcut_event(ctx, raw_input.modifiers);
        normalize_terminal_shortcut_events(&mut raw_input.events, raw_input.modifiers, restore_shortcuts);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let active_session_idx = self.session_manager.active_index();
        let session = self.session_manager.get_active_session_mut();

        // Step 1: 处理 IME 事件
        let all_events = ctx.input(|i| i.events.clone());
        let mut saw_ime_event = false;
        for evt in &all_events {
            if let egui::Event::Ime(ime_event) = evt {
                saw_ime_event = true;
                let mut terminal = session.terminal.lock();
                match ime_event {
                    egui::ImeEvent::Enabled => {
                        crate::debug_log!("[IME] Enabled");
                        terminal.ime_enabled = true;
                    }
                    egui::ImeEvent::Preedit(text) => {
                        crate::debug_log!("[IME] Preedit: {:?}", text);
                        terminal.set_preedit(text.clone(), text.len());
                    }
                    egui::ImeEvent::Commit(text) => {
                        crate::debug_log!("[IME] Commit: {:?}", text);
                        terminal.clear_preedit();
                        if !text.is_empty() {
                            let _ = session.shell.write(text.as_bytes());
                        }
                        terminal.ime_enabled = false;
                    }
                    egui::ImeEvent::Disabled => {
                        crate::debug_log!("[IME] Disabled");
                        terminal.ime_enabled = false;
                        terminal.clear_preedit();
                    }
                }
            }
        }

        let window_title = {
            let terminal = session.terminal.lock();
            terminal.window_title.clone()
        };
        if !window_title.is_empty() && window_title != self.last_window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title));
            self.last_window_title = {
                let terminal = session.terminal.lock();
                terminal.window_title.clone()
            };
        }

        // Step 2: 处理快捷键 - 使用可配置的快捷键系统

        // 命令调色板快捷键 (Ctrl+Shift+P)
        if ctx.input(|i| i.key_pressed(egui::Key::P) && i.modifiers.ctrl && i.modifiers.shift) {
            self.command_palette.open();
        }

        // 当命令调色板打开时，处理其事件
        if self.command_palette.is_open {
            let all_events = ctx.input(|i| i.events.clone());

            for evt in &all_events {
                match evt {
                    egui::Event::Key { key, modifiers: _, pressed, .. } if *pressed => {
                        match key {
                            egui::Key::Escape => {
                                self.command_palette.close();
                            }
                            egui::Key::ArrowUp => {
                                self.command_palette.select_prev();
                            }
                            egui::Key::ArrowDown => {
                                self.command_palette.select_next();
                            }
                            egui::Key::Enter => {
                                if let Some(command) = self.command_palette.get_selected_command() {
                                    self.command_palette.execute_command(command.clone());
                                    self.command_palette.close();
                                    // 执行命令
                                    match command {
                                        keybindings::Command::SearchOpen => {
                                            self.search_state.toggle();
                                        }
                                        keybindings::Command::SearchClose => {
                                            self.search_state.close();
                                        }
                                        keybindings::Command::SessionNew => {
                                            let new_idx = self.session_manager.new_session(None, None);
                                            self.session_manager.switch_session(new_idx);
                                        }
                                        keybindings::Command::SessionClose | keybindings::Command::TerminalSendEof => {
                                            if self.session_manager.len() > 1 {
                                                let active_idx = self.session_manager.active_index();
                                                self.session_manager.close_session(active_idx);
                                            } else {
                                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                                return;
                                            }
                                        }
                                        keybindings::Command::SessionNext => {
                                            self.session_manager.switch_to_next_session();
                                        }
                                        keybindings::Command::SessionPrev => {
                                            self.session_manager.switch_to_prev_session();
                                        }
                                        keybindings::Command::SessionJump(n) => {
                                            if n < 9 {
                                                self.session_manager.switch_session(n);
                                            }
                                        }
                                        keybindings::Command::TerminalScrollUp => {
                                            let session = self.session_manager.get_active_session_mut();
                                            let mut terminal = session.terminal.lock();
                                            if !terminal.is_alt_buffer_active() {
                                                terminal.scroll(3);
                                            }
                                        }
                                        keybindings::Command::TerminalScrollDown => {
                                            let session = self.session_manager.get_active_session_mut();
                                            let mut terminal = session.terminal.lock();
                                            if !terminal.is_alt_buffer_active() {
                                                terminal.scroll(-3);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    egui::Event::Text(text) => {
                        if !text.is_empty() && *text != "\n" && *text != "\r" {
                            self.command_palette.search_query.push_str(text);
                            self.command_palette.update_search_results();
                        }
                    }
                    _ => {}
                }
            }

            // 如果调色板打开，不处理其他快捷键
            if self.command_palette.is_open {
                // 获取命令调色板选中的命令，但不执行（仅在按 Enter 时执行）
                // render_ui 中会显示调色板
                self.render_ui(ctx);
                return;
            }
        }

        // 收集所有按下的快捷键
        let pressed_keys: Vec<(egui::Key, egui::Modifiers)> = ctx.input(|i| {
            i.events.iter().filter_map(|evt| {
                if let egui::Event::Key { key, modifiers, pressed: true, .. } = evt {
                    Some((*key, *modifiers))
                } else {
                    None
                }
            }).collect()
        });

        // 处理每个按下的快捷键
        for (key, modifiers) in pressed_keys {
            if let Some(keybinding_str) = build_keybinding_string(key, modifiers) {
                if let Some(command) = self.keybindings.get_command(&keybinding_str) {
                    match command {
                        keybindings::Command::SearchOpen => {
                            self.search_state.toggle();
                        }
                        keybindings::Command::SearchClose => {
                            self.search_state.close();
                        }
                        keybindings::Command::SessionNew => {
                            let new_idx = self.session_manager.new_session(None, None);
                            self.session_manager.switch_session(new_idx);
                        }
                        keybindings::Command::SessionClose | keybindings::Command::TerminalSendEof => {
                            if self.session_manager.len() > 1 {
                                self.session_manager.close_session(active_session_idx);
                            } else {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                return;
                            }
                        }
                        keybindings::Command::SessionNext => {
                            self.session_manager.switch_to_next_session();
                        }
                        keybindings::Command::SessionPrev => {
                            self.session_manager.switch_to_prev_session();
                        }
                        keybindings::Command::SessionJump(n) => {
                            if n < 9 {
                                self.session_manager.switch_session(n);
                            }
                        }
                        keybindings::Command::TerminalScrollUp => {
                            let session = self.session_manager.get_active_session_mut();
                            let mut terminal = session.terminal.lock();
                            if !terminal.is_alt_buffer_active() {
                                terminal.scroll(3);
                            }
                        }
                        keybindings::Command::TerminalScrollDown => {
                            let session = self.session_manager.get_active_session_mut();
                            let mut terminal = session.terminal.lock();
                            if !terminal.is_alt_buffer_active() {
                                terminal.scroll(-3);
                            }
                        }
                        // 其他命令在下面处理
                        _ => {}
                    }
                }
            }
        }

        // Step 2: 会话管理快捷键
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl && i.modifiers.shift) {
            // Ctrl+Shift+T: 创建新会话并自动切换到它
            let new_idx = self.session_manager.new_session(None, None);
            self.session_manager.switch_session(new_idx);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::W) && i.modifiers.ctrl && !i.modifiers.shift) {
            // Ctrl+W: 关闭当前会话，如果是最后一个则关闭窗口
            if self.session_manager.len() > 1 {
                self.session_manager.close_session(active_session_idx);
            } else {
                // 关闭整个应用窗口
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl && !i.modifiers.shift) {
            // Ctrl+Tab: 下一个会话
            self.session_manager.switch_to_next_session();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl && i.modifiers.shift) {
            // Ctrl+Shift+Tab: 前一个会话
            self.session_manager.switch_to_prev_session();
        }

        // Ctrl+D 同 Ctrl+W 功能：关闭会话或窗口
        if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl && !i.modifiers.shift) {
            if self.session_manager.len() > 1 {
                self.session_manager.close_session(active_session_idx);
            } else {
                // 关闭整个应用窗口
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        // 数字快捷键：Ctrl+1..9 切换到对应会话
        for num in 1..=9 {
            let key = match num {
                1 => egui::Key::Num1,
                2 => egui::Key::Num2,
                3 => egui::Key::Num3,
                4 => egui::Key::Num4,
                5 => egui::Key::Num5,
                6 => egui::Key::Num6,
                7 => egui::Key::Num7,
                8 => egui::Key::Num8,
                9 => egui::Key::Num9,
                _ => continue,
            };
            if ctx.input(|i| i.key_pressed(key) && i.modifiers.ctrl) {
                self.session_manager.switch_session(num - 1);
            }
        }

        // Ctrl+PageUp: 前一个会话
        if ctx.input(|i| i.key_pressed(egui::Key::PageUp) && i.modifiers.ctrl) {
            self.session_manager.switch_to_prev_session();
        }

        // Ctrl+PageDown: 下一个会话
        if ctx.input(|i| i.key_pressed(egui::Key::PageDown) && i.modifiers.ctrl) {
            self.session_manager.switch_to_next_session();
        }

        // 获取当前活跃会话（在所有快捷键处理完后）
        let session = self.session_manager.get_active_session_mut();

        // Step 2.5: 搜索面板事件处理
        if self.search_state.is_open {
            let all_events = ctx.input(|i| i.events.clone());

            for evt in &all_events {
                match evt {
                    egui::Event::Key { key, modifiers, pressed, .. } if *pressed => {
                        match key {
                            egui::Key::Escape => {
                                self.search_state.close();
                            }
                            egui::Key::Enter => {
                                if !modifiers.shift {
                                    self.search_state.next_match();
                                } else {
                                    self.search_state.prev_match();
                                }
                            }
                            egui::Key::ArrowUp => {
                                self.search_state.history_prev();
                            }
                            egui::Key::ArrowDown => {
                                self.search_state.history_next();
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        // Step 3: 处理复制粘贴（从配置系统或硬编码的 Ctrl+Shift+C/V）
        let all_events = ctx.input(|i| i.events.clone());
        let mut consumed_keys = std::collections::HashSet::new();

        let mut saw_ctrl_shift_c = false;
        let mut saw_ctrl_shift_v = false;

        for evt in &all_events {
            if let egui::Event::Key { key, modifiers, pressed, .. } = evt {
                if *pressed {
                    if *key == egui::Key::C && modifiers.ctrl && modifiers.shift {
                        saw_ctrl_shift_c = true;
                    }
                    if *key == egui::Key::V && modifiers.ctrl && modifiers.shift {
                        saw_ctrl_shift_v = true;
                    }
                }
            }
        }

        if saw_ctrl_shift_c {
            if let Some(clipboard) = &self.clipboard {
                let terminal = session.terminal.lock();
                if let Some(text) = terminal.copy_selection() {
                    let _ = clipboard.copy(&text);
                    consumed_keys.insert("Ctrl+Shift+C".to_string());
                }
            }
        }

        if saw_ctrl_shift_v {
            if let Some(clipboard) = &self.clipboard {
                if let Ok(text) = clipboard.paste() {
                    let _ = session.shell.write(text.replace("\r\n", "\n").as_bytes());
                    consumed_keys.insert("Ctrl+Shift+V".to_string());
                }
            }
        }

        // Step 4: 处理普通键盘输入
        // 当搜索面板打开时，不处理普通键盘输入（搜索面板会处理输入）
        let mut keyboard_input = Vec::new();
        if !self.search_state.is_open {
            // 转换 consumed_keys 为需要的格式（HashSet<&str>）
            let consumed_keys_refs: std::collections::HashSet<&str> = consumed_keys
                .iter()
                .map(|s| s.as_str())
                .collect();
            self.renderer
                .handle_keyboard_input(ctx, &mut keyboard_input, &consumed_keys_refs, saw_ime_event);
        }

        let has_keyboard_input = !keyboard_input.is_empty();
        if has_keyboard_input {
            let mut input_guard = self.input_queue.lock();
            input_guard.extend(keyboard_input);
        }

        // Step 5: 发送输入到 shell
        {
            let mut input_guard = self.input_queue.lock();
            if !input_guard.is_empty() {
                let _ = session.shell.write(&input_guard);
                input_guard.clear();
            }
        }

        // Step 6: 处理 shell 事件
        let mut has_new_output = false;
        while let Ok(event) = session.shell.events().try_recv() {
            match event {
                ShellEvent::Output(data) => {
                    let mut terminal = session.terminal.lock();
                    terminal.process_input(&data);
                    self.status_message.clear();
                    has_new_output = true;
                }
                ShellEvent::Exit(code) => {
                    self.status_message = format!("Shell exited with code: {}", code);
                    has_new_output = true;
                }
                ShellEvent::Error(e) => {
                    self.status_message = format!("Error: {}", e);
                    has_new_output = true;
                }
            }
        }

        // Step 7: 发送终端输出回 shell（DSR 响应等）
        {
            let mut terminal = session.terminal.lock();
            let output = terminal.get_output();
            if !output.is_empty() {
                let _ = session.shell.write(&output);
            }
        }

        // Step 8: 光标闪烁
        let mut cursor_state_changed = false;
        let mut cursor_blink_active = false;
        {
            let terminal = session.terminal.lock();
            let app_wants_cursor_visible = terminal.is_cursor_visible();
            drop(terminal);

            if app_wants_cursor_visible {
                cursor_blink_active = true;
                if self.last_cursor_blink.elapsed() > Duration::from_millis(500) {
                    self.cursor_visible = !self.cursor_visible;
                    self.last_cursor_blink = std::time::Instant::now();
                    cursor_state_changed = true;
                }
            } else {
                if self.cursor_visible {
                    self.cursor_visible = false;
                    cursor_state_changed = true;
                }
            }
        }

        // Step 9: 滚动处理
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl) {
            let mut terminal = session.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                terminal.scroll(-3);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl) {
            let mut terminal = session.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                terminal.scroll(3);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::PageUp) && !i.modifiers.ctrl) {
            let mut terminal = session.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let (_, rows) = terminal.get_dimensions();
                terminal.scroll(rows as isize);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::PageDown) && !i.modifiers.ctrl) {
            let mut terminal = session.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let (_, rows) = terminal.get_dimensions();
                terminal.scroll(-(rows as isize));
            }
        }

        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        let shift_pressed = ctx.input(|i| i.modifiers.shift);

        // 检查是否启用鼠标报告
        let mouse_enabled = {
            let terminal = session.terminal.lock();
            terminal.is_mouse_enabled()
        };

        // 鼠标滚轮处理：
        // 1. 如果应用启用了鼠标报告（如 vim），滚轮会在下面的鼠标处理部分发送给应用
        // 2. 如果应用未启用鼠标，或在普通终端，滚轮用于查看历史
        if scroll_delta != 0.0 && !mouse_enabled {
            let mut terminal = session.terminal.lock();
            // 根据是否按住 Shift 键来决定滚动速度
            let scroll_multiplier = if shift_pressed { 5.0 } else { 2.0 };

            // 根据滚轮滚动方向和速度计算滚动行数
            // scroll_delta > 0: 向上滚（显示更早的内容）
            // scroll_delta < 0: 向下滚（显示更新的内容）
            let scroll_lines = if scroll_delta > 0.0 {
                // 向上滚轮，显示历史
                let lines = (scroll_delta * scroll_multiplier).ceil() as isize;
                lines.max(1)
            } else {
                // 向下滚轮，显示最新
                let lines = (scroll_delta.abs() * scroll_multiplier).ceil() as isize;
                -(lines.max(1))
            };
            terminal.scroll(scroll_lines);
        }

        // Step 11: 鼠标处理（包括滚轮）
        let mouse_reports: Vec<String> = {
            let terminal = session.terminal.lock();
            if !terminal.is_mouse_enabled() {
                drop(terminal);
                Vec::new()
            } else {
                let mut reports = Vec::new();

                // 获取鼠标位置信息
                if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    let screen_rect = ctx.viewport_rect();
                    let char_width = self.renderer.char_width;
                    let line_height = self.renderer.line_height;

                    let clamped_x = (pos.x - screen_rect.left()).max(0.0);
                    let clamped_y = (pos.y - screen_rect.top()).max(0.0);

                    let col = if char_width > 0.0 {
                        ((clamped_x / char_width) as usize).min(self.cols - 1)
                    } else {
                        0
                    };
                    let row = if line_height > 0.0 {
                        ((clamped_y / line_height) as usize).min(self.rows - 1)
                    } else {
                        0
                    };

                    // 处理鼠标滚轮（当启用鼠标报告时）
                    let scroll_delta_for_mouse = ctx.input(|i| i.smooth_scroll_delta.y);
                    if scroll_delta_for_mouse != 0.0 {
                        // 滚轮按钮号：64 = 向上滚，65 = 向下滚
                        let button = if scroll_delta_for_mouse > 0.0 { 64 } else { 65 };

                        // 发送多个滚轮事件，基于滚动距离
                        let scroll_count = (scroll_delta_for_mouse.abs().ceil() as usize).max(1);
                        for _ in 0..scroll_count {
                            if let Some(report) = terminal.get_mouse_report(button, col, row) {
                                reports.push(report);
                            }
                        }
                    }

                    // 处理鼠标按钮
                    let button_pressed = ctx.input(|i| {
                        let mut btns = Vec::new();
                        if i.pointer.button_pressed(egui::PointerButton::Primary) {
                            btns.push(0);
                        }
                        if i.pointer.button_pressed(egui::PointerButton::Secondary) {
                            btns.push(2);
                        }
                        if i.pointer.button_pressed(egui::PointerButton::Middle) {
                            btns.push(1);
                        }
                        btns
                    });

                    for button_num in button_pressed {
                        if let Some(report) = terminal.get_mouse_report(button_num, col, row) {
                            reports.push(report);
                        }
                    }
                }

                drop(terminal);
                reports
            }
        };

        let has_mouse_input = !mouse_reports.is_empty();
        if has_mouse_input {
            for report in mouse_reports {
                let _ = session.shell.write(report.as_bytes());
            }
        }

        // Step 12: 链接检测和交互
        {
            let terminal = session.terminal.lock();
            let links = self.link_detector.detect_all_links(&terminal.grid);
            drop(terminal);

            // 检测悬停的链接
            self.hovered_link = None;
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let screen_rect = ctx.viewport_rect();
                let char_width = self.renderer.char_width;
                let line_height = self.renderer.line_height;

                let clamped_x = (pos.x - screen_rect.left()).max(0.0);
                let clamped_y = (pos.y - screen_rect.top()).max(0.0);

                let col = if char_width > 0.0 {
                    ((clamped_x / char_width) as usize).min(self.cols - 1)
                } else {
                    0
                };
                let row = if line_height > 0.0 {
                    ((clamped_y / line_height) as usize).min(self.rows - 1)
                } else {
                    0
                };

                // 查找当前位置是否有链接
                for link in &links {
                    if link.line == row && col >= link.col_start && col < link.col_end {
                        self.hovered_link = Some(link.clone());
                        // 设置鼠标光标为手型
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        break;
                    }
                }
            }

            // 处理 Ctrl+Click 打开链接
            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary) && i.modifiers.ctrl) {
                if let Some(link) = &self.hovered_link {
                    match link::open_link(link) {
                        Ok(_) => {
                            self.status_message = format!("Opened: {}", link.text);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to open link: {}", e);
                        }
                    }
                }
            }
        }

        // 渲染 UI
        self.render_ui(ctx);

        // 只在需要时请求重绘：有新输出、光标状态改变、或有未处理的输入
        let should_repaint = has_new_output
            || cursor_state_changed
            || has_keyboard_input
            || has_mouse_input;

        if should_repaint {
            ctx.request_repaint();
        } else if cursor_blink_active {
            let blink_interval = Duration::from_millis(500);
            let next_blink_in = blink_interval.saturating_sub(self.last_cursor_blink.elapsed());
            ctx.request_repaint_after(next_blink_in);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_terminal_shortcut_events, shortcut_event_to_key_event};
    use eframe::egui;

    #[test]
    fn copy_event_becomes_ctrl_c_key_event() {
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };

        let event = shortcut_event_to_key_event(egui::Event::Copy, modifiers)
            .expect("copy event should map to a key event");

        assert_eq!(
            event,
            egui::Event::Key {
                key: egui::Key::C,
                physical_key: Some(egui::Key::C),
                pressed: true,
                repeat: false,
                modifiers,
            }
        );
    }

    #[test]
    fn paste_event_becomes_ctrl_shift_v_key_event_when_restored() {
        let modifiers = egui::Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Default::default()
        };
        let mut events = vec![egui::Event::Paste("ignored clipboard payload".to_owned())];

        normalize_terminal_shortcut_events(&mut events, modifiers, true);

        assert_eq!(
            events,
            vec![egui::Event::Key {
                key: egui::Key::V,
                physical_key: Some(egui::Key::V),
                pressed: true,
                repeat: false,
                modifiers,
            }]
        );
    }

    #[test]
    fn semantic_clipboard_events_are_dropped_when_not_restored() {
        let modifiers = egui::Modifiers::default();
        let mut events = vec![
            egui::Event::Copy,
            egui::Event::Paste("ignored".to_owned()),
            egui::Event::Text("a".to_owned()),
        ];

        normalize_terminal_shortcut_events(&mut events, modifiers, false);

        assert_eq!(events, vec![egui::Event::Text("a".to_owned())]);
    }
}
