mod color;
mod debug;
mod terminal;
mod ui;
mod clipboard;
mod pty;
mod shell;

use eframe::egui;
use std::sync::Arc;
use std::time::Duration;
use terminal::TerminalState;
use ui::TerminalRenderer;
use clipboard::ClipboardManager;
use parking_lot::Mutex as ParkingMutex;
use shell::{ShellSession, ShellEvent};
use crossbeam::channel::Receiver;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Terminal Emulator - ANSI Color Support",
        options,
        Box::new(|cc| {
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

            Ok(Box::new(TerminalApp::new()))
        }),
    )
}

struct TerminalApp {
    terminal: Arc<ParkingMutex<TerminalState>>,
    renderer: TerminalRenderer,
    input_queue: Arc<ParkingMutex<Vec<u8>>>,
    clipboard: Option<ClipboardManager>,
    shell: Option<ShellSession>,
    shell_rx: Option<Receiver<ShellEvent>>,
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
    fn new() -> Self {
        let cols = 100;
        let rows = 30;

        let terminal = TerminalState::new(cols, rows);

        // 尝试启动 shell
        let (shell, shell_rx) = match ShellSession::new(cols, rows) {
            Ok(session) => {
                let rx = session.events().clone();
                eprintln!("✓ Shell session started successfully");
                (Some(session), Some(rx))
            }
            Err(e) => {
                eprintln!("✗ Failed to start shell: {}", e);
                (None, None)
            }
        };

        // 初始化终端显示
        let term = terminal;

        let renderer = TerminalRenderer::new(14.0, 0.0);  // padding 改为 0
        let clipboard = ClipboardManager::new().ok();

        TerminalApp {
            terminal: Arc::new(ParkingMutex::new(term)),
            input_queue: Arc::new(ParkingMutex::new(Vec::new())),
            renderer,
            clipboard,
            shell,
            shell_rx,
            cols,
            rows,
            last_cursor_blink: std::time::Instant::now(),
            cursor_visible: true,
            status_message: String::new(),
        }
    }

