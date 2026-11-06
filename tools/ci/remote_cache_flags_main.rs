//! `remote_cache_flags_bin` prints the Bazel flags for the remote cache, one per line.

use remote_cache_flags::{Cache, flags};
use std::process::ExitCode;

fn main() -> ExitCode {
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    let cache = Cache {
        url: &var("REMOTE_CACHE"),
        header: &var("REMOTE_CACHE_HEADER"),
        buildbuddy_key: &var("BUILDBUDDY_API_KEY"),
        event: &var("GITHUB_EVENT_NAME"),
    };
    match flags(&cache) {
        Ok(list) => {
            for flag in list {
                println!("{flag}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("dx-ci: {message}");
            ExitCode::FAILURE
        }
    }
}
