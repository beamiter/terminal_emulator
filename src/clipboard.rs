#[cfg(unix)]
mod unix_clipboard {
    use anyhow::Result;

    pub struct ClipboardManager;

    impl ClipboardManager {
        pub fn new() -> Result<Self> {
            Ok(ClipboardManager)
        }

        /// 复制文本到系统剪贴板
        pub fn copy(&self, text: &str) -> Result<()> {
            // 使用 xclip 命令（如果可用）
            use std::process::{Command, Stdio};
            use std::io::Write;

            match Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .stdin(Stdio::piped())
                .spawn()
            {
                Ok(mut child) => {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(text.as_bytes());
                    }
                    let _ = child.wait();
                    Ok(())
                }
                Err(_) => {
                    // xclip 不可用，尝试 xsel
                    match Command::new("xsel")
                        .arg("--clipboard")
                        .arg("--input")
                        .stdin(Stdio::piped())
                        .spawn()
                    {
                        Ok(mut child) => {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(text.as_bytes());
                            }
                            let _ = child.wait();
                            Ok(())
                        }
                        Err(_) => {
                            // 都不可用，静默失败
                            Ok(())
                        }
                    }
                }
            }
        }

        /// 从系统剪贴板粘贴文本
        pub fn paste(&self) -> Result<String> {
            use std::process::Command;

            // 尝试 xclip
            match Command::new("xclip")
                .arg("-selection")
                .arg("clipboard")
                .arg("-o")
                .output()
            {
                Ok(output) => {
                    return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
                }
                Err(_) => {}
            }

            // 尝试 xsel
            match Command::new("xsel")
                .arg("--clipboard")
                .arg("--output")
                .output()
            {
                Ok(output) => {
                    return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
                }
                Err(_) => {}
            }

            // 都失败了，返回空字符串
            Ok(String::new())
        }
    }
}

#[cfg(windows)]
mod windows_clipboard {
    use anyhow::Result;

    pub struct ClipboardManager;

    impl ClipboardManager {
        pub fn new() -> Result<Self> {
            Ok(ClipboardManager)
        }

        pub fn copy(&self, _text: &str) -> Result<()> {
            // Windows 剪贴板实现（需要 winapi）
            // 暂时实现为占位符
            Ok(())
        }

        pub fn paste(&self) -> Result<String> {
            // Windows 剪贴板实现（需要 winapi）
            // 暂时实现为占位符
            Ok(String::new())
        }
    }
}

#[cfg(unix)]
pub use unix_clipboard::ClipboardManager;

#[cfg(windows)]
pub use windows_clipboard::ClipboardManager;
