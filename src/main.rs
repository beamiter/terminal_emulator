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
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();

            // Try to load system CJK fonts
            let cjk_font_paths = [
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJKsc-Regular.otf",
                "/usr/share/fonts/wenquanyi/wqy-zenhei.ttc",
            ];

            for path in &cjk_font_paths {
                if let Ok(font_data) = std::fs::read(path) {
                    fonts.font_data.insert(
                        "cjk".to_owned(),
                        std::sync::Arc::new(egui::FontData::from_owned(font_data)),
                    );
                    fonts.families
                        .get_mut(&egui::FontFamily::Monospace)
                        .unwrap()
                        .push("cjk".to_owned());
                    break;
                }
            }

            cc.egui_ctx.set_fonts(fonts);
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
    test_text: String,  // 用于测试 Ctrl+X/C/V
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
                eprintln!("✓ Shell session started successfully");
                (Some(session), Some(rx))
            }
            Err(e) => {
                eprintln!("✗ Failed to start shell: {}", e);
                (None, None)
            }
        };

        // 初始化终端显示
        let mut term = terminal;

        // 写入初始提示
        let init_msg = b"Terminal Emulator v0.4.0\r\nEnter your commands:\r\n$ ";
        term.process_input(init_msg);

        let renderer = TerminalRenderer::new(14.0, 0.0);  // padding 改为 0
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
            test_text: String::from("Test Ctrl+C/X/V here"),
        }
    }

    fn render_ui(&mut self, ctx: &egui::Context) {
        let terminal_guard = self.terminal.lock();
        let (width, height) = terminal_guard.get_dimensions();
        drop(terminal_guard); // 释放锁，允许后续修改
        self.cols = width;
        self.rows = height;

        // 测试窗口：显示 egui 原生 TextEdit
        egui::Window::new("Test KeyInput")
            .fixed_pos(egui::pos2(10.0, 10.0))
            .fixed_size(egui::vec2(400.0, 100.0))
            .show(ctx, |ui| {
                ui.label("Try Ctrl+C/X/V here to test:");
                ui.text_edit_multiline(&mut self.test_text);
                ui.label("(Check stderr for key events)");
            });

        // 使用 Area 来完全自定义布局，避免 panel 的 padding
        egui::Area::new(egui::Id::new("terminal_area"))
            .fixed_pos(egui::pos2(0.0, 130.0))
            .show(ctx, |ui| {
                let screen_size = ctx.viewport_rect().size();
                ui.set_max_size(egui::vec2(screen_size.x, screen_size.y - 130.0));
                let mut terminal_guard = self.terminal.lock();
                self.renderer.render(ui, &mut terminal_guard, self.cursor_visible);
            });
    }
}

