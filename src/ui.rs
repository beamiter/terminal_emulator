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

    pub fn render(&self, ui: &mut Ui, terminal: &TerminalState, cursor_visible: bool) -> Response {
        let grid = terminal.get_visible_cells();

        let rows = grid.len();
        let cols = if rows > 0 { grid[0].len() } else { 80 };

        // 获取可用空间
        let available_rect = ui.available_rect_before_wrap();
        let available_width = available_rect.width();
        let available_height = available_rect.height();

        // 计算能够显示的字符网格大小（考虑到 padding）
        let usable_width = available_width - self.padding * 2.0;
        let usable_height = available_height - self.padding * 2.0;

        // 计算实际的字符宽度和行高以填满可用空间
        let char_width = (usable_width / cols as f32).max(4.0);  // 最小 4px/字符
        let line_height = (usable_height / rows as f32).max(8.0); // 最小 8px/行

        // 创建一个覆盖整个可用空间的矩形
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, available_height),
            egui::Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);

        // 绘制背景
        painter.rect_filled(
            rect,
            egui::CornerRadius::ZERO,
            color::defaults::BACKGROUND,
        );

        let cursor_pos = terminal.get_cursor_pos();

        // 绘制网格内容
        for row_idx in 0..rows {
            for col_idx in 0..cols {
                let cell = &grid[row_idx][col_idx];

                let x = rect.left() + self.padding + col_idx as f32 * char_width;
                let y = rect.top() + self.padding + row_idx as f32 * line_height;

                // 确定背景颜色
                let bg_color = if terminal.is_cell_selected(row_idx, col_idx) {
                    color::defaults::selection()
                } else if cell.flags.inverse {
                    color::to_egui_color32(cell.foreground)
                } else {
                    color::to_egui_color32(cell.background)
                };

                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(x, y),
                    Vec2::new(char_width, line_height),
                );

                painter.rect_filled(cell_rect, egui::CornerRadius::ZERO, bg_color);

                // 绘制文本
                if cell.character != ' ' {
                    let fg_color = if cell.flags.inverse {
                        color::to_egui_color32(cell.background)
                    } else {
                        color::to_egui_color32(cell.foreground)
                    };

                    let text = cell.character.to_string();

                    // 动态调整字体大小以适应可用空间
                    let font_size = (line_height * 0.8).min(16.0).max(8.0);
                    let mut font_id = FontId::monospace(font_size);

                    if cell.flags.bold {
                        font_id.size *= 1.1;
                    }

                    let galley = ui.painter().layout_no_wrap(
                        text,
                        font_id,
                        fg_color,
                    );

                    // 居中绘制字符
                    let text_x = x + (char_width - galley.size().x) / 2.0;
                    let text_y = y + (line_height - galley.size().y) / 2.0;
                    painter.galley(egui::pos2(text_x, text_y), galley, Color32::TRANSPARENT);

                    // 下划线
                    if cell.flags.underline {
                        let underline_y = y + line_height - 2.0;
                        painter.line_segment(
                            [egui::pos2(x, underline_y), egui::pos2(x + char_width, underline_y)],
                            egui::Stroke::new(1.0, fg_color),
                        );
                    }
                }

                // 绘制光标
                if (row_idx, col_idx) == cursor_pos {
                    if cursor_visible {
                        painter.rect_filled(
                            cell_rect,
                            egui::CornerRadius::ZERO,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 40),
                        );
                        painter.rect_stroke(
                            cell_rect,
                            egui::CornerRadius::ZERO,
                            egui::Stroke::new(2.0, color::defaults::CURSOR),
                            egui::StrokeKind::Middle,
                        );
                    }
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

                    // 控制键组合
                    if modifiers.ctrl && key == egui::Key::C {
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
