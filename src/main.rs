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
        Box::new(|_cc| {
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
                (Some(session), Some(rx))
            }
            Err(e) => {
                eprintln!("Failed to start shell: {}", e);
                (None, None)
            }
        };

        // Show welcome message with colors
        let mut term = terminal;
        let welcome = if shell.is_some() {
            "Shell started successfully\r\n$ ".as_bytes()
        } else {
            "Failed to start shell\r\nYou can still test ANSI colors and terminal features\r\n$ ".as_bytes()
        };
        term.process_input(welcome);

        let renderer = TerminalRenderer::new(14.0, 5.0);
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

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        let terminal_guard = self.terminal.lock();
        let (width, height) = terminal_guard.get_dimensions();
        self.cols = width;
        self.rows = height;

        ui.allocate_exact_size(
            egui::vec2(
                width as f32 * self.renderer.char_width + self.renderer.padding * 2.0,
                height as f32 * self.renderer.line_height + self.renderer.padding * 2.0,
            ),
            egui::Sense::click_and_drag(),
        );

        self.renderer.render(ui, &terminal_guard, self.cursor_visible);
    }
}

impl eframe::App for TerminalApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let terminal_guard = self.terminal.lock();
        let (width, height) = terminal_guard.get_dimensions();
        self.cols = width;
        self.rows = height;

        ui.allocate_exact_size(
            egui::vec2(
                width as f32 * self.renderer.char_width + self.renderer.padding * 2.0,
                height as f32 * self.renderer.line_height + self.renderer.padding * 2.0,
            ),
            egui::Sense::click_and_drag(),
        );

        self.renderer.render(ui, &terminal_guard, self.cursor_visible);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

        // Handle Ctrl+Shift+C for copy selection
        if ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl && i.modifiers.shift) {
            if let Some(clipboard) = &self.clipboard {
                let terminal = self.terminal.lock();
                if let Some(text) = terminal.copy_selection() {
                    let _ = clipboard.copy(&text);
                }
            }
        }

        // Handle Ctrl+V for paste
        if ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.ctrl) {
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

        // Main UI
        #[allow(deprecated)]
        {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.render_ui(ui);
            });
        }

        // Request repaint for cursor blinking
        ctx.request_repaint();

        std::thread::sleep(Duration::from_millis(16));
    }
}
