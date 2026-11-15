#![expect(
    unsafe_code,
    reason = "exports an unmangled C symbol for the native linker fixture"
)]

#[unsafe(no_mangle)]
pub extern "C" fn hello_cdylib() {}
