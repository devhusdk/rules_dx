use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::consumer::write;
use dx_nested as nested;
use dx_testing::Run;

pub fn tool(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("{program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {args:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

pub fn outputs() -> Option<PathBuf> {
    let dir = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR").ok()?;
    let test = std::thread::current().name().unwrap_or("main").to_owned();
    Some(Path::new(&dir).join(test))
}

pub fn keep(name: &str, text: &str) {
    if let Some(dir) = outputs() {
        write(&dir.join(name), text);
    }
}

pub fn bazel(root: &Path, command: &str, targets: &[&str], flags: &[&str]) -> Run {
    let args: Vec<&str> = std::iter::once(command)
        .chain(flags.iter().copied())
        .chain(targets.iter().copied())
        .collect();
    let run = nested::bazel_in(root, &args);
    let name: String = [command]
        .iter()
        .chain(flags)
        .chain(targets)
        .flat_map(|part| part.chars().chain(['-']))
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    keep(&format!("bazel-{name}log.txt"), &run.combined());
    run
}

pub fn passes(run: &Run) {
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
}

pub fn passed(run: &Run, case: &str) -> bool {
    run.combined()
        .lines()
        .any(|line| line.contains(case) && line.contains("' passed"))
}

pub fn installed_xcode() -> String {
    let version = tool("xcodebuild", &["-version"]);
    let first = version.lines().next().unwrap_or_default();
    let number = first
        .strip_prefix("Xcode ")
        .unwrap_or_else(|| panic!("xcodebuild -version: {version}"));
    assert!(
        number.starts_with("16."),
        "the qualified cell runs Xcode 16; this executor selects {number}"
    );
    number.to_owned()
}

pub fn version(text: &str) -> Vec<u32> {
    text.split('.')
        .filter_map(|part| part.parse().ok())
        .collect()
}

pub fn runtime_for(sdk_version: &str) -> String {
    let listing = tool("xcrun", &["simctl", "list", "runtimes", "available"]);
    keep("runtimes.txt", &listing);
    let major = sdk_version.split('.').next().unwrap_or_default();
    let mut found: Vec<Vec<u32>> = listing
        .lines()
        .filter_map(|line| line.strip_prefix("iOS "))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter(|found| found.split('.').next() == Some(major))
        .map(version)
        .collect();
    found.sort();
    let wanted = version(sdk_version);
    let chosen = if found.contains(&wanted) {
        wanted
    } else {
        found
            .pop()
            .unwrap_or_else(|| panic!("no iOS {major} simulator runtime in:\n{listing}"))
    };
    chosen
        .iter()
        .take(2)
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

pub struct Simulator {
    pub name: String,
    pub udid: String,
}

impl Simulator {
    pub fn create(prefix: &str, runtime: &str) -> Simulator {
        let name = format!("{prefix}-{}", std::process::id());
        let id = format!(
            "com.apple.CoreSimulator.SimRuntime.iOS-{}",
            runtime.replace('.', "-")
        );
        let udid = tool("xcrun", &["simctl", "create", &name, "iPhone 16", &id]);
        Simulator { name, udid }
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        let _ = Command::new("xcrun")
            .args(["simctl", "shutdown", &self.udid])
            .output();
        let _ = Command::new("xcrun")
            .args(["simctl", "delete", &self.udid])
            .output();
    }
}

pub fn wait_for(path: &Path) -> String {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.is_empty() {
                return text;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    panic!("the launched app wrote no {path:?}");
}
