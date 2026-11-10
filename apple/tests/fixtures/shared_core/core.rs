use std::ffi::{c_char, CStr};

use shared_tasks::{reply, Board};

#[unsafe(no_mangle)]
pub extern "C" fn shared_core_open(board: *const c_char) -> i32 {
    if board.is_null() {
        return -1;
    }
    let Ok(text) = unsafe { CStr::from_ptr(board) }.to_str() else {
        return -1;
    };
    let Ok(mut loaded) = Board::load(text) else {
        return -1;
    };
    if !reply(&mut loaded, "add check").starts_with("ok:") {
        return -1;
    }
    i32::try_from(loaded.tasks().len()).unwrap_or(-1)
}
