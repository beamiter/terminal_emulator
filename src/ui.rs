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

pub struct TerminalRenderer {
    pub font_size: f32,
    pub char_width: f32,
    pub line_height: f32,
    pub padding: f32,
}

impl TerminalRenderer {
    pub fn new(font_size: f32, padding: f32) -> Self {
        let char_width = font_size * 0.6;
        let line_height = font_size * 1.2;

        TerminalRenderer {
            font_size,
            char_width,
            line_height,
            padding,
        }
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

        // Calculate character width/height
        let line_height = (available_height / rows as f32).max(8.0);
        // For monospace fonts, use tighter spacing (0.45 for more compact display)
        let char_width = line_height * 0.45;

        // eprintln!("[UI] Char size: {:.1} x {:.1}", char_width, line_height);

        // Allocate the full available space
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, available_height),
            egui::Sense::click_and_drag(),
        );

        // eprintln!("[UI] Rect: {:?}", rect);

        let painter = ui.painter_at(rect);

        let cursor_pos = terminal.get_cursor_pos();

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
                let x = rect.left() + col_idx as f32 * char_width;
                let y = rect.top() + row_idx as f32 * line_height;

                let bg_color = if terminal.is_cell_selected(row_idx, col_idx) {
                    color::defaults::selection()
                } else if cell.flags.inverse {
                    resolve_foreground_color(cell.foreground)
                } else {
                    resolve_background_color(cell.background)
                };

                let cell_width = if cell.wide { char_width * 2.0 } else { char_width };
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(x, y),
                    Vec2::new(cell_width, line_height),
                );

                painter.rect_filled(cell_rect, egui::CornerRadius::ZERO, bg_color);

                // Render character
                if cell.character != ' ' && !cell.wide_continuation {
                    let fg_color = if cell.flags.inverse {
                        resolve_background_color(cell.background)
                    } else {
                        resolve_foreground_color(cell.foreground)
                    };

                    let text = cell.character.to_string();
                    // Font size should fit within cell width, not just line height
                    // For wide characters, max width is char_width * 2
                    let max_font_size = (char_width * 0.9).max(8.0);
                    let font_size = (line_height * 0.7).min(max_font_size);
                    let mut font_id = FontId::monospace(font_size);

                    if cell.flags.bold {
                        font_id.size *= 1.1;
                    }

                    let galley = ui.painter().layout_no_wrap(
                        text.clone(),
                        font_id,
                        fg_color,
                    );

                    // Left-align text in cell
                    let text_x = x + 1.0;
                    let text_y = y + (line_height - galley.size().y) / 2.0;

                    painter.galley(egui::pos2(text_x, text_y), galley, fg_color);

                    if cell.flags.underline {
                        let underline_y = y + line_height - 1.0;
                        painter.line_segment(
                            [egui::pos2(x, underline_y), egui::pos2(x + char_width, underline_y)],
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
                let font_size_preedit = (line_height * 0.7).min(char_width * 0.9).max(8.0);
                let font_id = FontId::monospace(font_size_preedit);
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

    pub fn handle_mouse_input(
        &self,
        response: &Response,
        terminal: &mut TerminalState,
        rect: egui::Rect,
    ) {
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let col = ((pos.x - rect.left() - self.padding) / self.char_width) as usize;
                let row = ((pos.y - rect.top() - self.padding) / self.line_height) as usize;

                terminal.select_text((row, col), (row, col));
            }
        }

        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let col = ((pos.x - rect.left() - self.padding) / self.char_width) as usize;
                let row = ((pos.y - rect.top() - self.padding) / self.line_height) as usize;

                terminal.select_text((row, col), (row, col));
            }
        }
    }

    pub fn handle_keyboard_input(
        &self,
        ctx: &egui::Context,
        input: &mut Vec<u8>,
        consumed_keys: &std::collections::HashSet<&str>,
    ) {
        let events = ctx.input(|i| i.events.clone());

        for event in events {
            match event {
                egui::Event::Text(text) => {
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
                    // 检查该按键是否已被上层处理
                    let key_combo = match key {
                        egui::Key::C if modifiers.ctrl && !modifiers.shift => "Ctrl+C",
                        egui::Key::C if modifiers.ctrl && modifiers.shift => "Ctrl+Shift+C",
                        egui::Key::X if modifiers.ctrl && !modifiers.shift => "Ctrl+X",
                        egui::Key::V if modifiers.ctrl && !modifiers.shift => "Ctrl+V",
                        egui::Key::V if modifiers.ctrl && modifiers.shift => "Ctrl+Shift+V",
                        _ => "",
                    };

                    // 如果已被消费，跳过处理
                    if !key_combo.is_empty() && consumed_keys.contains(key_combo) {
                        eprintln!("[handle_keyboard_input] Skipping {} (already consumed)", key_combo);
                        continue;
                    }

                    // Handle normal key sequences
                    let seq = match key {
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
                        egui::Key::Delete => Some("\x1b[3~"),
                        egui::Key::PageUp => Some("\x1b[5~"),
                        egui::Key::PageDown => Some("\x1b[6~"),
                        _ => None,
                    };

                    if let Some(s) = seq {
                        input.extend(s.as_bytes());
                    }

                    // Handle Ctrl+letter combinations (send control characters)
                    if modifiers.ctrl && !modifiers.shift && !modifiers.alt {
                        match key {
                            egui::Key::A => input.push(0x01), // Ctrl+A
                            egui::Key::B => input.push(0x02), // Ctrl+B (backward page in vim)
                            egui::Key::C => input.push(0x03), // Ctrl+C (SIGINT)
                            egui::Key::D => input.push(0x04), // Ctrl+D (EOF)
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
                            egui::Key::V => input.push(0x16), // Ctrl+V (paste/literal)
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
