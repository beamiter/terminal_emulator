mod color;
mod terminal;
mod ui;

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use terminal::TerminalState;
use ui::TerminalRenderer;

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
    terminal: Arc<Mutex<TerminalState>>,
    renderer: TerminalRenderer,
    input_queue: Arc<Mutex<Vec<u8>>>,
    cols: usize,
    rows: usize,
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
                       \x1b[1;35m$ \x1b[0m";
        term.process_input(welcome);

        let renderer = TerminalRenderer::new(14.0, 5.0);

        TerminalApp {
            terminal: Arc::new(Mutex::new(term)),
            input_queue: Arc::new(Mutex::new(Vec::new())),
            cols,
            rows,
            renderer,
        }
    }
}

impl eframe::App for TerminalApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Ok(mut terminal_guard) = self.terminal.lock() {
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

            self.renderer.render(ui, &terminal_guard);
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Collect keyboard input
        let mut keyboard_input = Vec::new();
        self.renderer
            .handle_keyboard_input(ctx, &mut keyboard_input);

        if !keyboard_input.is_empty() {
            if let Ok(mut input_guard) = self.input_queue.lock() {
                input_guard.extend(keyboard_input);
            }
        }

        // Process queued input
        if let Ok(mut input_guard) = self.input_queue.lock() {
            if !input_guard.is_empty() {
                if let Ok(mut terminal) = self.terminal.lock() {
                    // Echo input to terminal
                    terminal.process_input(&input_guard);
                }
                input_guard.clear();
            }
        }

        // Main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui(ui, frame);
        });

        // Request repaint
        ctx.request_repaint();

        std::thread::sleep(Duration::from_millis(16));
    }
}
