mod color;
mod terminal;
mod ui;
mod clipboard;
mod pty;

use eframe::egui;
use std::sync::Arc;
use std::time::Duration;
use terminal::TerminalState;
use ui::TerminalRenderer;
use clipboard::ClipboardManager;
use parking_lot::Mutex as ParkingMutex;

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
    cols: usize,
    rows: usize,
    last_cursor_blink: std::time::Instant,
    cursor_visible: bool,
}

impl TerminalApp {
    fn new() -> Self {
        let cols = 100;
        let rows = 30;

        let terminal = TerminalState::new(cols, rows);

        // Show welcome message with colors
        let mut term = terminal;
        let welcome = b"\x1b[1;31mWelcome to Terminal Emulator\x1b[0m\r\n\
                       \x1b[1;32m- Supports ANSI colors\r\n\
                       \x1b[1;33m- Type commands to see output\r\n\
                       \x1b[1;34m- Try: echo -e \"\\033[31mRed\\033[0m\"\x1b[0m\r\n\
                       \x1b[1;35m- Shortcuts: Ctrl+C (copy), Ctrl+V (paste), Ctrl+L (clear)\x1b[0m\r\n\
                       \x1b[1;35m$ \x1b[0m";
        term.process_input(welcome);

        let renderer = TerminalRenderer::new(14.0, 5.0);
        let clipboard = ClipboardManager::new().ok();

        TerminalApp {
            terminal: Arc::new(ParkingMutex::new(term)),
            input_queue: Arc::new(ParkingMutex::new(Vec::new())),
            renderer,
            clipboard,
            cols,
            rows,
            last_cursor_blink: std::time::Instant::now(),
            cursor_visible: true,
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

        // Handle global shortcuts
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

        // Handle Ctrl+V for paste
        if ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.ctrl) {
            if let Some(clipboard) = &self.clipboard {
                if let Ok(text) = clipboard.paste() {
                    let mut input_guard = self.input_queue.lock();
                    input_guard.extend(text.as_bytes());
                }
            }
        }

        // Process queued input
        {
            let mut input_guard = self.input_queue.lock();
            if !input_guard.is_empty() {
                let mut terminal = self.terminal.lock();
                // Echo input to terminal
                terminal.process_input(&input_guard);
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
