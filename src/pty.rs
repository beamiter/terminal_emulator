use anyhow::{anyhow, Result};
use std::os::unix::io::RawFd;
use std::ffi::CString;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

// 声明全局环境变量指针
extern "C" {
    static environ: *const *const libc::c_char;
}

#[cfg(unix)]
mod unix_pty {
    use super::*;

    pub struct Pty {
        master: RawFd,
        child_pid: i32,
        terminated: Arc<AtomicBool>,
    }

    impl Pty {
        pub fn new(cols: usize, rows: usize) -> Result<Self> {
            unsafe {
                // 1. 创建 PTY
                let mut master = 0;
                let mut slave = 0;

                let win_size = libc::winsize {
                    ws_row: rows as u16,
                    ws_col: cols as u16,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                if libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), &win_size) != 0 {
                    return Err(anyhow!("Failed to open PTY"));
                }

                // 2. 设置 master 非阻塞模式
                let flags = libc::fcntl(master, libc::F_GETFL, 0);
                if flags >= 0 {
                    let _ = libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }

                // 设置 FD_CLOEXEC，防止子进程继承
                let fd_flags = libc::fcntl(master, libc::F_GETFD, 0);
                if fd_flags >= 0 {
                    let _ = libc::fcntl(master, libc::F_SETFD, fd_flags | libc::FD_CLOEXEC);
                }

                // 3. Fork 子进程
                let fork_result = libc::fork();

                if fork_result < 0 {
                    libc::close(master);
                    libc::close(slave);
                    return Err(anyhow!("Failed to fork"));
                }

                if fork_result == 0 {
                    // 子进程分支
                    // 关闭 master
                    libc::close(master);

                    // 创建新的会话和进程组（将此进程设为会话leader）
                    libc::setsid();

                    // 设置 slave 为控制终端
                    if libc::ioctl(slave, libc::TIOCSCTTY, 0) != 0 {
                        libc::perror(b"ioctl TIOCSCTTY failed\0".as_ptr() as *const i8);
                    }

                    // 重定向 stdin/stdout/stderr 到 PTY slave
                    libc::dup2(slave, libc::STDIN_FILENO);
                    libc::dup2(slave, libc::STDOUT_FILENO);
                    libc::dup2(slave, libc::STDERR_FILENO);

                    // 关闭原始 slave fd（因为已经重定向了）
                    if slave > libc::STDERR_FILENO {
                        libc::close(slave);
                    }

                    // 获取 shell 路径 - 在 fork 之前可能更安全，但这里应该也可以
                    let shell_path = if let Ok(shell) = std::env::var("SHELL") {
                        shell
                    } else {
                        "/bin/bash".to_string()
                    };

                    let term_name = CString::new("TERM").unwrap();
                    let term_value = CString::new("xterm-256color").unwrap();
                    libc::setenv(term_name.as_ptr(), term_value.as_ptr(), 1);

                    let color_term_name = CString::new("COLORTERM").unwrap();
                    let color_term_value = CString::new("truecolor").unwrap();
                    libc::setenv(color_term_name.as_ptr(), color_term_value.as_ptr(), 1);

                    // 创建 C 字符串们
                    let shell_cstr = match CString::new(shell_path.clone()) {
                        Ok(s) => s,
                        Err(_) => {
                            libc::perror(b"Invalid shell path\0".as_ptr() as *const i8);
                            libc::exit(127);
                        }
                    };

                    // 使用简单的 "-bash" 作为 argv[0]
                    let dash_bash = match CString::new("-bash") {
                        Ok(s) => s,
                        Err(_) => {
                            libc::perror(b"Invalid shell name\0".as_ptr() as *const i8);
                            libc::exit(127);
                        }
                    };

                    // 构造 argv
                    let argv = [
                        dash_bash.as_ptr(),
                        std::ptr::null(),
                    ];

                    // 执行 shell，继承当前环境
                    libc::execve(shell_cstr.as_ptr(), argv.as_ptr(), environ);

                    // 如果 execve 返回，说明出错
                    libc::perror(b"execve failed\0".as_ptr() as *const i8);
                    libc::exit(127);
                } else {
                    // 父进程分支
                    // 关闭 slave
                    libc::close(slave);

                    let terminated = Arc::new(AtomicBool::new(false));

                    Ok(Pty {
                        master,
                        child_pid: fork_result as i32,
                        terminated,
                    })
                }
            }
        }

