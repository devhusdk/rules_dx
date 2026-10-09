fn env_or_unset(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| "<unset>".to_owned())
}

fn read_or_no_access(path: &str) -> String {
    std::fs::read_to_string(path)
        .map(|content| content.trim_end_matches(['\r', '\n']).to_owned())
        .unwrap_or_else(|_| "no-access".to_owned())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    println!("argv={argv:?}");
    println!("declared={}", env_or_unset("DX_WASI_DECLARED"));
    println!("wrapped={}", env_or_unset("DX_WASI_WRAPPED"));
    println!(
        "hostile={}",
        if std::env::var("DX_WASI_HOSTILE").is_ok() {
            "leaked"
        } else {
            "hidden"
        }
    );
    println!("data={}", read_or_no_access("/data/input.txt"));
    let probe = std::env::var("DX_WASI_PROBE").unwrap_or_else(|_| "/denied/secret.txt".to_owned());
    println!("probe={}", read_or_no_access(&probe));
    if let Ok(code) = std::env::var("DX_WASI_EXIT") {
        std::process::exit(code.parse().unwrap_or(1));
    }
}
