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
        egui::Key::Backspace => Some("\x7f"),  // Send DEL (0x7f)
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
    pub dragging_scrollbar: bool,
    pub scrollbar_visibility: crate::config::ScrollbarVisibility,
    requested_initial_focus: bool,
    ime_enabled: bool,
    last_ime_rect: Option<egui::Rect>,
}

impl TerminalRenderer {
    const SCROLLBAR_WIDTH: f32 = 8.0;
    const SCROLLBAR_GAP: f32 = 2.0;
    const MIN_THUMB_HEIGHT: f32 = 24.0;
    const SCROLLBAR_HIT_EXPAND: f32 = 8.0;

    pub fn new(
        font_size: f32,
        padding: f32,
        scrollbar_visibility: crate::config::ScrollbarVisibility,
    ) -> Self {
        let char_width = font_size * 0.62;
        let line_height = font_size * 1.35;

        TerminalRenderer {
            font_size,
            char_width,
            line_height,
            padding,
            dragging_scrollbar: false,
            scrollbar_visibility,
            requested_initial_focus: false,
            ime_enabled: false,
            last_ime_rect: None,
        }
    }

    pub fn sync_font_metrics(&mut self, ctx: &egui::Context) {
        let font_id = FontId::monospace(self.font_size);
        let (char_width, line_height) = ctx.fonts_mut(|fonts| {
            let glyph_width = fonts.glyph_width(&font_id, 'W');
            let row_height = fonts.row_height(&font_id);
            (glyph_width, row_height)
        });

        if char_width.is_finite() && char_width > 0.0 {
            self.char_width = char_width;
        }

        if line_height.is_finite() && line_height > 0.0 {
            self.line_height = line_height;
        }
    }

    fn content_size(&self, available: Vec2) -> Vec2 {
        let outer_width = (available.x - self.padding * 2.0).max(self.char_width);
        let outer_height = (available.y - self.padding * 2.0).max(self.line_height);
        let reserved_scrollbar_width =
            (Self::SCROLLBAR_WIDTH + Self::SCROLLBAR_GAP).min((outer_width - self.char_width).max(0.0));

        Vec2::new(
            (outer_width - reserved_scrollbar_width).max(self.char_width),
            outer_height,
        )
    }

