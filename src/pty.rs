#[cfg(unix)]
mod unix_pty {
    use anyhow::{anyhow, Result};
    use std::os::unix::io::RawFd;

    pub struct Pty {
        master: RawFd,
        child_pid: Option<i32>,
    }

    impl Pty {
        pub fn new(cols: usize, rows: usize) -> Result<Self> {
            unsafe {
                let mut master = 0;

                let win_size = libc::winsize {
                    ws_row: rows as u16,
                    ws_col: cols as u16,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                // Try to open a PTY
                let res = libc::openpty(&mut master, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &win_size);

                if res != 0 {
                    return Err(anyhow!("Failed to open PTY"));
                }

                // Set master to non-blocking
                let flags = libc::fcntl(master, libc::F_GETFL, 0);
                if flags != -1 {
                    let _ = libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }

                Ok(Pty {
                    master,
                    child_pid: None,
                })
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

                if libc::ioctl(self.master, libc::TIOCSWINSZ, (&win_size) as *const _ as *mut libc::c_void) < 0 {
                    return Err(anyhow!("Failed to resize PTY"));
                }
            }
            Ok(())
        }
    }

    impl Drop for Pty {
        fn drop(&mut self) {
            if let Some(pid) = self.child_pid {
                unsafe {
                    let _ = libc::kill(pid, libc::SIGTERM);
                }
            }
            unsafe {
                let _ = libc::close(self.master);
            }
        }
    }
}

#[cfg(windows)]
mod windows_pty {
    use anyhow::{anyhow, Result};

    pub struct Pty {
        // Windows PTY implementation placeholder
    }

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
    }
}

#[cfg(unix)]
pub use unix_pty::Pty;

#[cfg(windows)]
pub use windows_pty::Pty;
