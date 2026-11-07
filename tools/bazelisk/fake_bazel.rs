//! Stands in for Bazel in wrapper_test: prints BAZEL_SH and each argument on its own line.

fn main() {
    println!("BAZEL_SH={}", std::env::var("BAZEL_SH").unwrap_or_default());
    for arg in std::env::args().skip(1) {
        println!("arg={arg}");
    }
    let code = std::env::var("FAKE_BAZEL_EXIT").map_or(0, |code| code.parse().unwrap());
    std::process::exit(code);
}