    fn layout_rects(&self, rect: egui::Rect) -> (egui::Rect, egui::Rect) {
        let outer_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + self.padding, rect.top() + self.padding),
            egui::pos2(
                (rect.right() - self.padding).max(rect.left() + self.char_width),
                (rect.bottom() - self.padding).max(rect.top() + self.line_height),
            ),
        );

        let reserved_scrollbar_width =
            (Self::SCROLLBAR_WIDTH + Self::SCROLLBAR_GAP).min((outer_rect.width() - self.char_width).max(0.0));
        let content_rect = egui::Rect::from_min_max(
            outer_rect.min,
            egui::pos2((outer_rect.right() - reserved_scrollbar_width).max(outer_rect.left()), outer_rect.bottom()),
        );
        let scrollbar_rect = egui::Rect::from_min_max(
            egui::pos2((outer_rect.right() - Self::SCROLLBAR_WIDTH).max(content_rect.right()), outer_rect.top()),
            outer_rect.max,
        );

        (content_rect, scrollbar_rect)
    }

    pub fn grid_dimensions(&self, available: Vec2) -> (usize, usize) {
        let content_size = self.content_size(available);
        let usable_width = content_size.x;
        let usable_height = content_size.y;

        let cols = (usable_width / self.char_width).floor().max(1.0) as usize;
        let rows = (usable_height / self.line_height).floor().max(1.0) as usize;

        (cols, rows)
    }

    /// 在指定矩形内渲染（用于多窗格模式）
    pub fn render_in_rect(&mut self, ui: &mut Ui, terminal: &mut TerminalState, cursor_visible: bool, search_state: &crate::search::SearchState, links: &[crate::link::Link], hovered_link: &Option<crate::link::Link>, target_rect: egui::Rect) -> Response {
        let grid = terminal.get_visible_cells();

        let rows = grid.len();
        let cols = if rows > 0 { grid[0].len() } else { 80 };

        let line_height = self.line_height;
        let char_width = self.char_width;

        // Allocate in the target rectangle area
        let rect = target_rect;
        let response = ui.allocate_exact_size(
            egui::vec2(rect.width(), rect.height()),
            egui::Sense::click_and_drag().union(egui::Sense::focusable_noninteractive()),
        ).1;

        self.render_terminal_at_rect(ui, terminal, cursor_visible, search_state, links, hovered_link, rect, response, cols, rows, line_height, char_width)
    }

    pub fn render(&mut self, ui: &mut Ui, terminal: &mut TerminalState, cursor_visible: bool, search_state: &crate::search::SearchState, links: &[crate::link::Link], hovered_link: &Option<crate::link::Link>) -> Response {
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

        self.render_terminal_at_rect(ui, terminal, cursor_visible, search_state, links, hovered_link, rect, response.clone(), cols, rows, line_height, char_width)
    }

    fn render_terminal_at_rect(&mut self, ui: &mut Ui, terminal: &mut TerminalState, cursor_visible: bool, search_state: &crate::search::SearchState, links: &[crate::link::Link], hovered_link: &Option<crate::link::Link>, rect: egui::Rect, response: egui::Response, _cols: usize, _rows: usize, line_height: f32, char_width: f32) -> Response {
        let grid = terminal.get_visible_cells();
        let rows = grid.len();
        let cols = if rows > 0 { grid[0].len() } else { 80 };

        // eprintln!("[UI] Rect: {:?}", rect);

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::CornerRadius::ZERO, color::defaults::BACKGROUND);

        let (content_rect, scrollbar_rect) = self.layout_rects(rect);
        let cursor_pos = terminal.get_cursor_pos();
        let ime_rect = cursor_rect(content_rect, cursor_pos.0, cursor_pos.1, char_width, line_height);

        let ctx = ui.ctx();
        if response.clicked() || (!self.requested_initial_focus && !ctx.memory(|mem| mem.has_focus(response.id))) {
            response.request_focus();
            self.requested_initial_focus = true;
        }

        let has_focus = ctx.memory(|mem| mem.has_focus(response.id));
        if has_focus != self.ime_enabled {
            ctx.send_viewport_cmd(egui::ViewportCommand::IMEAllowed(has_focus));
            if has_focus {
                ctx.send_viewport_cmd(egui::ViewportCommand::IMEPurpose(egui::IMEPurpose::Terminal));
            }
            self.ime_enabled = has_focus;
            if !has_focus {
                self.last_ime_rect = None;
            }
        }

        if has_focus {
            let ime_rect_changed = self
                .last_ime_rect
                .map(|prev| prev != ime_rect)
                .unwrap_or(true);
            if ime_rect_changed {
                ctx.send_viewport_cmd(egui::ViewportCommand::IMERect(ime_rect));
                self.last_ime_rect = Some(ime_rect);
            }
        }

        // Pre-compute scrollbar geometry for hit-testing
        let scrollbar_width = scrollbar_rect.width();
        let scrollbar_x = scrollbar_rect.left();
        let scrollbar_hovered = ctx
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|pos| scrollbar_rect.expand(Self::SCROLLBAR_HIT_EXPAND).contains(pos));
        let show_scrollbar = terminal.scrollback.len() > 0
            && match self.scrollbar_visibility {
                crate::config::ScrollbarVisibility::Always => true,
                crate::config::ScrollbarVisibility::Auto => scrollbar_hovered || self.dragging_scrollbar,
            };

        // Compute thumb rect and related values for interaction
        let scrollbar_thumb_rect: Option<(egui::Rect, f32, f32, f32)> = if terminal.scrollback.len() > 0 {
            let total_lines = terminal.scrollback.len() + rows;
            let visible_lines = rows;
            if total_lines > visible_lines {
                let scrollbar_height = scrollbar_rect.height();
                let thumb_height = ((visible_lines as f32 / total_lines as f32) * scrollbar_height)
                    .clamp(Self::MIN_THUMB_HEIGHT, scrollbar_height);
                let thumb_y = (terminal.scroll_offset as f32 / terminal.scrollback.len() as f32)
                    * (scrollbar_height - thumb_height);
                let thumb_rect = egui::Rect::from_min_size(
                    egui::pos2(scrollbar_x, scrollbar_rect.top() + thumb_y),
                    egui::vec2(scrollbar_width, thumb_height),
                );
                Some((thumb_rect, scrollbar_height, thumb_height, terminal.scrollback.len() as f32))
            } else {
                None
            }
        } else {
            None
        };

        // Handle mouse events for text selection
        // Track selection start on initial mouse down
        // Scrollbar interaction: detect if drag started on thumb
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                if pos.x >= scrollbar_x {
                    if let Some((thumb_rect, ..)) = scrollbar_thumb_rect {
                        if thumb_rect.contains(pos) {
                            self.dragging_scrollbar = true;
                        }
                    }
                }
            }
        }

        // Scrollbar drag: update scroll_offset while dragging thumb
        if self.dragging_scrollbar && response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some((_, scrollbar_height, thumb_height, scrollback_len_f)) = scrollbar_thumb_rect {
                    let track_height = scrollbar_height - thumb_height;
                    if track_height > 0.0 {
                        // Convert Y position to scroll_offset (clamped to valid range)
                        let relative_y = (pos.y - scrollbar_rect.top() - thumb_height / 2.0).clamp(0.0, track_height);
                        let new_offset = ((relative_y / track_height) * scrollback_len_f).round() as usize;
                        terminal.scroll_offset = new_offset.min(terminal.scrollback.len());
                    }
                }
            }
        }

        // Reset dragging state when mouse released
        if response.drag_stopped() {
            self.dragging_scrollbar = false;
        }

        // Click in scrollbar track (not on thumb): page up/down
        if response.drag_started() && !self.dragging_scrollbar {
            if let Some(pos) = response.interact_pointer_pos() {
                if pos.x >= scrollbar_x && terminal.scrollback.len() > 0 {
                    if let Some((thumb_rect, ..)) = scrollbar_thumb_rect {
                        if pos.y < thumb_rect.top() {
                            // Click above thumb: scroll up (older history)
                            terminal.scroll(rows as isize);
                        } else if pos.y > thumb_rect.bottom() {
                            // Click below thumb: scroll down (newer/live view)
                            terminal.scroll(-(rows as isize));
                        }
                    }
                }
            }
        }

        // Text selection: only when not interacting with scrollbar
        if response.drag_started() && !self.dragging_scrollbar {
            if let Some(pos) = response.interact_pointer_pos() {
                // Only select text if NOT in scrollbar area
                if pos.x < scrollbar_x {
                    // Clamp position to rect bounds to prevent underflow
                    let clamped_x = (pos.x - content_rect.left()).clamp(0.0, content_rect.width().max(0.0));
                    let clamped_y = (pos.y - content_rect.top()).clamp(0.0, content_rect.height().max(0.0));

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
        }

        // Update selection end during drag
        if response.dragged() && !self.dragging_scrollbar {
            if let Some(pos) = response.interact_pointer_pos() {
                if pos.x < scrollbar_x {
                    // Clamp position to rect bounds to prevent underflow
                    let clamped_x = (pos.x - content_rect.left()).clamp(0.0, content_rect.width().max(0.0));
                    let clamped_y = (pos.y - content_rect.top()).clamp(0.0, content_rect.height().max(0.0));

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
                let (x, snapped_width) = snapped_span(content_rect.left(), col_idx, char_width);
                let (y, snapped_height) = snapped_span(content_rect.top(), row_idx, line_height);

                let mut bg_color = if terminal.is_cell_selected(row_idx, col_idx) {
                    color::defaults::selection()
                } else if cell.flags.inverse {
                    resolve_foreground_color(cell.foreground)
                } else {
                    resolve_background_color(cell.background)
                };

                // Check if this cell is part of a search match (反色高亮)
                if !search_state.matches.is_empty() && !search_state.query.is_empty() {
                    for (match_idx, m) in search_state.matches.iter().enumerate() {
                        if m.line == row_idx && col_idx >= m.col_start && col_idx < m.col_end {
                            // 反色高亮：交换前景和背景色
                            let orig_fg = resolve_foreground_color(cell.foreground);

                            bg_color = orig_fg;

                            // 如果这是当前匹配项，加深颜色
                            if match_idx == search_state.current_match_index % search_state.matches.len() {
                                // 让高亮更突出（降低 alpha 以加深）
                                let [r, g, b, _a] = bg_color.to_srgba_unmultiplied();
                                bg_color = Color32::from_rgba_unmultiplied(
                                    (r as u16 * 180 / 255) as u8,
                                    (g as u16 * 180 / 255) as u8,
                                    (b as u16 * 180 / 255) as u8,
                                    255,
                                );
                            }
                            break;
                        }
                    }
                }

                let cell_width = if cell.wide {
                    let (_, next_width) = snapped_span(content_rect.left(), col_idx + 1, char_width);
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
                    let mut fg_color = if cell.flags.inverse {
                        resolve_background_color(cell.background)
                    } else {
                        resolve_foreground_color(cell.foreground)
                    };

                    // 检查是否是链接，修改颜色
                    let is_link = if links.is_empty() {
                        false
                    } else {
                        let mut found = false;
                        for link in links {
                            if link.line == row_idx && col_idx >= link.col_start && col_idx < link.col_end {
                                let is_hovered_link = hovered_link.as_ref().map(|l| l == link).unwrap_or(false);
                                // 链接颜色：蓝色或浅蓝色（悬停时）
                                fg_color = if is_hovered_link {
                                    Color32::from_rgb(100, 200, 255)  // 浅蓝
                                } else {
                                    Color32::from_rgb(50, 150, 255)   // 深蓝
                                };
                                found = true;
                                break;
                            }
                        }
                        found
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

                    // 链接显示下划线
                    if is_link {
                        let underline_y = y + line_height - 1.0;
                        painter.line_segment(
                            [egui::pos2(x, underline_y), egui::pos2(x + cell_width, underline_y)],
                            egui::Stroke::new(1.0, fg_color),
                        );
                    }

                    if cell.flags.underline {
                        let underline_y = y + line_height - 1.0;
                        painter.line_segment(
                            [egui::pos2(x, underline_y), egui::pos2(x + cell_width, underline_y)],
                            egui::Stroke::new(1.0, fg_color),
                        );
                    }

                    if cell.flags.strikethrough {
                        let strikethrough_y = y + line_height / 2.0;
                        painter.line_segment(
                            [egui::pos2(x, strikethrough_y), egui::pos2(x + cell_width, strikethrough_y)],
                            egui::Stroke::new(1.0, fg_color),
                        );
                    }
                }

                // Render cursor
                if (row_idx, col_idx) == cursor_pos && cursor_visible {
                    match &terminal.cursor_shape {
                        crate::terminal::CursorShape::Block => {
                            // Block cursor - filled rectangle
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
                        crate::terminal::CursorShape::Underline => {
                            // Underline cursor
                            let underline_y = y + line_height - 2.0;
                            painter.line_segment(
                                [egui::pos2(x, underline_y), egui::pos2(x + cell_width, underline_y)],
                                egui::Stroke::new(2.0, color::defaults::CURSOR),
                            );
                        }
                        crate::terminal::CursorShape::Beam => {
                            // Beam/vertical line cursor
                            painter.line_segment(
                                [egui::pos2(x + 1.0, y), egui::pos2(x + 1.0, y + line_height)],
                                egui::Stroke::new(1.5, color::defaults::CURSOR),
                            );
                        }
                    }
                }
            }
        }

        // Render IME preedit text below cursor
        if !terminal.preedit_text.is_empty() && terminal.ime_enabled {
            let preedit_display = format!("➜ {}", terminal.preedit_text);

            // 在光标下方显示预编辑文本
            let preedit_x = content_rect.left() + cursor_pos.1 as f32 * char_width;
            let preedit_y = content_rect.top() + cursor_pos.0 as f32 * line_height + line_height;

            // 确保不超出屏幕范围
            if preedit_y + line_height <= content_rect.bottom() {
                let font_id = FontId::monospace(self.font_size);
                let galley = ui.painter().layout_no_wrap(
                    preedit_display,
                    font_id,
                    Color32::from_rgb(200, 200, 0),  // 黄色标记
                );

                painter.galley(egui::pos2(preedit_x, preedit_y), galley, Color32::WHITE);
            }
        }

        // Draw scrollbar background and thumb
        if show_scrollbar {
            let track_color = if self.dragging_scrollbar {
                Color32::from_rgba_unmultiplied(92, 92, 100, 88)
            } else if scrollbar_hovered {
                Color32::from_rgba_unmultiplied(84, 84, 92, 64)
            } else {
                Color32::from_rgba_unmultiplied(72, 72, 80, 42)
            };
            painter.rect_filled(scrollbar_rect, 6.0, track_color);

            // Recompute thumb with current scroll_offset (may have changed from interaction)
            if let Some((_, scrollbar_height, _, scrollback_len_f)) = scrollbar_thumb_rect {
                let total_lines = terminal.scrollback.len() + rows;
                let visible_lines = rows;
                let thumb_height = ((visible_lines as f32 / total_lines as f32) * scrollbar_height)
                    .clamp(Self::MIN_THUMB_HEIGHT, scrollbar_height);
                let thumb_y = (terminal.scroll_offset as f32 / scrollback_len_f) * (scrollbar_height - thumb_height);
                let thumb_rect = egui::Rect::from_min_size(
                    egui::pos2(scrollbar_x, scrollbar_rect.top() + thumb_y),
                    egui::vec2(scrollbar_width, thumb_height),
                );

                // Visual feedback: thumb changes color when being dragged
                let thumb_color = if self.dragging_scrollbar {
                    Color32::from_rgba_unmultiplied(188, 188, 196, 188)
                } else if scrollbar_hovered {
                    Color32::from_rgba_unmultiplied(166, 166, 176, 156)
                } else {
                    Color32::from_rgba_unmultiplied(146, 146, 156, 118)
                };
                painter.rect_filled(thumb_rect.shrink2(egui::vec2(1.0, 0.0)), 6.0, thumb_color);
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
                            egui::Key::D => {}, // Ctrl+D (handled by keybindings system - close session/EOF)
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
                            egui::Key::W => {}, // Ctrl+W (handled by keybindings system - close session)
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
