fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    for line in consumer_wasi::report(&argv) {
        println!("{line}");
    }
    if let Ok(code) = std::env::var("DX_WASI_EXIT") {
        std::process::exit(code.parse().unwrap_or(1));
    }
}
