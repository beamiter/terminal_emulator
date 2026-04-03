use crate::pty::Pty;
use crossbeam::channel::{Receiver, Sender, unbounded};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::thread;

enum ShellCommand {
    Input(Vec<u8>),
    Resize { cols: usize, rows: usize },
}

#[derive(Clone, Debug)]
pub enum ShellEvent {
    Output(Vec<u8>),
    Exit(i32),
    Error(String),
}

/// ShellSession 管理 PTY 和后台 I/O 线程
pub struct ShellSession {
    input_tx: Sender<ShellCommand>,
    event_rx: Receiver<ShellEvent>,
    pub is_running: bool,
}

impl ShellSession {
    /// 启动新的 shell session
    pub fn new(cols: usize, rows: usize) -> std::result::Result<Self, String> {
        Self::new_with_cwd(cols, rows, None)
    }

    /// 启动新的 shell session，指定初始工作目录
    pub fn new_with_cwd(cols: usize, rows: usize, cwd: Option<&str>) -> std::result::Result<Self, String> {
        match Pty::new_with_cwd(cols, rows, cwd) {
            Ok(pty) => {
                let (input_tx, input_rx) = unbounded::<ShellCommand>();
                let (event_tx, event_rx) = unbounded::<ShellEvent>();

                // 将 PTY 和通道放入 Arc<Mutex> 供后台线程使用
                let pty = Arc::new(Mutex::new(pty));
                let pty_clone = Arc::clone(&pty);

                // 启动后台 I/O 线程
                thread::spawn(move || {
                    Self::io_loop(pty_clone, input_rx, event_tx);
                });

                Ok(ShellSession {
                    input_tx,
                    event_rx,
                    is_running: true,
                })
            }
            Err(e) => Err(format!("Failed to create shell session: {}", e)),
        }
    }

    /// 获取事件接收器（用于读取 shell 事件）
    pub fn events(&self) -> &Receiver<ShellEvent> {
        &self.event_rx
    }

    /// 后台 I/O 循环 - 持续读取 PTY 输出和处理输入
    fn io_loop(
        pty: Arc<Mutex<Pty>>,
        input_rx: Receiver<ShellCommand>,
        event_tx: Sender<ShellEvent>,
    ) {
        let mut buf = vec![0u8; 4096];
        let mut last_alive_check = std::time::Instant::now();
        let mut iteration = 0;

        crate::debug_log!("[IOLoop] 后台 I/O 线程启动");

        loop {
            iteration += 1;

            // 处理输入队列（非阻塞）
            while let Ok(command) = input_rx.try_recv() {
                if let Ok(mut pty_guard) = pty.lock() {
                    match command {
                        ShellCommand::Input(data) => {
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

                            if let Err(e) = pty_guard.write(&data) {
                                let _ = event_tx.send(ShellEvent::Error(format!("Write error: {}", e)));
                            }
                        }
                        ShellCommand::Resize { cols, rows } => {
                            if let Err(e) = pty_guard.resize(cols, rows) {
                                let _ = event_tx.send(ShellEvent::Error(format!("Resize error: {}", e)));
                            }
                        }
                    }
                }
            }

            // 读取 PTY 输出
            {
                if let Ok(mut pty_guard) = pty.lock() {
                    match pty_guard.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            let data = buf[..n].to_vec();
                            if event_tx.send(ShellEvent::Output(data)).is_err() {
                                crate::debug_log!("[IOLoop] 接收者已断开，退出循环");
                                return;
                            }
                        }
                        Err(e) => {
                            crate::debug_log!("[IOLoop] 读取错误: {}", e);
                            // 读取失败可能意味着进程已退出，立即检查
                            if !pty_guard.is_alive() {
                                crate::debug_log!("[IOLoop] 读取失败且进程已退出，退出循环");
                                match pty_guard.wait_timeout(0) {
                                    Ok(exit_code) => {
                                        let _ = event_tx.send(ShellEvent::Exit(exit_code));
                                    }
                                    Err(e) => {
                                        let _ = event_tx.send(ShellEvent::Error(format!("Process exit error: {}", e)));
                                    }
                                }
                                return;
                            }
                            // 进程仍活着但读取失败（可能是临时网络问题等），继续
                            let _ = event_tx.send(ShellEvent::Error(format!("Read error: {}", e)));
                        }
                        _ => {
                            // 非阻塞读，没有数据时不处理
                        }
                    }

                    // 每100ms检查一次子进程状态
                    if last_alive_check.elapsed() > Duration::from_millis(100) {
                        if !pty_guard.is_alive() {
                            crate::debug_log!("[IOLoop] 检测到子进程已退出");
                            match pty_guard.wait_timeout(0) {
                                Ok(exit_code) => {
                                    crate::debug_log!("[IOLoop] 子进程退出码: {}", exit_code);
                                    let _ = event_tx.send(ShellEvent::Exit(exit_code));
                                }
                                Err(e) => {
                                    let _ = event_tx.send(ShellEvent::Error(format!(
                                        "Process exit error: {}",
                                        e
                                    )));
                                }
                            }
                            return;
                        }
                        last_alive_check = std::time::Instant::now();
                    }
                }
            }

            // 每1000次迭代（约10秒）输出一次状态
            if iteration % 1000 == 0 {
                crate::debug_log!("[IOLoop] 后台线程仍在运行... (迭代: {})", iteration);
            }

            // 睡眠以防止 CPU 忙轮询，保留 UI 响应性
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 向 shell 发送输入数据（例如用户输入）
    pub fn write(&self, data: &[u8]) -> std::result::Result<(), String> {
        self.input_tx
            .send(ShellCommand::Input(data.to_vec()))
            .map_err(|e| format!("Failed to send input: {}", e))
    }

    pub fn resize(&self, cols: usize, rows: usize) -> std::result::Result<(), String> {
        self.input_tx
            .send(ShellCommand::Resize { cols, rows })
            .map_err(|e| format!("Failed to send resize: {}", e))
    }

    pub fn mark_exited(&mut self) {
        self.is_running = false;
    }
}
