use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
    BrightBlack, BrightRed, BrightGreen, BrightYellow,
    BrightBlue, BrightMagenta, BrightCyan, BrightWhite,
    Indexed(u8),
    Rgb(u8, u8, u8),
    Default,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StyleFlags {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub dim: bool,
}

#[derive(Clone, Debug)]
pub struct TerminalCell {
    pub character: char,
    pub foreground: Color,
    pub background: Color,
    pub flags: StyleFlags,
}

impl Default for TerminalCell {
    fn default() -> Self {
        TerminalCell {
            character: ' ',
            foreground: Color::Default,
            background: Color::Default,
            flags: StyleFlags::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

pub struct TerminalState {
    pub grid: Vec<Vec<TerminalCell>>,
    pub scrollback: VecDeque<Vec<TerminalCell>>,
    pub selection: Option<Selection>,
    pub scroll_offset: usize,
    max_scrollback: usize,

    pub cursor_row: usize,
    pub cursor_col: usize,
    saved_cursor_row: usize,
    saved_cursor_col: usize,

    current_fg: Color,
    current_bg: Color,
    current_flags: StyleFlags,
}

impl TerminalState {
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = vec![vec![TerminalCell::default(); cols]; rows];

        TerminalState {
            grid,
            scrollback: VecDeque::new(),
            selection: None,
            scroll_offset: 0,
            max_scrollback: 10000,
            cursor_row: 0,
            cursor_col: 0,
            saved_cursor_row: 0,
            saved_cursor_col: 0,
            current_fg: Color::Default,
            current_bg: Color::Default,
            current_flags: StyleFlags::default(),
        }
    }

    pub fn process_input(&mut self, input: &[u8]) {
        let mut i = 0;

        while i < input.len() {
            let byte = input[i];

            match byte {
                b'\x08' => {
                    if self.cursor_col > 0 {
                        self.cursor_col -= 1;
                    }
                    i += 1;
                }
                b'\n' => {
                    self.cursor_row += 1;
                    if self.cursor_row >= self.grid.len() {
                        self.cursor_row = self.grid.len() - 1;
                        self.scroll_down();
                    }
                    i += 1;
                }
                b'\r' => {
                    self.cursor_col = 0;
                    i += 1;
                }
                b'\x07' => {
                    // Bell - ignore
                    i += 1;
                }
                b'\t' => {
                    // Tab
                    self.cursor_col = ((self.cursor_col + 8) / 8) * 8;
                    if self.cursor_col >= self.grid[0].len() {
                        self.cursor_col = self.grid[0].len() - 1;
                    }
                    i += 1;
                }
                b'\x1b' if i + 1 < input.len() && input[i + 1] == b'[' => {
                    // Escape sequence
                    i += 2;
                    let mut params = Vec::new();
                    let mut param_str = String::new();

                    // Parse numeric parameters
                    while i < input.len() && (input[i].is_ascii_digit() || input[i] == b';') {
                        if input[i] == b';' {
                            if let Ok(n) = param_str.parse::<u16>() {
                                params.push(n);
                            }
                            param_str.clear();
                        } else {
                            param_str.push(input[i] as char);
                        }
                        i += 1;
                    }

                    if !param_str.is_empty() {
                        if let Ok(n) = param_str.parse::<u16>() {
                            params.push(n);
                        }
                    }

                    if i < input.len() {
                        let cmd = input[i] as char;
                        self.handle_escape_sequence(&params, cmd);
                        i += 1;
                    }
                }
                32..=126 => {
                    // Printable character
                    if self.cursor_col >= self.grid[self.cursor_row].len() {
                        self.cursor_col = 0;
                        self.cursor_row += 1;
                        if self.cursor_row >= self.grid.len() {
                            self.cursor_row = self.grid.len() - 1;
                            self.scroll_down();
                        }
                    }

                    let cell = &mut self.grid[self.cursor_row][self.cursor_col];
                    cell.character = byte as char;
                    cell.foreground = self.current_fg;
                    cell.background = self.current_bg;
                    cell.flags = self.current_flags;

                    self.cursor_col += 1;
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
    }

    fn handle_escape_sequence(&mut self, params: &[u16], cmd: char) {
        match cmd {
            'A' => {
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' => {
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.grid.len() - 1);
            }
            'C' => {
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.grid[0].len() - 1);
            }
            'D' => {
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            'E' => {
                // Move cursor down and to start of line
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.grid.len() - 1);
                self.cursor_col = 0;
            }
            'F' => {
                // Move cursor up and to start of line
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
                self.cursor_col = 0;
            }
            'G' => {
                // Move cursor to column
                let col = params.first().copied().unwrap_or(1) as usize;
                self.cursor_col = col.saturating_sub(1).min(self.grid[0].len() - 1);
            }
            'H' | 'f' => {
                let row = params.get(0).copied().unwrap_or(1) as usize;
                let col = params.get(1).copied().unwrap_or(1) as usize;
                self.cursor_row = row.saturating_sub(1).min(self.grid.len() - 1);
                self.cursor_col = col.saturating_sub(1).min(self.grid[0].len() - 1);
            }
            'J' => {
                match params.first().copied().unwrap_or(0) {
                    0 => {
                        // Clear from cursor to end of display
                        for col in self.cursor_col..self.grid[0].len() {
                            self.grid[self.cursor_row][col] = TerminalCell::default();
                        }
                        for row in (self.cursor_row + 1)..self.grid.len() {
                            for col in 0..self.grid[0].len() {
                                self.grid[row][col] = TerminalCell::default();
                            }
                        }
                    }
                    1 => {
                        // Clear from start to cursor
                        for row in 0..=self.cursor_row {
                            let end_col = if row == self.cursor_row {
                                self.cursor_col + 1
                            } else {
                                self.grid[0].len()
                            };
                            for col in 0..end_col {
                                self.grid[row][col] = TerminalCell::default();
                            }
                        }
                    }
                    2 => self.clear_screen(),
                    _ => {}
                }
            }
            'K' => {
                // Clear line
                match params.first().copied().unwrap_or(0) {
                    0 => {
                        // Clear from cursor to end of line
                        for col in self.cursor_col..self.grid[0].len() {
                            self.grid[self.cursor_row][col] = TerminalCell::default();
                        }
                    }
                    1 => {
                        // Clear from start of line to cursor
                        for col in 0..=self.cursor_col {
                            self.grid[self.cursor_row][col] = TerminalCell::default();
                        }
                    }
                    2 => {
                        // Clear entire line
                        for col in 0..self.grid[0].len() {
                            self.grid[self.cursor_row][col] = TerminalCell::default();
                        }
                    }
                    _ => {}
                }
            }
            'm' => {
                // SGR - Select Graphic Rendition
                self.handle_sgr(params);
            }
            's' => {
                self.saved_cursor_row = self.cursor_row;
                self.saved_cursor_col = self.cursor_col;
            }
            'u' => {
                self.cursor_row = self.saved_cursor_row.min(self.grid.len() - 1);
                self.cursor_col = self.saved_cursor_col.min(self.grid[0].len() - 1);
            }
            'S' => {
                // Scroll up
                let n = params.first().copied().unwrap_or(1) as usize;
                for _ in 0..n {
                    if self.grid.len() > 0 {
                        let cols = self.grid[0].len();
                        let old_line = std::mem::replace(
                            &mut self.grid[0],
                            vec![TerminalCell::default(); cols],
                        );
                        self.grid.remove(0);
                        self.grid.push(vec![TerminalCell::default(); cols]);
                        if self.scrollback.len() >= self.max_scrollback {
                            self.scrollback.pop_front();
                        }
                        self.scrollback.push_back(old_line);
                    }
                }
            }
            'T' => {
                // Scroll down
                let n = params.first().copied().unwrap_or(1) as usize;
                for _ in 0..n {
                    if self.grid.len() > 0 {
                        let cols = self.grid[0].len();
                        self.grid.pop();
                        self.grid.insert(0, vec![TerminalCell::default(); cols]);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_sgr(&mut self, params: &[u16]) {
        let mut i = 0;
        while i < params.len() {
            let param = params[i];
            match param {
                0 => {
                    self.current_flags = StyleFlags::default();
                    self.current_fg = Color::Default;
                    self.current_bg = Color::Default;
                }
                1 => self.current_flags.bold = true,
                2 => self.current_flags.dim = true,
                3 => self.current_flags.italic = true,
                4 => self.current_flags.underline = true,
                7 => self.current_flags.inverse = true,
                22 => self.current_flags.bold = false,
                23 => self.current_flags.italic = false,
                24 => self.current_flags.underline = false,
                27 => self.current_flags.inverse = false,
                30..=37 => {
                    self.current_fg = match param {
                        30 => Color::Black,
                        31 => Color::Red,
                        32 => Color::Green,
                        33 => Color::Yellow,
                        34 => Color::Blue,
                        35 => Color::Magenta,
                        36 => Color::Cyan,
                        37 => Color::White,
                        _ => Color::Default,
                    };
                }
                40..=47 => {
                    self.current_bg = match param {
                        40 => Color::Black,
                        41 => Color::Red,
                        42 => Color::Green,
                        43 => Color::Yellow,
                        44 => Color::Blue,
                        45 => Color::Magenta,
                        46 => Color::Cyan,
                        47 => Color::White,
                        _ => Color::Default,
                    };
                }
                90..=97 => {
                    self.current_fg = match param {
                        90 => Color::BrightBlack,
                        91 => Color::BrightRed,
                        92 => Color::BrightGreen,
                        93 => Color::BrightYellow,
                        94 => Color::BrightBlue,
                        95 => Color::BrightMagenta,
                        96 => Color::BrightCyan,
                        97 => Color::BrightWhite,
                        _ => Color::Default,
                    };
                }
                100..=107 => {
                    self.current_bg = match param {
                        100 => Color::BrightBlack,
                        101 => Color::BrightRed,
                        102 => Color::BrightGreen,
                        103 => Color::BrightYellow,
                        104 => Color::BrightBlue,
                        105 => Color::BrightMagenta,
                        106 => Color::BrightCyan,
                        107 => Color::BrightWhite,
                        _ => Color::Default,
                    };
                }
                // Extended color support: 38;5;n (256 color) and 38;2;r;g;b (RGB)
                38 => {
                    if i + 2 < params.len() {
                        match params[i + 1] {
                            5 => {
                                // 256 color mode
                                self.current_fg = Color::Indexed(params[i + 2] as u8);
                                i += 2;
                            }
                            2 => {
                                // RGB mode
                                if i + 4 < params.len() {
                                    self.current_fg = Color::Rgb(
                                        params[i + 2] as u8,
                                        params[i + 3] as u8,
                                        params[i + 4] as u8,
                                    );
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                48 => {
                    if i + 2 < params.len() {
                        match params[i + 1] {
                            5 => {
                                // 256 color mode for background
                                self.current_bg = Color::Indexed(params[i + 2] as u8);
                                i += 2;
                            }
                            2 => {
                                // RGB mode for background
                                if i + 4 < params.len() {
                                    self.current_bg = Color::Rgb(
                                        params[i + 2] as u8,
                                        params[i + 3] as u8,
                                        params[i + 4] as u8,
                                    );
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn clear_screen(&mut self) {
        for row in &mut self.grid {
            for cell in row {
                *cell = TerminalCell::default();
            }
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn scroll_down(&mut self) {
        if self.grid.len() > 0 {
            let cols = self.grid[0].len();
            let old_line = std::mem::replace(&mut self.grid[0], vec![TerminalCell::default(); cols]);
            self.grid.remove(0);
            self.grid.push(vec![TerminalCell::default(); cols]);

            if self.scrollback.len() >= self.max_scrollback {
                self.scrollback.pop_front();
            }
            self.scrollback.push_back(old_line);
        }
    }

    pub fn get_visible_cells(&self) -> Vec<Vec<TerminalCell>> {
        self.grid.clone()
    }

    pub fn get_cursor_pos(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    pub fn select_text(&mut self, start: (usize, usize), end: (usize, usize)) {
        let (start, end) = if start < end { (start, end) } else { (end, start) };
        self.selection = Some(Selection { start, end });
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn copy_selection(&self) -> Option<String> {
        self.selection.map(|sel| {
            let mut result = String::new();
            let cols = self.grid[0].len();

            if sel.start.0 == sel.end.0 {
                for col in sel.start.1..=sel.end.1.min(cols - 1) {
                    result.push(self.grid[sel.start.0][col].character);
                }
            } else {
                for row in sel.start.0..=sel.end.0.min(self.grid.len() - 1) {
                    let start_col = if row == sel.start.0 { sel.start.1 } else { 0 };
                    let end_col = if row == sel.end.0 {
                        sel.end.1.min(cols - 1)
                    } else {
                        cols - 1
                    };

                    for col in start_col..=end_col {
                        result.push(self.grid[row][col].character);
                    }

                    if row < sel.end.0 {
                        result.push('\n');
                    }
                }
            }

            result
        })
    }

    pub fn scroll(&mut self, lines: isize) {
        if lines > 0 {
            self.scroll_offset = self.scroll_offset.saturating_add(lines as usize);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub((-lines) as usize);
        }
        self.scroll_offset = self.scroll_offset.min(self.scrollback.len());
    }

    pub fn on_resize(&mut self, cols: usize, rows: usize) {
        self.grid = vec![vec![TerminalCell::default(); cols]; rows];
        self.scroll_offset = 0;
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    pub fn get_dimensions(&self) -> (usize, usize) {
        if self.grid.is_empty() {
            (0, 0)
        } else {
            (self.grid[0].len(), self.grid.len())
        }
    }

    pub fn is_cell_selected(&self, row: usize, col: usize) -> bool {
        if let Some(sel) = self.selection {
            let (start, end) = if sel.start <= sel.end {
                (sel.start, sel.end)
            } else {
                (sel.end, sel.start)
            };

            if row < start.0 || row > end.0 {
                return false;
            }

            if row == start.0 && row == end.0 {
                col >= start.1 && col <= end.1
            } else if row == start.0 {
                col >= start.1
            } else if row == end.0 {
                col <= end.1
            } else {
                true
            }
        } else {
            false
        }
    }
}
