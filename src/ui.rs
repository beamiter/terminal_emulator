use crate::color;
use crate::terminal::TerminalState;
use egui::{Color32, FontId, Response, Ui, Vec2};

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
        let char_width = (available_width / cols as f32).max(4.0);
        let line_height = (available_height / rows as f32).max(8.0);

        // eprintln!("[UI] Char size: {:.1} x {:.1}", char_width, line_height);

        // Allocate the full available space
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, available_height),
            egui::Sense::click_and_drag(),
        );

        // eprintln!("[UI] Rect: {:?}", rect);

        let painter = ui.painter_at(rect);

        // Paint background for entire rect
        painter.rect_filled(
            rect,
            egui::CornerRadius::ZERO,
            color::defaults::BACKGROUND,
        );

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
                    color::to_egui_color32(cell.foreground)
                } else {
                    // Handle default background color specially
                    match cell.background {
                        crate::terminal::Color::Default => color::defaults::BACKGROUND,
                        _ => color::to_egui_color32(cell.background),
                    }
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
                        color::to_egui_color32(cell.background)
                    } else {
                        color::to_egui_color32(cell.foreground)
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

                    // 控制键组合（Ctrl+C 只在没有选中时发送 SIGINT）
                    if modifiers.ctrl && !modifiers.shift && key == egui::Key::C {
                        input.extend(b"\x03");  // SIGINT
                    }
                    if modifiers.ctrl && key == egui::Key::D {
                        input.extend(b"\x04");  // EOF
                    }
                    if modifiers.ctrl && key == egui::Key::L {
                        input.extend(b"\x0c");  // 清屏（Ctrl+L）
                    }
                }
                _ => {}
            }
        }
    }
}
