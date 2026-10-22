//! C interface that the Android activity and native callers link against.

use std::sync::{Mutex, MutexGuard};

use shared_tasks::{asset_report, reply, Board};

static BOARD: Mutex<Option<Board>> = Mutex::new(None);

fn board() -> MutexGuard<'static, Option<Board>> {
    BOARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

unsafe fn text<'a>(data: *const u8, len: usize) -> Option<&'a str> {
    let bytes = if data.is_null() {
        if len != 0 {
            return None;
        }
        &[][..]
    } else {
        std::slice::from_raw_parts(data, len)
    };
    std::str::from_utf8(bytes).ok()
}

unsafe fn copy_out(value: &str, out: *mut u8, cap: usize) -> usize {
    if !out.is_null() && cap > 0 {
        let count = value.len().min(cap - 1);
        std::ptr::copy_nonoverlapping(value.as_ptr(), out, count);
        *out.add(count) = 0;
    }
    value.len()
}

/// Loads a board and returns its task count, or -1 when the text is invalid.
///
/// # Safety
///
/// `data` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn tasks_load(data: *const u8, len: usize) -> i32 {
    let Some(loaded) = text(data, len).and_then(|text| Board::load(text).ok()) else {
        return -1;
    };
    let count = i32::try_from(loaded.tasks().len()).unwrap_or(i32::MAX);
    *board() = Some(loaded);
    count
}

/// Applies one command and writes its reply; returns 0, 1 when rejected, or -1 without a board.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to `cap` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn tasks_apply(data: *const u8, len: usize, out: *mut u8, cap: usize) -> i32 {
    let mut guard = board();
    let (Some(command), Some(current)) = (text(data, len), guard.as_mut()) else {
        copy_out("err:no_board", out, cap);
        return -1;
    };
    let answer = reply(current, command);
    copy_out(&answer, out, cap);
    i32::from(answer.starts_with("err:"))
}

/// Writes the board summary and returns its length, or 0 without a board.
///
/// # Safety
///
/// `out` must point to `cap` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn tasks_summary(out: *mut u8, cap: usize) -> usize {
    match board().as_ref() {
        Some(current) => copy_out(&current.summary().to_string(), out, cap),
        None => 0,
    }
}

/// Writes the asset report for one named payload and returns its length, or 0 for an invalid name.
///
/// # Safety
///
/// `name` and `data` must point to `name_len` and `len` readable bytes, `out` to `cap` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn tasks_asset_report(
    name: *const u8,
    name_len: usize,
    data: *const u8,
    len: usize,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(name) = text(name, name_len) else {
        return 0;
    };
    if data.is_null() && len != 0 {
        return 0;
    }
    let bytes = if data.is_null() {
        &[][..]
    } else {
        std::slice::from_raw_parts(data, len)
    };
    copy_out(&asset_report(name, bytes), out, cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> String {
        let mut buf = [0u8; 64];
        let len = unsafe { tasks_summary(buf.as_mut_ptr(), buf.len()) };
        String::from_utf8(buf[..len].to_vec()).expect("utf8 summary")
    }

    fn apply(command: &str) -> (i32, String) {
        let mut buf = [0u8; 64];
        let code =
            unsafe { tasks_apply(command.as_ptr(), command.len(), buf.as_mut_ptr(), buf.len()) };
        let end = buf.iter().position(|b| *b == 0).expect("nul terminated");
        (code, String::from_utf8(buf[..end].to_vec()).expect("utf8"))
    }

    #[test]
    fn c_interface_drives_the_board() {
        let seed = "[x] One\n[ ] Two @notes.txt\n";
        assert_eq!(unsafe { tasks_load(seed.as_ptr(), seed.len()) }, 2);
        assert_eq!(summary(), "tasks=2 done=1 open=1 assets=1");
        assert_eq!(apply("add Three"), (0, "ok:added 3".to_owned()));
        assert_eq!(apply("done 9"), (1, "err:no_such_task".to_owned()));
        assert_eq!(summary(), "tasks=3 done=1 open=2 assets=1");
        let bad = "nope";
        assert_eq!(unsafe { tasks_load(bad.as_ptr(), bad.len()) }, -1);
        assert_eq!(summary(), "tasks=3 done=1 open=2 assets=1");
        let mut small = [0u8; 4];
        let len = unsafe { tasks_summary(small.as_mut_ptr(), small.len()) };
        assert_eq!(len, "tasks=3 done=1 open=2 assets=1".len());
        assert_eq!(&small, b"tas\0");
        let name = "a.txt";
        let data = b"Wikipedia";
        let mut out = [0u8; 64];
        let len = unsafe {
            tasks_asset_report(
                name.as_ptr(),
                name.len(),
                data.as_ptr(),
                data.len(),
                out.as_mut_ptr(),
                out.len(),
            )
        };
        assert_eq!(&out[..len], b"asset a.txt bytes=9 adler32:11e60398");
    }
}
