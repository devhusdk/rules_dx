#![deny(clippy::indexing_slicing, clippy::string_slice)]
//! The test stand-in for host tools: records Bazel calls, prints its environment, sleeps, emits or exits.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let me = std::env::current_exe().expect("current_exe");
    if me.file_stem().is_some_and(|stem| stem == "real-bazel") {
        let log = me.with_extension("log");
        let line = format!(
            "{}|{}|{}\n",
            std::env::current_dir().expect("cwd").display(),
            std::env::var("TEST_TMPDIR").unwrap_or_else(|_| "unset".to_owned()),
            args.join(" ")
        );
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .and_then(|mut file| file.write_all(line.as_bytes()))
            .expect("record call");
        return;
    }
    match args.first().map(String::as_str) {
        None => {
            for (key, value) in std::env::vars() {
                println!("{key}={value}");
            }
        }
        Some("sleep") => std::thread::sleep(std::time::Duration::from_secs(
            args.get(1).and_then(|secs| secs.parse().ok()).unwrap_or(60),
        )),
        Some("emit") => {
            eprintln!("{}", args.get(1).map_or("", String::as_str));
            std::process::exit(args.get(2).and_then(|code| code.parse().ok()).unwrap_or(0));
        }
        Some("heartbeat") => {
            let file = args.get(1).expect("heartbeat file");
            loop {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file)
                    .and_then(|mut out| out.write_all(b"x\n"))
                    .expect("heartbeat");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        Some("exit") => std::process::exit(args.get(1).and_then(|code| code.parse().ok()).unwrap_or(0)),
        Some(other) => {
            eprintln!("nested probe: unknown mode {other}");
            std::process::exit(2);
        }
    }
}