    #[allow(deprecated)]
    fn render_ui(&mut self, ctx: &egui::Context) {
        // 使用 CentralPanel，背景由终端自己渲染，不用 egui 主题覆盖
        let frame = egui::Frame::NONE
            .inner_margin(0.0);

        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| {
                let (cols, rows) = self.renderer.grid_dimensions(ui.available_size());

                if cols != self.cols || rows != self.rows {
                    if let Some(shell) = &self.shell {
                        let _ = shell.resize(cols, rows);
                    }

                    let mut terminal = self.terminal.lock();
                    terminal.on_resize(cols, rows);
                    self.cols = cols;
                    self.rows = rows;
                }

                let mut terminal_guard = self.terminal.lock();
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
        // Step 1: 处理 IME 事件（在快捷键处理之前）
        let all_events = ctx.input(|i| i.events.clone());
        let mut saw_ime_event = false;
        for evt in &all_events {
            if let egui::Event::Ime(ime_event) = evt {
                saw_ime_event = true;
                let mut terminal = self.terminal.lock();
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
                        // 将提交的文本发送给 shell
                        if !text.is_empty() {
                            if let Some(shell) = &self.shell {
                                let _ = shell.write(text.as_bytes());
                            } else {
                                let mut input_guard = self.input_queue.lock();
                                input_guard.extend(text.as_bytes());
                            }
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
            let terminal = self.terminal.lock();
            terminal.window_title.clone()
        };
        if !window_title.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title));
        }

        // Step 2: 在 egui 层面先处理所有快捷键和粘贴事件
        let all_events = ctx.input(|i| i.events.clone());
        let mut consumed_keys = std::collections::HashSet::new();

        // Handle Ctrl+Shift+C/V for terminal copy/paste (not passed to shell)
        // raw_input_hook has already filtered out Copy/Paste/Cut events
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

        // Handle copy
        if saw_ctrl_shift_c {
            if let Some(clipboard) = &self.clipboard {
                let terminal = self.terminal.lock();
                if let Some(text) = terminal.copy_selection() {
                    let _ = clipboard.copy(&text);
                    consumed_keys.insert("Ctrl+Shift+C");
                }
            }
        }

        // Handle paste
        if saw_ctrl_shift_v {
            if let Some(clipboard) = &self.clipboard {
                if let Ok(text) = clipboard.paste() {
                    if let Some(shell) = &self.shell {
                        let _ = shell.write(text.replace("\r\n", "\n").as_bytes());
                    } else {
                        let mut input_guard = self.input_queue.lock();
                        input_guard.extend(text.replace("\r\n", "\n").as_bytes());
                    }
                    consumed_keys.insert("Ctrl+Shift+V");
                }
            }
        }

        // Step 2: 处理普通键盘输入，传给 handle_keyboard_input
        // 但排除已经被消费的按键
        let mut keyboard_input = Vec::new();
        self.renderer
            .handle_keyboard_input(ctx, &mut keyboard_input, &consumed_keys, saw_ime_event);

        if !keyboard_input.is_empty() {
            let mut input_guard = self.input_queue.lock();
            input_guard.extend(keyboard_input);
        }

        // Process queued input - send to shell instead of local terminal
        {
            let mut input_guard = self.input_queue.lock();
            if !input_guard.is_empty() {
                if let Some(shell) = &self.shell {
                    // 发送输入到 shell
                    let _ = shell.write(&input_guard);
                } else {
                    // 如果没有 shell，本地处理输入
                    let mut terminal = self.terminal.lock();
                    terminal.process_input(&input_guard);
                }
                input_guard.clear();
            }
        }

        // 处理 shell 事件
        if let Some(rx) = &self.shell_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ShellEvent::Output(data) => {
                        let mut terminal = self.terminal.lock();
                        terminal.process_input(&data);
                        self.status_message.clear();
                    }
                    ShellEvent::Exit(code) => {
                        self.status_message = format!("Shell exited with code: {}", code);
                        if let Some(mut shell) = self.shell.take() {
                            shell.mark_exited();
                        }
                    }
                    ShellEvent::Error(e) => {
                        self.status_message = format!("Error: {}", e);
                    }
                }
            }
        }

        // Handle cursor blinking (500ms on, 500ms off)
        // Only blink if the application hasn't hidden the cursor
        {
            let terminal = self.terminal.lock();
            let app_wants_cursor_visible = terminal.is_cursor_visible();
            drop(terminal);

            if app_wants_cursor_visible {
                if self.last_cursor_blink.elapsed() > Duration::from_millis(500) {
                    self.cursor_visible = !self.cursor_visible;
                    self.last_cursor_blink = std::time::Instant::now();
                }
            } else {
                // Application has hidden cursor via \x1b[?25l
                self.cursor_visible = false;
            }
        }

        // Handle Ctrl+D to exit the application
        if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl && !i.modifiers.shift) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Handle Ctrl+Up/Down for scroll
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl) {
            let mut terminal = self.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                terminal.scroll(-3);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl) {
            let mut terminal = self.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                terminal.scroll(3);
            }
        }

        // Handle PageUp/PageDown for scrolling through scrollback
        if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
            let mut terminal = self.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let (_, rows) = terminal.get_dimensions();
                terminal.scroll(rows as isize);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
            let mut terminal = self.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let (_, rows) = terminal.get_dimensions();
                terminal.scroll(-(rows as isize));
            }
        }

        // Handle mouse scroll wheel
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            let mut terminal = self.terminal.lock();
            if !terminal.is_alt_buffer_active() {
                let scroll_lines = if scroll_delta > 0.0 { 3 } else { -3 };
                terminal.scroll(scroll_lines);
            }
        }

        // Handle mouse clicks and movement for applications supporting mouse reporting
        let mouse_reports: Vec<String> = {
            let terminal = self.terminal.lock();
            if !terminal.is_mouse_enabled() {
                drop(terminal);
                Vec::new()
            } else {
                let mut reports = Vec::new();

                // Get current mouse position from context
                if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    // Get screen rect (same as in render_ui)
                    let screen_rect = ctx.viewport_rect();
                    let char_width = self.renderer.char_width;
                    let line_height = self.renderer.line_height;

                    // Calculate grid coordinates
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

                    // Check for button presses
                    let button_pressed = ctx.input(|i| {
                        let mut btns = Vec::new();
                        if i.pointer.button_pressed(egui::PointerButton::Primary) {
                            btns.push(0); // Left button
                        }
                        if i.pointer.button_pressed(egui::PointerButton::Secondary) {
                            btns.push(2); // Right button
                        }
                        if i.pointer.button_pressed(egui::PointerButton::Middle) {
                            btns.push(1); // Middle button
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

        // Send mouse reports to shell
        if !mouse_reports.is_empty() {
            for report in mouse_reports {
                if let Some(shell) = &self.shell {
                    let _ = shell.write(report.as_bytes());
                } else {
                    let mut input_guard = self.input_queue.lock();
                    input_guard.extend(report.as_bytes());
                }
            }
        }

        // 渲染 UI - 使用 Area 实现真正的全屏填充
        self.render_ui(ctx);

        // Request repaint for cursor blinking
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
