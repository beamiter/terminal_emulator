use crate::color;
use crate::terminal::TerminalState;
use egui::{Color32, FontId, Response, Ui, Vec2};

fn resolve_foreground_color(color_value: crate::terminal::Color) -> Color32 {
    match color_value {
        crate::terminal::Color::Default => color::defaults::FOREGROUND,
        _ => color::to_egui_color32(color_value),
    }
}

fn resolve_background_color(color_value: crate::terminal::Color) -> Color32 {
    match color_value {
        crate::terminal::Color::Default => color::defaults::BACKGROUND,
        _ => color::to_egui_color32(color_value),
    }
}

fn snapped_span(origin: f32, index: usize, cell_size: f32) -> (f32, f32) {
    let start = (origin + index as f32 * cell_size).round();
    let end = (origin + (index + 1) as f32 * cell_size).round();
    (start, (end - start).max(1.0))
}

fn cursor_rect(rect: egui::Rect, row: usize, col: usize, char_width: f32, line_height: f32) -> egui::Rect {
    let (x, width) = snapped_span(rect.left(), col, char_width);
    let (y, height) = snapped_span(rect.top(), row, line_height);
    egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(width, height))
}

fn key_to_terminal_sequence(key: egui::Key, modifiers: egui::Modifiers) -> Option<&'static str> {
    if modifiers.ctrl || modifiers.alt || modifiers.mac_cmd || modifiers.command_only() {
        return None;
    }

    match key {
        egui::Key::Enter => Some("\r"),
        egui::Key::Escape => Some("\x1b"),
        egui::Key::Backspace => Some("\x08"),
        egui::Key::Tab => Some("\t"),
        egui::Key::ArrowUp => Some("\x1b[A"),
        egui::Key::ArrowDown => Some("\x1b[B"),
        egui::Key::ArrowRight => Some("\x1b[C"),
        egui::Key::ArrowLeft => Some("\x1b[D"),
        egui::Key::Home => Some("\x1b[H"),
        egui::Key::End => Some("\x1b[F"),
        egui::Key::Insert => Some("\x1b[2~"),
        egui::Key::Delete => Some("\x1b[3~"),
        egui::Key::PageUp => Some("\x1b[5~"),
        egui::Key::PageDown => Some("\x1b[6~"),
        egui::Key::F1 => Some("\x1bOP"),
        egui::Key::F2 => Some("\x1bOQ"),
        egui::Key::F3 => Some("\x1bOR"),
        egui::Key::F4 => Some("\x1bOS"),
        egui::Key::F5 => Some("\x1b[15~"),
        egui::Key::F6 => Some("\x1b[17~"),
        egui::Key::F7 => Some("\x1b[18~"),
        egui::Key::F8 => Some("\x1b[19~"),
        egui::Key::F9 => Some("\x1b[20~"),
        egui::Key::F10 => Some("\x1b[21~"),
        egui::Key::F11 => Some("\x1b[23~"),
        egui::Key::F12 => Some("\x1b[24~"),
        _ => None,
    }
}

pub struct TerminalRenderer {
    pub font_size: f32,
    pub char_width: f32,
    pub line_height: f32,
    pub padding: f32,
}

impl TerminalRenderer {
    pub fn new(font_size: f32, padding: f32) -> Self {
        let char_width = font_size * 0.62;
        let line_height = font_size * 1.35;

        TerminalRenderer {
            font_size,
            char_width,
            line_height,
            padding,
        }
    }

    pub fn grid_dimensions(&self, available: Vec2) -> (usize, usize) {
        let usable_width = (available.x - self.padding * 2.0).max(self.char_width);
        let usable_height = (available.y - self.padding * 2.0).max(self.line_height);

        let cols = (usable_width / self.char_width).floor().max(1.0) as usize;
        let rows = (usable_height / self.line_height).floor().max(1.0) as usize;

        (cols, rows)
    }

