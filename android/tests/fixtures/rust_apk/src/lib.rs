//! Product core for the packaged Rust Android application.

/// Status reported to the NativeActivity entry point after startup.
#[no_mangle]
pub extern "C" fn app_main() -> i32 {
    42
}

/// Asset path the application loads at startup.
pub fn asset_name() -> &'static str {
    "message.txt"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_reports_ready_status() {
        assert_eq!(app_main(), 42);
    }

    #[test]
    fn startup_asset_name_matches_packaged_asset() {
        assert_eq!(asset_name(), "message.txt");
    }
}
