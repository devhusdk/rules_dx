//! Product core for the packaged Rust Android application.

use std::sync::atomic::{AtomicI32, Ordering};

const ASSET_TEXT: &str = "hello from the packaged asset";

static LIFECYCLE: AtomicI32 = AtomicI32::new(Lifecycle::Created as i32);
static TOUCHES: AtomicI32 = AtomicI32::new(0);
static KEYS: AtomicI32 = AtomicI32::new(0);

/// Lifecycle states the activity reports to the product core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Lifecycle {
    Created = 0,
    Started = 1,
    Resumed = 2,
    Paused = 3,
    Stopped = 4,
    Destroyed = 5,
}

impl Lifecycle {
    fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Created),
            1 => Some(Self::Started),
            2 => Some(Self::Resumed),
            3 => Some(Self::Paused),
            4 => Some(Self::Stopped),
            5 => Some(Self::Destroyed),
            _ => None,
        }
    }

    fn allows(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Started)
                | (Self::Started, Self::Resumed)
                | (Self::Started, Self::Stopped)
                | (Self::Resumed, Self::Paused)
                | (Self::Paused, Self::Resumed)
                | (Self::Paused, Self::Stopped)
                | (Self::Stopped, Self::Started)
                | (Self::Stopped, Self::Destroyed)
                | (Self::Created, Self::Destroyed)
        )
    }
}

/// Status reported to the NativeActivity entry point after startup.
#[no_mangle]
pub extern "C" fn app_main() -> i32 {
    LIFECYCLE.store(Lifecycle::Created as i32, Ordering::SeqCst);
    TOUCHES.store(0, Ordering::SeqCst);
    KEYS.store(0, Ordering::SeqCst);
    42
}

/// Records a lifecycle change and returns the new state, or -1 when the change is invalid.
#[no_mangle]
pub extern "C" fn app_lifecycle(event: i32) -> i32 {
    let Some(next) = Lifecycle::from_code(event) else {
        return -1;
    };
    let current =
        Lifecycle::from_code(LIFECYCLE.load(Ordering::SeqCst)).unwrap_or(Lifecycle::Created);
    if !current.allows(next) {
        return -1;
    }
    LIFECYCLE.store(next as i32, Ordering::SeqCst);
    next as i32
}

/// Counts one touch event and returns the running total.
#[no_mangle]
pub extern "C" fn app_touch() -> i32 {
    TOUCHES.fetch_add(1, Ordering::SeqCst) + 1
}

/// Counts one key event and returns the running total.
#[no_mangle]
pub extern "C" fn app_key() -> i32 {
    KEYS.fetch_add(1, Ordering::SeqCst) + 1
}

/// Returns 0 when the bytes match the packaged asset and 1 otherwise.
///
/// # Safety
///
/// `data` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn app_asset_check(data: *const u8, len: usize) -> i32 {
    if data.is_null() {
        return 1;
    }
    let bytes = std::slice::from_raw_parts(data, len);
    i32::from(bytes.trim_ascii_end() != ASSET_TEXT.as_bytes())
}

/// Asset path the application loads at startup.
pub fn asset_name() -> &'static str {
    "message.txt"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn startup_reports_ready_status() {
        let _guard = SERIAL.lock().unwrap();
        assert_eq!(app_main(), 42);
    }

    #[test]
    fn startup_asset_name_matches_packaged_asset() {
        assert_eq!(asset_name(), "message.txt");
    }

    #[test]
    fn lifecycle_follows_the_activity_order() {
        let _guard = SERIAL.lock().unwrap();
        app_main();
        assert_eq!(app_lifecycle(1), 1);
        assert_eq!(app_lifecycle(2), 2);
        assert_eq!(app_lifecycle(3), 3);
        assert_eq!(app_lifecycle(2), 2);
        assert_eq!(app_lifecycle(3), 3);
        assert_eq!(app_lifecycle(4), 4);
        assert_eq!(app_lifecycle(1), 1);
        assert_eq!(app_lifecycle(4), 4);
        assert_eq!(app_lifecycle(5), 5);
    }

    #[test]
    fn lifecycle_rejects_out_of_order_and_unknown_events() {
        let _guard = SERIAL.lock().unwrap();
        app_main();
        assert_eq!(app_lifecycle(2), -1);
        assert_eq!(app_lifecycle(9), -1);
        assert_eq!(app_lifecycle(-1), -1);
        assert_eq!(app_lifecycle(1), 1);
    }

    #[test]
    fn input_events_are_counted_per_kind() {
        let _guard = SERIAL.lock().unwrap();
        app_main();
        assert_eq!(app_touch(), 1);
        assert_eq!(app_touch(), 2);
        assert_eq!(app_key(), 1);
        app_main();
        assert_eq!(app_touch(), 1);
    }

    #[test]
    fn asset_check_accepts_only_the_packaged_text() {
        let good = b"hello from the packaged asset\n";
        let bad = b"something else";
        assert_eq!(unsafe { app_asset_check(good.as_ptr(), good.len()) }, 0);
        assert_eq!(unsafe { app_asset_check(bad.as_ptr(), bad.len()) }, 1);
        assert_eq!(unsafe { app_asset_check(std::ptr::null(), 0) }, 1);
    }
}