impl eframe::App for TerminalApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // UI handled in update()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Step 1: 在 egui 层面先处理所有快捷键和粘贴事件
        let all_events = ctx.input(|i| i.events.clone());
        let mut consumed_keys = std::collections::HashSet::new();

        eprintln!("\n[Step1] 本帧接收 {} 个事件:", all_events.len());
        for (idx, evt) in all_events.iter().enumerate() {
            match evt {
                egui::Event::Key { key, physical_key, pressed, repeat, modifiers } => {
                    eprintln!("  [{}] Key: key={:?}, physical={:?}, pressed={}, repeat={}, ctrl={}, shift={}, alt={}, cmd={}",
                        idx, key, physical_key, pressed, repeat, modifiers.ctrl, modifiers.shift, modifiers.alt, modifiers.command);
                }
                egui::Event::Text(text) => {
                    let hex = text.chars()
                        .map(|c| format!("U+{:04X}", c as u32))
                        .collect::<Vec<_>>()
                        .join(" ");
                    eprintln!("  [{}] Text: {:?} ({})", idx, text, hex);
                }
                egui::Event::Paste(content) => {
                    eprintln!("  [{}] Paste: {} chars", idx, content.len());
                }
                _ => {
                    eprintln!("  [{}] Other: {:?}", idx, evt);
                }
            }
        }

        // 处理粘贴事件（Ctrl+V 会被转换成这个）
        for evt in &all_events {
            if let egui::Event::Paste(content) = evt {
                eprintln!("[Step1] ✓ Detected Paste event: {} chars", content.len());
                if !content.is_empty() {
                    if let Some(shell) = &self.shell {
                        let _ = shell.write(content.as_bytes());
                    } else {
                        let mut input_guard = self.input_queue.lock();
                        input_guard.extend(content.as_bytes());
                    }
                    consumed_keys.insert("Paste");
                }
            }
        }

        for evt in &all_events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = evt {
                // 打印所有 Ctrl+ 组合用于调试
                if modifiers.ctrl {
                    eprintln!("[Step1-Raw] Ctrl+{:?}, Shift: {}", key, modifiers.shift);
                }

                // === 快捷键优先级：复制/粘贴 > 终端信号 ===

                // Ctrl+C: 如果有选中，优先复制；否则发送 SIGINT
                if *key == egui::Key::C && modifiers.ctrl && !modifiers.shift {
                    if let Some(text) = self.terminal.lock().copy_selection() {
                        eprintln!("[Step1] ✓ Ctrl+C (copy selected text)");
                        if let Some(clipboard) = &self.clipboard {
                            let _ = clipboard.copy(&text);
                        }
                        consumed_keys.insert("Ctrl+C");
                    }
                    // 如果没有选中，放行给 handle_keyboard_input，发送 SIGINT
                }

                // Ctrl+X: 剪切（需要有选中文本）
                if *key == egui::Key::X && modifiers.ctrl && !modifiers.shift {
                    if let Some(text) = self.terminal.lock().copy_selection() {
                        eprintln!("[Step1] ✓ Ctrl+X (cut selected text)");
                        if let Some(clipboard) = &self.clipboard {
                            let _ = clipboard.copy(&text);
                        }
                        // 清除选中（剪切后）
                        self.terminal.lock().clear_selection();
                        consumed_keys.insert("Ctrl+X");
                    }
                }

                // Ctrl+V: 粘贴（通常被转换成 Paste 事件，这里作为备份）
                if *key == egui::Key::V && modifiers.ctrl && !modifiers.shift {
                    eprintln!("[Step1] ✓ Ctrl+V (paste) - as Key event");
                    if let Some(clipboard) = &self.clipboard {
                        if let Ok(text) = clipboard.paste() {
                            eprintln!("[Step1] Pasted: {} chars", text.len());
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
                    consumed_keys.insert("Ctrl+V");
                }

                // Ctrl+Shift+C: 强制复制
                if *key == egui::Key::C && modifiers.ctrl && modifiers.shift {
                    eprintln!("[Step1] ✓ Ctrl+Shift+C (force copy)");
                    if let Some(clipboard) = &self.clipboard {
                        let terminal = self.terminal.lock();
                        if let Some(text) = terminal.copy_selection() {
                            let _ = clipboard.copy(&text);
                        }
                    }
                    consumed_keys.insert("Ctrl+Shift+C");
                }

                // Ctrl+Shift+V: 强制粘贴
                if *key == egui::Key::V && modifiers.ctrl && modifiers.shift {
                    eprintln!("[Step1] ✓ Ctrl+Shift+V (force paste)");
                    if let Some(clipboard) = &self.clipboard {
                        if let Ok(text) = clipboard.paste() {
                            eprintln!("[Step1] Pasted: {} chars", text.len());
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
                    consumed_keys.insert("Ctrl+Shift+V");
                }
            }
        }

        // Step 2: 处理普通键盘输入，传给 handle_keyboard_input
        // 但排除已经被消费的按键
        let mut keyboard_input = Vec::new();
        eprintln!("[Step2-DEBUG] consumed_keys 包含: {:?}", consumed_keys);
        self.renderer
            .handle_keyboard_input(ctx, &mut keyboard_input, &consumed_keys);

        if !keyboard_input.is_empty() {
            let preview = keyboard_input.iter()
                .take(20)
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            eprintln!("[Step2-DEBUG] 收到键盘输入 ({} 字节): [{}{}]",
                keyboard_input.len(),
                preview,
                if keyboard_input.len() > 20 { " ..." } else { "" });

            let mut input_guard = self.input_queue.lock();
            input_guard.extend(keyboard_input);
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

        // 渲染 UI - 使用 Area 实现真正的全屏填充
        self.render_ui(ctx);

        // Request repaint for cursor blinking
        ctx.request_repaint();

        std::thread::sleep(Duration::from_millis(16));
    }
}
