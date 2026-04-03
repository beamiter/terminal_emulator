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

            Ok(Box::new(TerminalApp::new(&cfg_clone)))
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

impl TerminalApp {
    fn new(cfg: &config::Config) -> Self {
        let cols = cfg.cols;
        let rows = cfg.rows;

        // 创建首个会话
        let terminal = TerminalState::new(cols, rows);

        // 尝试启动 shell
        let (shell, _) = match ShellSession::new(cols, rows) {
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
            let dummy_shell = ShellSession::new(cols, rows).unwrap_or_else(|e| {
                panic!("Cannot create even a dummy shell session: {}", e)
            });
            Session::with_default_name(0, Arc::new(ParkingMutex::new(terminal)), dummy_shell)
        };

        let session_manager = SessionManager::new(session);

        let renderer = TerminalRenderer::new(cfg.font_size, cfg.padding);
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
                let available_height = ui.available_height();

                // Tab 栏 - 绘制标签和按钮
                {
                    let tab_rect = egui::Rect::from_min_size(
                        ui.cursor().left_top(),
                        egui::vec2(ui.available_width(), tab_height),
                    );

                    let painter = ui.painter();

                    // 背景
                    painter.rect_filled(tab_rect, 0.0, egui::Color32::from_rgb(40, 40, 40));
                    painter.hline(
                        tab_rect.left()..=tab_rect.right(),
                        tab_rect.bottom(),
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 100)),
                    );

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

                        let tab_width = galley.rect.width() + 20.0;
                        let tab_rect_item = egui::Rect::from_min_size(
                            egui::pos2(x_offset, tab_rect.top() + 5.0),
                            egui::vec2(tab_width, tab_height - 10.0),
                        );

                        let is_active = *idx == active_idx;
                        let bg_color = if is_active {
                            egui::Color32::from_rgb(70, 70, 80)
                        } else {
                            egui::Color32::from_rgb(50, 50, 60)
                        };

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

                        painter.text(
                            tab_rect_item.center(),
                            egui::Align2::CENTER_CENTER,
                            tab_text,
                            egui::FontId::monospace(12.0),
                            if is_active {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(180, 180, 190)
                            },
                        );

                        // 检测点击
                        if ctx.input(|i| i.pointer.any_click()) {
                            if let Some(click_pos) = ctx.input(|i| i.pointer.press_origin()) {
                                if tab_rect_item.contains(click_pos) {
                                    self.session_manager.switch_session(*idx);
                                }
                            }
                        }

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

                    painter.rect_filled(plus_btn_rect, 1.0, egui::Color32::from_rgb(50, 50, 60));
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

                    painter.text(
                        plus_btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "+",
                        egui::FontId::monospace(14.0),
                        egui::Color32::from_rgb(180, 180, 190),
                    );

                    // 检测 "+" 按钮点击
                    if ctx.input(|i| i.pointer.any_click()) {
                        if let Some(click_pos) = ctx.input(|i| i.pointer.press_origin()) {
                            if plus_btn_rect.contains(click_pos) {
                                self.session_manager.new_session(None, None);
                            }
                        }
                    }

                    // 向下移动光标
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), tab_height), egui::Sense::hover());
                }

                // 分隔线
                let separator_rect = egui::Rect::from_min_size(
                    ui.cursor().left_top(),
                    egui::vec2(ui.available_width(), 1.0),
                );
                ui.painter().hline(
                    separator_rect.left()..=separator_rect.right(),
                    separator_rect.top(),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 90)),
                );
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());

                // 终端显示区域
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
                self.renderer.render(ui, &mut terminal_guard, self.cursor_visible);
            });
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
        if !window_title.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title));
        }

        // Step 2: 处理快捷键 - 会话管理
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

        // 获取当前活跃会话（在所有快捷键处理完后）
        let session = self.session_manager.get_active_session_mut();

        // Step 3: 处理 Ctrl+Shift+C/V 复制粘贴
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
        let mut keyboard_input = Vec::new();
        // 转换 consumed_keys 为需要的格式（HashSet<&str>）
        let consumed_keys_refs: std::collections::HashSet<&str> = consumed_keys
            .iter()
            .map(|s| s.as_str())
            .collect();
        self.renderer
            .handle_keyboard_input(ctx, &mut keyboard_input, &consumed_keys_refs, saw_ime_event);

        if !keyboard_input.is_empty() {
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
        while let Ok(event) = session.shell.events().try_recv() {
            match event {
                ShellEvent::Output(data) => {
                    let mut terminal = session.terminal.lock();
                    terminal.process_input(&data);
                    self.status_message.clear();
                }
                ShellEvent::Exit(code) => {
                    self.status_message = format!("Shell exited with code: {}", code);
                }
                ShellEvent::Error(e) => {
                    self.status_message = format!("Error: {}", e);
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
        {
            let terminal = session.terminal.lock();
            let app_wants_cursor_visible = terminal.is_cursor_visible();
            drop(terminal);

            if app_wants_cursor_visible {
                if self.last_cursor_blink.elapsed() > Duration::from_millis(500) {
                    self.cursor_visible = !self.cursor_visible;
                    self.last_cursor_blink = std::time::Instant::now();
                }
            } else {
                self.cursor_visible = false;
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

        if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
            let mut terminal = session.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let (_, rows) = terminal.get_dimensions();
                terminal.scroll(rows as isize);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
            let mut terminal = session.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let (_, rows) = terminal.get_dimensions();
                terminal.scroll(-(rows as isize));
            }
        }

        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            let mut terminal = session.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let scroll_lines = if scroll_delta > 0.0 { 3 } else { -3 };
                terminal.scroll(scroll_lines);
            }
        }

        // Step 11: 鼠标处理
        let mouse_reports: Vec<String> = {
            let terminal = session.terminal.lock();
            if !terminal.is_mouse_enabled() {
                drop(terminal);
                Vec::new()
            } else {
                let mut reports = Vec::new();

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

        if !mouse_reports.is_empty() {
            for report in mouse_reports {
                let _ = session.shell.write(report.as_bytes());
            }
        }

        // 渲染 UI
        self.render_ui(ctx);

        // 请求重绘（用于光标闪烁）
        ctx.request_repaint();

        std::thread::sleep(Duration::from_millis(16));
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
