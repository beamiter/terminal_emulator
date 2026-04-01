use std::collections::VecDeque;
use unicode_width::UnicodeWidthChar;

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
    pub wide: bool,
    pub wide_continuation: bool,
}

impl Default for TerminalCell {
    fn default() -> Self {
        TerminalCell {
            character: ' ',
            foreground: Color::Default,
            background: Color::Default,
            flags: StyleFlags::default(),
            wide: false,
            wide_continuation: false,
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
    alt_grid: Vec<Vec<TerminalCell>>,
    pub scrollback: VecDeque<Vec<TerminalCell>>,
    pub selection: Option<Selection>,
    pub scroll_offset: usize,
    max_scrollback: usize,
    use_alt_buffer: bool,

    pub cursor_row: usize,
    pub cursor_col: usize,
    saved_cursor_row: usize,
    saved_cursor_col: usize,
    alt_cursor_row: usize,
    alt_cursor_col: usize,

    current_fg: Color,
    current_bg: Color,
    current_flags: StyleFlags,
    pub window_title: String,

    // Scrolling region (DECSTBM)
    scroll_region_top: usize,
    scroll_region_bottom: usize,

    // UTF-8 decoding buffer
    utf8_buf: [u8; 4],
    utf8_len: u8,
    utf8_expected: u8,

    // IME support
    pub ime_enabled: bool,
    pub preedit_text: String,
    pub preedit_cursor: usize,

    // DECSET modes
    modes: std::collections::HashSet<u16>,
}

impl TerminalState {
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = vec![vec![TerminalCell::default(); cols]; rows];
        let alt_grid = vec![vec![TerminalCell::default(); cols]; rows];

        let mut modes = std::collections::HashSet::new();
        modes.insert(25); // Cursor visible by default

        TerminalState {
            grid,
            alt_grid,
            scrollback: VecDeque::new(),
            selection: None,
            scroll_offset: 0,
            max_scrollback: 10000,
            use_alt_buffer: false,
            cursor_row: 0,
            cursor_col: 0,
            saved_cursor_row: 0,
            saved_cursor_col: 0,
            alt_cursor_row: 0,
            alt_cursor_col: 0,
            current_fg: Color::Default,
            current_bg: Color::Default,
            current_flags: StyleFlags::default(),
            window_title: String::new(),
            utf8_buf: [0; 4],
            utf8_len: 0,
            utf8_expected: 0,
            ime_enabled: false,
            preedit_text: String::new(),
            preedit_cursor: 0,
            scroll_region_top: 0,
            scroll_region_bottom: rows.saturating_sub(1),
            modes,
        }
    }

    fn put_char(&mut self, ch: char) {
        let width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width == 0 {
            return; // Skip zero-width characters for now
        }

        let cols = self.grid[self.cursor_row].len();

        // If wide character doesn't fit at end of line, wrap to next line
        if self.cursor_col + width > cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
            if self.cursor_row >= self.grid.len() {
                self.cursor_row = self.grid.len() - 1;
                self.scroll_down();
            }
        }

        // If current position has a continuation cell to its left, clear the wide character
        if self.cursor_col > 0 && self.grid[self.cursor_row][self.cursor_col].wide_continuation {
            self.grid[self.cursor_row][self.cursor_col - 1] = TerminalCell::default();
        }

        // If current position has a wide character, clear its continuation cell
        if self.grid[self.cursor_row][self.cursor_col].wide && self.cursor_col + 1 < cols {
            self.grid[self.cursor_row][self.cursor_col + 1] = TerminalCell::default();
        }

        // Write character
        let cell = &mut self.grid[self.cursor_row][self.cursor_col];
        cell.character = ch;
        cell.foreground = self.current_fg;
        cell.background = self.current_bg;
        cell.flags = self.current_flags;
        cell.wide = width == 2;
        cell.wide_continuation = false;

        // Set up wide character continuation cell if needed
        if width == 2 && self.cursor_col + 1 < cols {
            let cont_cell = &mut self.grid[self.cursor_row][self.cursor_col + 1];
            *cont_cell = TerminalCell::default();
            cont_cell.wide_continuation = true;
        }

        self.cursor_col += width;
    }

