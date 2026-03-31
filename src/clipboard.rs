#[cfg(unix)]
mod unix_clipboard {
    use anyhow::Result;

    pub struct ClipboardManager;

    impl ClipboardManager {
        pub fn new() -> Result<Self> {
            Ok(ClipboardManager)
        }

        pub fn copy(&self, _text: &str) -> Result<()> {
            // X11 clipboard support placeholder
            Ok(())
        }

        pub fn paste(&self) -> Result<String> {
            // X11 clipboard support placeholder
            Ok(String::new())
        }
    }
}

#[cfg(windows)]
mod windows_clipboard {
    use anyhow::{anyhow, Result};

    pub struct ClipboardManager;

    impl ClipboardManager {
        pub fn new() -> Result<Self> {
            Ok(ClipboardManager)
        }

        pub fn copy(&self, _text: &str) -> Result<()> {
            // Windows clipboard support placeholder
            Ok(())
        }

        pub fn paste(&self) -> Result<String> {
            Ok(String::new())
        }
    }
}

#[cfg(unix)]
pub use unix_clipboard::ClipboardManager;

#[cfg(windows)]
pub use windows_clipboard::ClipboardManager;

pub trait Clipboard {
    fn copy(&self, text: &str) -> anyhow::Result<()>;
    fn paste(&self) -> anyhow::Result<String>;
}

impl Clipboard for ClipboardManager {
    fn copy(&self, text: &str) -> anyhow::Result<()> {
        ClipboardManager::copy(self, text)
    }

    fn paste(&self) -> anyhow::Result<String> {
        ClipboardManager::paste(self)
    }
}

