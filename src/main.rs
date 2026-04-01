mod color;
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
    ime_buffer: String,  // IME 输入缓冲区
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
        let mut term = terminal;

        // 写入初始提示
        let init_msg = b"Terminal Emulator v0.4.0\r\nEnter your commands:\r\n$ ";
        term.process_input(init_msg);

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
            ime_buffer: String::new(),
        }
    }

    fn render_ui(&mut self, ctx: &egui::Context) {
        let terminal_guard = self.terminal.lock();
        let (width, height) = terminal_guard.get_dimensions();
        drop(terminal_guard);
        self.cols = width;
        self.rows = height;

        // 使用 CentralPanel 来填充整个窗口，使用深色背景
        let frame = egui::Frame::NONE
            .fill(egui::Color32::from_rgb(29, 29, 29))
            .inner_margin(0.0);

        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| {
                let mut terminal_guard = self.terminal.lock();
                self.renderer.render(ui, &mut terminal_guard, self.cursor_visible);
            });
    }
}

impl eframe::App for TerminalApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // UI handled in update()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Step 1: 处理 IME 事件（在快捷键处理之前）
        let all_events = ctx.input(|i| i.events.clone());
        for evt in &all_events {
            if let egui::Event::Ime(ime_event) = evt {
                let mut terminal = self.terminal.lock();
                match ime_event {
                    egui::ImeEvent::Enabled => {
                        eprintln!("[IME] Enabled");
                        terminal.ime_enabled = true;
                    }
                    egui::ImeEvent::Preedit(text) => {
                        eprintln!("[IME] Preedit: {:?}", text);
                        terminal.set_preedit(text.clone(), text.len());
                    }
                    egui::ImeEvent::Commit(text) => {
                        eprintln!("[IME] Commit: {:?}", text);
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
                        // 清除 IME 输入缓冲区
                        self.ime_buffer.clear();
                    }
                    egui::ImeEvent::Disabled => {
                        eprintln!("[IME] Disabled");
                        terminal.ime_enabled = false;
                        terminal.clear_preedit();
                        self.ime_buffer.clear();
                    }
                }
            }
        }

        // Step 2: 在 egui 层面先处理所有快捷键和粘贴事件
        let all_events = ctx.input(|i| i.events.clone());
        let mut consumed_keys = std::collections::HashSet::new();

        // 不再拦截快捷键，完全释放给 shell
        // Ctrl+C: shell 会收到 SIGINT（中断）
        // Ctrl+V: 由输入法处理或正常键盘输入
        // Ctrl+X: 不再做剪切

        // 仅保留 Ctrl+Shift+C/V 用于终端的复制粘贴（不影响 shell）
        for evt in &all_events {
            if let egui::Event::Key { key, pressed, modifiers, .. } = evt {
                // Ctrl+Shift+C: 终端复制选中文本
                if *key == egui::Key::C && modifiers.ctrl && modifiers.shift && !*pressed {
                    if let Some(clipboard) = &self.clipboard {
                        let terminal = self.terminal.lock();
                        if let Some(text) = terminal.copy_selection() {
                            let _ = clipboard.copy(&text);
                        }
                    }
                    consumed_keys.insert("Ctrl+Shift+C");
                }

                // Ctrl+Shift+V: 终端粘贴
                if *key == egui::Key::V && modifiers.ctrl && modifiers.shift && !*pressed {
                    if let Some(clipboard) = &self.clipboard {
                        if let Ok(text) = clipboard.paste() {
                            if !text.is_empty() {
                                if let Some(shell) = &self.shell {
                                    let _ = shell.write(text.as_bytes());
                                } else {
                                    let mut input_guard = self.input_queue.lock();
                                    input_guard.extend(text.as_bytes());
                                }
                            }
                        }
                    }
                    consumed_keys.insert("Ctrl+Shift+V");
                }
            }
        }

        // Step 2: 处理普通键盘输入，传给 handle_keyboard_input
        // 但排除已经被消费的按键
        let mut keyboard_input = Vec::new();
        self.renderer
            .handle_keyboard_input(ctx, &mut keyboard_input, &consumed_keys);

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

        // Handle Ctrl+Up/Down for scroll
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl) {
            // Ctrl+Down: scroll down
            let mut terminal = self.terminal.lock();
            terminal.scroll(-3);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl) {
            // Ctrl+Up: scroll up
            let mut terminal = self.terminal.lock();
            terminal.scroll(3);
        }

        // Handle PageUp/PageDown for scrolling through scrollback
        if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
            // PageUp: scroll up by one page (roughly screen height)
            let mut terminal = self.terminal.lock();
            let (_, rows) = terminal.get_dimensions();
            terminal.scroll(rows as isize);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
            // PageDown: scroll down by one page
            let mut terminal = self.terminal.lock();
            let (_, rows) = terminal.get_dimensions();
            terminal.scroll(-(rows as isize));
        }

        // Handle mouse scroll wheel
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            let mut terminal = self.terminal.lock();
            let scroll_lines = if scroll_delta > 0.0 { 3 } else { -3 };
            terminal.scroll(scroll_lines);
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
                    let char_width = (screen_rect.width() / self.cols as f32).max(4.0);
                    let line_height = (screen_rect.height() / self.rows as f32).max(8.0);

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