    fn clear_cell(&mut self, row: usize, col: usize) {
        let cols = self.grid[row].len();
        // If clearing a continuation cell, also clear the wide character body
        if self.grid[row][col].wide_continuation && col > 0 {
            self.grid[row][col - 1] = TerminalCell::default();
        }
        // If clearing a wide character body, also clear the continuation cell
        if self.grid[row][col].wide && col + 1 < cols {
            self.grid[row][col + 1] = TerminalCell::default();
        }
        self.grid[row][col] = TerminalCell::default();
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
                b'\x1b' if i + 1 < input.len() && input[i + 1] == b']' => {
                    // OSC (Operating System Command) sequence
                    // Format: ESC ] ... BEL or ESC ] ... ESC \
                    i += 2;  // Skip ESC ]

                    // Read until BEL (0x07) or ESC \ (0x1b 0x5c)
                    while i < input.len() {
                        if input[i] == 0x07 {
                            // BEL terminator
                            i += 1;
                            break;
                        } else if i + 1 < input.len() && input[i] == 0x1b && input[i + 1] == 0x5c {
                            // ESC \ terminator
                            i += 2;
                            break;
                        } else {
                            i += 1;
                        }
                    }
                }
                b'\x1b' if i + 1 < input.len() && input[i + 1] == b'[' => {
                    // CSI (Control Sequence Introducer) - normal escape sequence
                    i += 2;

                    // Skip private mode indicator (?)
                    let is_private_mode = i < input.len() && input[i] == b'?';
                    if is_private_mode {
                        i += 1;
                    }

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
                        if is_private_mode {
                            eprintln!("[ANSI-RAW] CSI ? (private mode) cmd={}", cmd);
                        }
                        self.handle_escape_sequence(&params, cmd);
                        i += 1;
                    }
                }
                32..=126 => {
                    // ASCII printable character
                    self.put_char(byte as char);
                    i += 1;
                }
                // UTF-8 2-byte sequence (0xC2-0xDF)
                0xC2..=0xDF => {
                    self.utf8_buf[0] = byte;
                    self.utf8_len = 1;
                    self.utf8_expected = 2;
                    i += 1;
                }
                // UTF-8 3-byte sequence (0xE0-0xEF)
                0xE0..=0xEF => {
                    self.utf8_buf[0] = byte;
                    self.utf8_len = 1;
                    self.utf8_expected = 3;
                    i += 1;
                }
                // UTF-8 4-byte sequence (0xF0-0xF4)
                0xF0..=0xF4 => {
                    self.utf8_buf[0] = byte;
                    self.utf8_len = 1;
                    self.utf8_expected = 4;
                    i += 1;
                }
                _ => {
                    // Invalid byte or continuation byte with no sequence - skip
                    if self.utf8_len > 0 && (byte & 0xC0) == 0x80 {
                        // UTF-8 continuation byte
                        self.utf8_buf[self.utf8_len as usize] = byte;
                        self.utf8_len += 1;
                        if self.utf8_len == self.utf8_expected {
                            // Sequence complete, decode it
                            if let Ok(s) = std::str::from_utf8(&self.utf8_buf[..self.utf8_len as usize]) {
                                if let Some(ch) = s.chars().next() {
                                    self.put_char(ch);
                                }
                            }
                            self.utf8_len = 0;
                        }
                    } else {
                        // Invalid continuation byte or stray byte - reset buffer and skip
                        self.utf8_len = 0;
                    }
                    i += 1;
                }
            }
        }
    }

    fn handle_escape_sequence(&mut self, params: &[u16], cmd: char) {
        // Debug logging for vim commands
        let params_str = params.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(";");
        eprintln!("[ANSI] CSI {}{}{}  (cursor: {},{}, use_alt: {})",
            if params_str.is_empty() { "(default)".to_string() } else { params_str.clone() },
            if !params.is_empty() { ":" } else { "" },
            cmd,
            self.cursor_row, self.cursor_col, self.use_alt_buffer);

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
                            self.clear_cell(self.cursor_row, col);
                        }
                        for row in (self.cursor_row + 1)..self.grid.len() {
                            for col in 0..self.grid[0].len() {
                                self.clear_cell(row, col);
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
                                self.clear_cell(row, col);
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
                            self.clear_cell(self.cursor_row, col);
                        }
                    }
                    1 => {
                        // Clear from start of line to cursor
                        for col in 0..=self.cursor_col {
                            self.clear_cell(self.cursor_row, col);
                        }
                    }
                    2 => {
                        // Clear entire line
                        for col in 0..self.grid[0].len() {
                            self.clear_cell(self.cursor_row, col);
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
                // Scroll up (Scroll Up, SU) - content moves up, new lines appear at bottom
                let n = params.first().copied().unwrap_or(1) as usize;
                eprintln!("[ANSI-S] Scroll up {} lines in region {}-{}", n, self.scroll_region_top, self.scroll_region_bottom);

                // Scroll within the scroll region by moving lines
                for _ in 0..n {
                    if self.scroll_region_top < self.grid.len() && self.scroll_region_bottom < self.grid.len() && self.scroll_region_top <= self.scroll_region_bottom {
                        let cols = self.grid[self.scroll_region_top].len();

                        // Shift lines up within the region
                        let mut new_lines = Vec::new();

                        // Keep lines from top+1 to bottom
                        for i in (self.scroll_region_top + 1)..=self.scroll_region_bottom {
                            if i < self.grid.len() {
                                new_lines.push(self.grid[i].clone());
                            }
                        }

                        // Add a blank line at the bottom
                        new_lines.push(vec![TerminalCell::default(); cols]);

                        // Replace region lines
                        for (i, line) in new_lines.iter().enumerate() {
                            self.grid[self.scroll_region_top + i] = line.clone();
                        }

                        // Save scrolled-out line to scrollback only if it was the top line of the full screen
                        if self.scroll_region_top == 0 {
                            if self.scrollback.len() >= self.max_scrollback {
                                self.scrollback.pop_front();
                            }
                        }
                    }
                }
            }
            'T' => {
                // Scroll down (Scroll Down, SD) - content moves down, new lines appear at top
                let n = params.first().copied().unwrap_or(1) as usize;
                eprintln!("[ANSI-T] Scroll down {} lines in region {}-{}", n, self.scroll_region_top, self.scroll_region_bottom);

                // Scroll within the scroll region by moving lines
                for _ in 0..n {
                    if self.scroll_region_top < self.grid.len() && self.scroll_region_bottom < self.grid.len() && self.scroll_region_top <= self.scroll_region_bottom {
                        let cols = self.grid[self.scroll_region_top].len();

                        // Shift lines down within the region by collecting from bottom to top
                        let mut new_lines = vec![vec![TerminalCell::default(); cols]]; // New blank line at top

                        // Keep lines from top to bottom-1
                        for i in self.scroll_region_top..self.scroll_region_bottom {
                            if i < self.grid.len() {
                                new_lines.push(self.grid[i].clone());
                            }
                        }

                        // Replace region lines
                        for (i, line) in new_lines.iter().enumerate() {
                            if self.scroll_region_top + i <= self.scroll_region_bottom {
                                self.grid[self.scroll_region_top + i] = line.clone();
                            }
                        }
                    }
                }
            }
            'n' => {
                // DSR - Device Status Report
                // ESC[6n requests cursor position
                if params.first().copied().unwrap_or(0) == 6 {
                    // Respond with CPR (Cursor Position Report): ESC[row;colR
                    // Row and Col are 1-indexed
                    let row = (self.cursor_row + 1) as u16;
                    let col = (self.cursor_col + 1) as u16;

                    // Store response for the application to read
                    // For now, we'll print it to debug
                    eprintln!("[CPR] Cursor at {}; {}", row, col);
                    // TODO: Send response back to application via PTY
                }
            }
            'h' => {
                // Set mode (DECSET)
                for &mode in params {
                    self.set_mode(mode);
                }
            }
            'l' => {
                // Reset mode (DECRST)
                for &mode in params {
                    self.reset_mode(mode);
                }
            }
            'r' => {
                // Set scroll region (DECSTBM)
                let top = params.get(0).copied().unwrap_or(1) as usize;
                let bottom = params.get(1).copied().unwrap_or(self.grid.len() as u16) as usize;

                // Convert from 1-indexed to 0-indexed, and clamp to valid range
                self.scroll_region_top = top.saturating_sub(1).min(self.grid.len().saturating_sub(1));
                self.scroll_region_bottom = bottom.saturating_sub(1).min(self.grid.len().saturating_sub(1));

                // If range is invalid, reset to full screen
                if self.scroll_region_top > self.scroll_region_bottom {
                    self.scroll_region_top = 0;
                    self.scroll_region_bottom = self.grid.len().saturating_sub(1);
                }

                eprintln!("[ANSI-r] Set scroll region: {} to {}", self.scroll_region_top, self.scroll_region_bottom);

                // Move cursor to home position when setting scroll region
                self.cursor_row = 0;
                self.cursor_col = 0;
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

    fn set_mode(&mut self, mode: u16) {
        match mode {
            25 => {
                // Show cursor (mode 25)
                self.modes.insert(25);
            }
            1000 | 1001 | 1002 | 1003 => {
                // Mouse reporting modes
                self.modes.insert(mode);
            }
            1006 => {
                // SGR mouse reporting format
                self.modes.insert(mode);
            }
            1049 => {
                // Alternate screen buffer
                if !self.use_alt_buffer {
                    // Save main buffer state (cursor position)
                    self.saved_cursor_row = self.cursor_row;
                    self.saved_cursor_col = self.cursor_col;

                    // Switch to alternate buffer
                    std::mem::swap(&mut self.grid, &mut self.alt_grid);
                    self.alt_cursor_row = self.cursor_row;
                    self.alt_cursor_col = self.cursor_col;
                    self.use_alt_buffer = true;

                    // Clear alt buffer and move cursor to home
                    self.clear_screen();
                    self.modes.insert(1049);
                }
            }
            7 => {
                // Autowrap mode
                self.modes.insert(7);
            }
            _ => {
                // Unknown mode, just store it
                self.modes.insert(mode);
            }
        }
    }

    fn reset_mode(&mut self, mode: u16) {
        match mode {
            25 => {
                // Hide cursor
                self.modes.remove(&25);
            }
            1000 | 1001 | 1002 | 1003 => {
                // Disable mouse reporting
                self.modes.remove(&mode);
            }
            1006 => {
                // Disable SGR mouse reporting format
                self.modes.remove(&mode);
            }
            1049 => {
                // Restore main screen buffer
                if self.use_alt_buffer {
                    // Save alt buffer state (cursor position)
                    self.alt_cursor_row = self.cursor_row;
                    self.alt_cursor_col = self.cursor_col;

                    // Switch back to main buffer
                    std::mem::swap(&mut self.grid, &mut self.alt_grid);
                    self.cursor_row = self.saved_cursor_row;
                    self.cursor_col = self.saved_cursor_col;
                    self.use_alt_buffer = false;
                    self.modes.remove(&1049);
                }
            }
            7 => {
                // Disable autowrap
                self.modes.remove(&7);
            }
            _ => {
                // Unknown mode, just remove it
                self.modes.remove(&mode);
            }
        }
    }

    pub fn is_cursor_visible(&self) -> bool {
        // Cursor is visible when mode 25 is SET (via \x1b[?25h)
        // Hidden when mode 25 is RESET (via \x1b[?25l)
        // Default to visible
        self.modes.contains(&25)
    }

    pub fn get_mouse_report(&self, button: u8, col: usize, row: usize) -> Option<String> {
        // Check if any mouse reporting mode is enabled
        if !self.modes.contains(&1000) && !self.modes.contains(&1002) && !self.modes.contains(&1003) {
            return None;
        }

        // SGR format (mode 1006) is preferred: CSI < button ; col ; row M/m
        // Standard format (mode 1000/1002): CSI M button col row (3 bytes)

        if self.modes.contains(&1006) {
            // SGR format: CSI < button ; x ; y M (button press) or m (button release)
            // For now, we'll generate press events (M) - release tracking would need more state
            let x = (col as u32 + 1).min(255); // 1-indexed, max 255
            let y = (row as u32 + 1).min(255); // 1-indexed, max 255
            Some(format!("\x1b[<{};{};{}M", button, x, y))
        } else {
            // Standard xterm format: CSI M button col row (raw bytes)
            // Col and row are offset by 32 (space character)
            let button_byte = (32 + button) as u8;
            let col_byte = (32 + (col as u8).min(223)) as u8;
            let row_byte = (32 + (row as u8).min(223)) as u8;
            Some(format!("\x1b[M{}{}{}", button_byte as char, col_byte as char, row_byte as char))
        }
    }

    pub fn is_mouse_enabled(&self) -> bool {
        self.modes.contains(&1000) || self.modes.contains(&1002) || self.modes.contains(&1003)
    }

    pub fn is_mouse_motion_enabled(&self) -> bool {
        self.modes.contains(&1002) || self.modes.contains(&1003)
    }

    fn scroll_down(&mut self) {
        if self.grid.len() > 0 {
            eprintln!("[SCROLL] scroll_down() in buffer (alt={})", self.use_alt_buffer);
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
        let rows = self.grid.len();
        let cols = if rows > 0 { self.grid[0].len() } else { 80 };

        // If not scrolling back, show current grid
        if self.scroll_offset == 0 {
            return self.grid.clone();
        }

        // Build view from scrollback + current grid
        let mut result = Vec::new();

        // Show lines from scrollback (if scroll_offset < scrollback.len())
        if self.scroll_offset > 0 && !self.scrollback.is_empty() {
            let start_idx = self.scrollback.len().saturating_sub(self.scroll_offset);
            for i in start_idx..self.scrollback.len() {
                if result.len() < rows {
                    result.push(self.scrollback[i].clone());
                }
            }
        }

        // Fill remaining rows with current grid
        for row in &self.grid {
            if result.len() < rows {
                result.push(row.clone());
            } else {
                break;
            }
        }

        // Pad with empty rows if needed
        while result.len() < rows {
            result.push(vec![TerminalCell::default(); cols]);
        }

        result
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
                    let cell = &self.grid[sel.start.0][col];
                    if !cell.wide_continuation {
                        result.push(cell.character);
                    }
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
                        let cell = &self.grid[row][col];
                        if !cell.wide_continuation {
                            result.push(cell.character);
                        }
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
            // Scroll up (show earlier lines)
            self.scroll_offset = self.scroll_offset.saturating_add(lines as usize);
        } else {
            // Scroll down (show later lines)
            self.scroll_offset = self.scroll_offset.saturating_sub((-lines) as usize);
        }

        // Clamp scroll_offset to valid range
        let max_scroll = self.scrollback.len();
        self.scroll_offset = self.scroll_offset.min(max_scroll);

        // When scrolling to bottom (offset 0), reset to live view
        if self.scroll_offset == 0 {
            self.scroll_offset = 0;
        }
    }

    pub fn reset_scroll(&mut self) {
        // Reset to showing live output (bottom of scrollback)
        self.scroll_offset = 0;
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

    // IME support methods
    pub fn set_preedit(&mut self, text: String, cursor: usize) {
        self.preedit_text = text;
        self.preedit_cursor = cursor;
    }

    pub fn clear_preedit(&mut self) {
        self.preedit_text.clear();
        self.preedit_cursor = 0;
    }

    pub fn commit_preedit(&mut self) -> String {
        let result = self.preedit_text.clone();
        self.clear_preedit();
        result
    }
}
