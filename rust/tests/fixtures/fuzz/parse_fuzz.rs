#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.starts_with(b"fuz") {
        panic!("planted crash");
    }
});
