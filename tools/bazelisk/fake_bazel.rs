//! Stands in for Bazel in wrapper_test: prints BAZEL_SH and each argument on its own line.

fn main() {
    println!("BAZEL_SH={}", std::env::var("BAZEL_SH").unwrap_or_default());
    for arg in std::env::args().skip(1) {
        println!("arg={arg}");
    }
    let code = match std::env::var("FAKE_BAZEL_EXIT").map(|code| code.parse()) {
        Err(_) => 0,
        Ok(Ok(code)) => code,
        Ok(Err(error)) => {
            eprintln!("fake_bazel: FAKE_BAZEL_EXIT is not an exit code: {error}");
            2
        }
    };
    std::process::exit(code);
}
