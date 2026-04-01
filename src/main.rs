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
                    fonts.families
                        .get_mut(&egui::FontFamily::Monospace)
                        .unwrap()
                        .push("cjk".to_owned());
                    break;
                }
            }

            cc.egui_ctx.set_fonts(fonts);
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
        }
    }

    fn render_ui(&mut self, ctx: &egui::Context) {
        let terminal_guard = self.terminal.lock();
        let (width, height) = terminal_guard.get_dimensions();
        drop(terminal_guard); // 释放锁，允许后续修改
        self.cols = width;
        self.rows = height;

        // 使用 Area 来完全自定义布局，避免 panel 的 padding
        egui::Area::new(egui::Id::new("terminal_area"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let screen_size = ctx.viewport_rect().size();
                ui.set_max_size(screen_size);
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
        // Handle Ctrl+Shift+C for copy selection
        if ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl && i.modifiers.shift) {
            if let Some(clipboard) = &self.clipboard {
                let terminal = self.terminal.lock();
                if let Some(text) = terminal.copy_selection() {
                    let _ = clipboard.copy(&text);
                }
            }
        }

        // Handle Ctrl+Shift+V for paste
        if ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.ctrl && i.modifiers.shift) {
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
        }

        // Handle copy with F2 or Ctrl+Insert as alternatives
        if ctx.input(|i| i.key_pressed(egui::Key::F2) || (i.key_pressed(egui::Key::Insert) && i.modifiers.ctrl && !i.modifiers.shift)) {
            if let Some(clipboard) = &self.clipboard {
                let terminal = self.terminal.lock();
                if let Some(text) = terminal.copy_selection() {
                    let _ = clipboard.copy(&text);
                }
            }
        }

        // Handle paste with F3 or Shift+Insert
        if ctx.input(|i| i.key_pressed(egui::Key::F3) || (i.key_pressed(egui::Key::Insert) && i.modifiers.shift && !i.modifiers.ctrl)) {
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
        if self.last_cursor_blink.elapsed() > Duration::from_millis(500) {
            self.cursor_visible = !self.cursor_visible;
            self.last_cursor_blink = std::time::Instant::now();
        }

        // Collect keyboard input
        let mut keyboard_input = Vec::new();
        self.renderer
            .handle_keyboard_input(ctx, &mut keyboard_input);

        if !keyboard_input.is_empty() {
            let mut input_guard = self.input_queue.lock();
            input_guard.extend(keyboard_input);
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

        // 渲染 UI - 使用 Area 实现真正的全屏填充
        self.render_ui(ctx);

        // Request repaint for cursor blinking
        ctx.request_repaint();

        std::thread::sleep(Duration::from_millis(16));
    }
}
