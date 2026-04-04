use crate::pty::Pty;
use crossbeam::channel::{Receiver, unbounded};
use eframe::egui;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub enum ShellEvent {
    Output(Vec<u8>),
    Exit(i32),
    Error(String),
}

/// ShellSession 管理 PTY 和后台 I/O 线程
pub struct ShellSession {
    pty: Arc<Mutex<Pty>>,
    event_rx: Receiver<ShellEvent>,
    pub is_running: bool,
    child_pid: i32,  // 存储 shell 子进程的 PID
}

impl ShellSession {
    /// 启动新的 shell session
    pub fn new(cols: usize, rows: usize, repaint_ctx: egui::Context) -> std::result::Result<Self, String> {
        Self::new_with_cwd(cols, rows, None, repaint_ctx)
    }

    /// 启动新的 shell session，指定初始工作目录
    pub fn new_with_cwd(
        cols: usize,
        rows: usize,
        cwd: Option<&str>,
        repaint_ctx: egui::Context,
    ) -> std::result::Result<Self, String> {
        match Pty::new_with_cwd(cols, rows, cwd) {
            Ok(pty) => {
                // 在把 pty 放入 Arc<Mutex> 前获取 child_pid
                let child_pid = pty.get_child_pid();

                let (event_tx, event_rx) = unbounded::<ShellEvent>();

                let pty = Arc::new(Mutex::new(pty));
                let pty_clone = Arc::clone(&pty);
                let repaint_ctx_clone = repaint_ctx.clone();

                thread::spawn(move || {
                    Self::io_loop(pty_clone, event_tx, repaint_ctx_clone);
                });

                Ok(ShellSession {
                    pty,
                    event_rx,
                    is_running: true,
                    child_pid,
                })
            }
            Err(e) => Err(format!("Failed to create shell session: {}", e)),
        }
    }

    /// 获取事件接收器（用于读取 shell 事件）
    pub fn events(&self) -> &Receiver<ShellEvent> {
        &self.event_rx
    }

    fn send_event(event_tx: &crossbeam::channel::Sender<ShellEvent>, repaint_ctx: &egui::Context, event: ShellEvent) -> bool {
        if event_tx.send(event).is_err() {
            return false;
        }
        repaint_ctx.request_repaint();
        true
    }

    /// 后台 I/O 循环 - 阻塞等待 PTY 可读，避免忙轮询
    fn io_loop(
        pty: Arc<Mutex<Pty>>,
        event_tx: crossbeam::channel::Sender<ShellEvent>,
        repaint_ctx: egui::Context,
    ) {
        let mut buf = vec![0u8; 4096];
        let mut last_alive_check = std::time::Instant::now();

        crate::debug_log!("[IOLoop] 后台 I/O 线程启动");

        loop {
            let timeout_ms = Duration::from_millis(100)
                .saturating_sub(last_alive_check.elapsed())
                .as_millis() as i32;

            let master_fd = match pty.lock() {
                Ok(pty_guard) => pty_guard.master_fd(),
                Err(_) => {
                    let _ = Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Error("PTY lock poisoned".to_string()));
                    return;
                }
            };

            match Pty::wait_fd_readable(master_fd, timeout_ms.max(0)) {
                Ok(true) => {}
                Ok(false) => {}
                Err(e) => {
                    if !Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Error(format!("Poll error: {}", e))) {
                        return;
                    }
                }
            }

            if let Ok(mut pty_guard) = pty.lock() {
                loop {
                    match pty_guard.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            let data = buf[..n].to_vec();
                            if !Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Output(data)) {
                                crate::debug_log!("[IOLoop] 接收者已断开，退出循环");
                                return;
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            crate::debug_log!("[IOLoop] 读取错误: {}", e);
                            if !pty_guard.is_alive() {
                                crate::debug_log!("[IOLoop] 读取失败且进程已退出，退出循环");
                                match pty_guard.wait_timeout(0) {
                                    Ok(exit_code) => {
                                        let _ = Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Exit(exit_code));
                                    }
                                    Err(e) => {
                                        let _ = Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Error(format!("Process exit error: {}", e)));
                                    }
                                }
                                return;
                            }
                            let _ = Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Error(format!("Read error: {}", e)));
                            break;
                        }
                    }
                }

                if last_alive_check.elapsed() >= Duration::from_millis(100) {
                    if !pty_guard.is_alive() {
                        crate::debug_log!("[IOLoop] 检测到子进程已退出");
                        match pty_guard.wait_timeout(0) {
                            Ok(exit_code) => {
                                crate::debug_log!("[IOLoop] 子进程退出码: {}", exit_code);
                                let _ = Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Exit(exit_code));
                            }
                            Err(e) => {
                                let _ = Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Error(format!(
                                    "Process exit error: {}",
                                    e
                                )));
                            }
                        }
                        return;
                    }
                    last_alive_check = std::time::Instant::now();
                }
            } else {
                let _ = Self::send_event(&event_tx, &repaint_ctx, ShellEvent::Error("PTY lock poisoned".to_string()));
                return;
            }
        }
    }

    /// 向 shell 发送输入数据（例如用户输入）
    pub fn write(&self, data: &[u8]) -> std::result::Result<(), String> {
        let has_sigint = data.contains(&0x03);
        let has_ctrl_x = data.contains(&0x18);
        let has_ctrl_v = data.contains(&0x16);

        if has_sigint || has_ctrl_x || has_ctrl_v {
            crate::debug_log!(
                "[IOLoop-DEBUG] 特殊字节码: SIGINT={} Ctrl+X={} Ctrl+V={}",
                has_sigint,
                has_ctrl_x,
                has_ctrl_v
            );
        }

        let preview = data.iter()
            .take(20)
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        crate::debug_log!(
            "[IOLoop] 输入已写入 PTY ({} 字节): [{}{}]",
            data.len(),
            preview,
            if data.len() > 20 { " ..." } else { "" }
        );

        let mut pty = self.pty.lock().map_err(|_| "Failed to lock PTY for write".to_string())?;
        pty.write(data)
            .map(|_| ())
            .map_err(|e| format!("Write error: {}", e))
    }

    pub fn resize(&self, cols: usize, rows: usize) -> std::result::Result<(), String> {
        let mut pty = self.pty.lock().map_err(|_| "Failed to lock PTY for resize".to_string())?;
        pty.resize(cols, rows)
            .map_err(|e| format!("Resize error: {}", e))
    }

    pub fn mark_exited(&mut self) {
        self.is_running = false;
    }

    /// 获取 shell 子进程的 PID
    pub fn get_child_pid(&self) -> i32 {
        self.child_pid
    }
}