        pub fn write(&mut self, data: &[u8]) -> Result<usize> {
            unsafe {
                let n = libc::write(self.master, data.as_ptr() as *const _, data.len());
                if n < 0 {
                    Err(anyhow!("Failed to write to PTY: {}", std::io::Error::last_os_error()))
                } else {
                    Ok(n as usize)
                }
            }
        }

        pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            unsafe {
                let n = libc::read(self.master, buf.as_mut_ptr() as *mut _, buf.len());
                if n < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.kind() == std::io::ErrorKind::WouldBlock {
                        Ok(0)
                    } else {
                        Err(anyhow!("Failed to read from PTY: {}", err))
                    }
                } else {
                    Ok(n as usize)
                }
            }
        }

        pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
            unsafe {
                let win_size = libc::winsize {
                    ws_row: rows as u16,
                    ws_col: cols as u16,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                if libc::ioctl(
                    self.master,
                    libc::TIOCSWINSZ,
                    (&win_size) as *const _ as *mut libc::c_void,
                ) < 0
                {
                    return Err(anyhow!("Failed to resize PTY"));
                }
            }
            Ok(())
        }

        pub fn is_alive(&self) -> bool {
            unsafe {
                let mut status = 0;
                let result = libc::waitpid(self.child_pid, &mut status, libc::WNOHANG);
                result == 0  // 0 表示子进程还活着
            }
        }

        pub fn wait_timeout(&mut self, _timeout_ms: u64) -> Result<i32> {
            unsafe {
                let mut status = 0;
                let result = libc::waitpid(self.child_pid, &mut status, 0);

                if result < 0 {
                    Err(anyhow!("waitpid failed"))
                } else if libc::WIFEXITED(status) {
                    Ok(libc::WEXITSTATUS(status) as i32)
                } else if libc::WIFSIGNALED(status) {
                    Ok(-(libc::WTERMSIG(status) as i32))
                } else {
                    Ok(-1)
                }
            }
        }

        pub fn terminate(&mut self) -> Result<()> {
            unsafe {
                if libc::kill(self.child_pid, libc::SIGTERM) == 0 {
                    // 给进程时间优雅退出
                    std::thread::sleep(std::time::Duration::from_millis(100));

                    // 如果仍未退出，强制杀死
                    if self.is_alive() {
                        let _ = libc::kill(self.child_pid, libc::SIGKILL);
                    }
                    Ok(())
                } else {
                    Err(anyhow!("Failed to terminate child process"))
                }
            }
        }

        pub fn send_signal(&mut self, signal: i32) -> Result<()> {
            unsafe {
                if libc::kill(self.child_pid, signal) == 0 {
                    Ok(())
                } else {
                    Err(anyhow!("Failed to send signal {}", signal))
                }
            }
        }
    }

    impl Drop for Pty {
        fn drop(&mut self) {
            if self.is_alive() {
                let _ = self.terminate();
            }
            unsafe {
                let _ = libc::close(self.master);
            }
        }
    }
}

#[cfg(windows)]
mod windows_pty {
    use super::*;

    pub struct Pty;

    impl Pty {
        pub fn new(_cols: usize, _rows: usize) -> Result<Self> {
            Err(anyhow!("PTY support not yet implemented on Windows"))
        }

        pub fn write(&mut self, _data: &[u8]) -> Result<usize> {
            Err(anyhow!("PTY not available"))
        }

        pub fn read(&mut self, _buf: &mut [u8]) -> Result<usize> {
            Err(anyhow!("PTY not available"))
        }

        pub fn resize(&mut self, _cols: usize, _rows: usize) -> Result<()> {
            Err(anyhow!("PTY not available"))
        }

        pub fn is_alive(&self) -> bool {
            false
        }

        pub fn wait_timeout(&mut self, _timeout_ms: u64) -> Result<i32> {
            Err(anyhow!("PTY not available"))
        }

        pub fn terminate(&mut self) -> Result<()> {
            Err(anyhow!("PTY not available"))
        }

        pub fn send_signal(&mut self, _signal: i32) -> Result<()> {
            Err(anyhow!("PTY not available"))
        }
    }
}

#[cfg(unix)]
pub use unix_pty::Pty;

#[cfg(windows)]
pub use windows_pty::Pty;
