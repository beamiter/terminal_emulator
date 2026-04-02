use std::sync::OnceLock;

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    *ENABLED.get_or_init(|| {
        std::env::var_os("TERMINAL_EMULATOR_DEBUG")
            .map(|value| value != "0")
            .unwrap_or(false)
    })
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if $crate::debug::enabled() {
            eprintln!($($arg)*);
        }
    };
}