    pub fn render(&self, ui: &mut Ui, terminal: &mut TerminalState, cursor_visible: bool) -> Response {
        let grid = terminal.get_visible_cells();

        let rows = grid.len();
        let cols = if rows > 0 { grid[0].len() } else { 80 };

        // Get available space
        let available = ui.available_size();
        let available_width = available.x;
        let available_height = available.y;

        // eprintln!("[UI] Available: {:.0} x {:.0}", available_width, available_height);
        // eprintln!("[UI] Grid: {} x {}", cols, rows);

        let line_height = self.line_height;
        let char_width = self.char_width;

        // eprintln!("[UI] Char size: {:.1} x {:.1}", char_width, line_height);

        // Allocate the full available space
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, available_height),
            egui::Sense::click_and_drag().union(egui::Sense::focusable_noninteractive()),
        );

        // eprintln!("[UI] Rect: {:?}", rect);

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::CornerRadius::ZERO, color::defaults::BACKGROUND);

        let cursor_pos = terminal.get_cursor_pos();
        let ime_rect = cursor_rect(rect, cursor_pos.0, cursor_pos.1, char_width, line_height);

        response.request_focus();

        let ctx = ui.ctx();
        ctx.send_viewport_cmd(egui::ViewportCommand::IMEAllowed(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::IMEPurpose(egui::IMEPurpose::Terminal));
        ctx.send_viewport_cmd(egui::ViewportCommand::IMERect(ime_rect));

        // Handle mouse events for text selection
        // Track selection start on initial mouse down
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                // Clamp position to rect bounds to prevent underflow
                let clamped_x = (pos.x - rect.left()).max(0.0);
                let clamped_y = (pos.y - rect.top()).max(0.0);

                let col = if char_width > 0.0 {
                    ((clamped_x / char_width) as usize).min(cols - 1)
                } else {
                    0
                };
                let row = if line_height > 0.0 {
                    ((clamped_y / line_height) as usize).min(rows - 1)
                } else {
                    0
                };
                terminal.select_text((row, col), (row, col));
            }
        }

        // Update selection end during drag
        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                // Clamp position to rect bounds to prevent underflow
                let clamped_x = (pos.x - rect.left()).max(0.0);
                let clamped_y = (pos.y - rect.top()).max(0.0);

                let col = if char_width > 0.0 {
                    ((clamped_x / char_width) as usize).min(cols - 1)
                } else {
                    0
                };
                let row = if line_height > 0.0 {
                    ((clamped_y / line_height) as usize).min(rows - 1)
                } else {
                    0
                };
                if let Some(sel) = terminal.selection {
                    terminal.select_text(sel.start, (row, col));
                }
            }
        }

        // Render grid
        for row_idx in 0..rows {
            for col_idx in 0..cols {
                let cell = &grid[row_idx][col_idx];

                // Skip rendering wide character continuations - they are handled by the wide character itself
                if cell.wide_continuation {
                    continue;
                }

                // Position from rect top-left
                let (x, snapped_width) = snapped_span(rect.left(), col_idx, char_width);
                let (y, snapped_height) = snapped_span(rect.top(), row_idx, line_height);

                let bg_color = if terminal.is_cell_selected(row_idx, col_idx) {
                    color::defaults::selection()
                } else if cell.flags.inverse {
                    resolve_foreground_color(cell.foreground)
                } else {
                    resolve_background_color(cell.background)
                };

                let cell_width = if cell.wide {
                    let (_, next_width) = snapped_span(rect.left(), col_idx + 1, char_width);
                    snapped_width + next_width
                } else {
                    snapped_width
                };
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(x, y),
                    Vec2::new(cell_width, snapped_height),
                );

                if bg_color != color::defaults::BACKGROUND {
                    painter.rect_filled(cell_rect, egui::CornerRadius::ZERO, bg_color);
                }

                // Render character
                if cell.character != ' ' && !cell.wide_continuation {
                    let fg_color = if cell.flags.inverse {
                        resolve_background_color(cell.background)
                    } else {
                        resolve_foreground_color(cell.foreground)
                    };

                    let text = cell.character.to_string();
                    let mut font_id = FontId::monospace(self.font_size);

                    if cell.flags.bold {
                        font_id.size *= 1.1;
                    }

                    let galley = ui.painter().layout_no_wrap(
                        text.clone(),
                        font_id,
                        fg_color,
                    );

                    let text_x = x + (cell_width - galley.size().x).max(0.0) / 2.0;
                    let text_y = y + (snapped_height - galley.size().y) / 2.0;

                    painter.galley(egui::pos2(text_x, text_y), galley, fg_color);

                    if cell.flags.underline {
                        let underline_y = y + line_height - 1.0;
                        painter.line_segment(
                            [egui::pos2(x, underline_y), egui::pos2(x + cell_width, underline_y)],
                            egui::Stroke::new(1.0, fg_color),
                        );
                    }
                }

                // Render cursor
                if (row_idx, col_idx) == cursor_pos && cursor_visible {
                    painter.rect_filled(
                        cell_rect,
                        egui::CornerRadius::ZERO,
                        Color32::from_rgba_unmultiplied(80, 80, 80, 100),
                    );
                    painter.rect_stroke(
                        cell_rect,
                        egui::CornerRadius::ZERO,
                        egui::Stroke::new(1.5, color::defaults::CURSOR),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }

        // Render IME preedit text below cursor
        if !terminal.preedit_text.is_empty() && terminal.ime_enabled {
            let preedit_display = format!("➜ {}", terminal.preedit_text);

            // 在光标下方显示预编辑文本
            let preedit_x = rect.left() + cursor_pos.1 as f32 * char_width;
            let preedit_y = rect.top() + cursor_pos.0 as f32 * line_height + line_height;

            // 确保不超出屏幕范围
            if preedit_y + line_height <= rect.bottom() {
                let font_id = FontId::monospace(self.font_size);
                let galley = ui.painter().layout_no_wrap(
                    preedit_display,
                    font_id,
                    Color32::from_rgb(200, 200, 0),  // 黄色标记
                );

                painter.galley(egui::pos2(preedit_x, preedit_y), galley, Color32::WHITE);
            }
        }

        response
    }
    pub fn handle_keyboard_input(
        &self,
        ctx: &egui::Context,
        input: &mut Vec<u8>,
        _consumed_keys: &std::collections::HashSet<&str>,
        suppress_text_events: bool,
    ) {
        let events = ctx.input(|i| i.events.clone());

        for event in events {
            match event {
                egui::Event::Text(text) => {
                    if suppress_text_events {
                        continue;
                    }
                    // 不处理特殊按键对应的文本事件
                    if !text.is_empty() && text.as_bytes()[0] < 32 {
                        continue;
                    }
                    input.extend(text.as_bytes());
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    // Skip Ctrl+Shift+C/V/X - these will be handled in main.rs for copy/paste
                    if modifiers.ctrl && modifiers.shift && !modifiers.alt {
                        match key {
                            egui::Key::C | egui::Key::V | egui::Key::X => {
                                continue;
                            }
                            _ => {}
                        }
                    }

                    // Handle normal key sequences
                    let seq = key_to_terminal_sequence(key, modifiers);

                    if let Some(s) = seq {
                        input.extend(s.as_bytes());
                    }

                    // Handle Ctrl+letter combinations (send control characters)
                    if modifiers.ctrl && !modifiers.shift && !modifiers.alt {
                        match key {
                            egui::Key::A => input.push(0x01), // Ctrl+A
                            egui::Key::B => input.push(0x02), // Ctrl+B (backward page in vim)
                            egui::Key::C => input.push(0x03), // Ctrl+C (SIGINT)
                            egui::Key::D => {}, // Ctrl+D (handled in main.rs to exit app)
                            egui::Key::E => input.push(0x05), // Ctrl+E
                            egui::Key::F => input.push(0x06), // Ctrl+F (forward page in vim)
                            egui::Key::G => input.push(0x07), // Ctrl+G
                            egui::Key::H => input.push(0x08), // Ctrl+H (backspace)
                            egui::Key::I => input.push(0x09), // Ctrl+I (tab)
                            egui::Key::J => input.push(0x0a), // Ctrl+J (linefeed)
                            egui::Key::K => input.push(0x0b), // Ctrl+K
                            egui::Key::L => input.push(0x0c), // Ctrl+L (clear screen)
                            egui::Key::M => input.push(0x0d), // Ctrl+M (return)
                            egui::Key::N => input.push(0x0e), // Ctrl+N
                            egui::Key::O => input.push(0x0f), // Ctrl+O
                            egui::Key::P => input.push(0x10), // Ctrl+P
                            egui::Key::Q => input.push(0x11), // Ctrl+Q
                            egui::Key::R => input.push(0x12), // Ctrl+R
                            egui::Key::S => input.push(0x13), // Ctrl+S
                            egui::Key::T => input.push(0x14), // Ctrl+T
                            egui::Key::U => input.push(0x15), // Ctrl+U (delete line in vim)
                            egui::Key::V => input.push(0x16), // Ctrl+V (visual block in vim)
                            egui::Key::W => input.push(0x17), // Ctrl+W
                            egui::Key::X => input.push(0x18), // Ctrl+X
                            egui::Key::Y => input.push(0x19), // Ctrl+Y
                            egui::Key::Z => input.push(0x1a), // Ctrl+Z (suspend)
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
