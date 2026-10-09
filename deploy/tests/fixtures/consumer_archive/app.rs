use std::path::PathBuf;

unsafe extern "C" {
    fn consumer_greet_create(name: *const std::os::raw::c_char) -> *mut std::ffi::c_void;
    fn consumer_greet_message(state: *const std::ffi::c_void) -> *const std::os::raw::c_char;
    fn consumer_greet_destroy(state: *mut std::ffi::c_void);
}

fn banner_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| format!("cannot locate exe: {error}"))?;
    exe.parent()
        .map(|dir| dir.join("resources").join("banner.txt"))
        .ok_or_else(|| "exe has no parent directory".to_owned())
}

fn run() -> i32 {
    let banner = match banner_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("consumer_app: {error}");
            return 1;
        }
    };
    let text = match std::fs::read_to_string(&banner) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("consumer_app: cannot read {}: {error}", banner.display());
            return 1;
        }
    };
    let name = match std::ffi::CString::new("consumer") {
        Ok(name) => name,
        Err(error) => {
            eprintln!("consumer_app: cannot encode name: {error}");
            return 1;
        }
    };
    let state = unsafe { consumer_greet_create(name.as_ptr()) };
    if state.is_null() {
        eprintln!("consumer_app: native create failed");
        return 1;
    }
    let message = unsafe { std::ffi::CStr::from_ptr(consumer_greet_message(state)) }
        .to_string_lossy()
        .into_owned();
    unsafe { consumer_greet_destroy(state) };
    for line in text.lines() {
        println!("consumer_app resource: {line}");
    }
    println!("consumer_app native: {message}");
    0
}

fn main() {
    std::process::exit(run());
}
