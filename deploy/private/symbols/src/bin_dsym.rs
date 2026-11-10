//! Checks a macOS dSYM bundle against its binary and writes the dsym manifest.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Args {
    dsymutil: PathBuf,
    input: PathBuf,
    bundle: PathBuf,
    manifest: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let names = ["--dsymutil", "--in", "--bundle", "--manifest"];
    let mut found: [Option<PathBuf>; 4] = Default::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let slot = names
            .iter()
            .position(|n| n == flag)
            .ok_or_else(|| format!("unknown flag {flag}"))?;
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        found[slot] = Some(PathBuf::from(value));
    }
    let mut take = |i: usize| {
        found[i]
            .take()
            .ok_or_else(|| format!("{} is required", names[i]))
    };
    Ok(Args {
        dsymutil: take(0)?,
        input: take(1)?,
        bundle: take(2)?,
        manifest: take(3)?,
    })
}

const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn version(dsymutil: &Path) -> Result<String, String> {
    let mut command = Command::new(dsymutil);
    command.arg("--version");
    let out = dx_process::capture::run_captured(&mut command, TOOL_TIMEOUT)
        .map_err(|e| format!("cannot run {}: {e}", dsymutil.display()))?;
    if !out.success() {
        return Err(format!("{} --version failed", dsymutil.display()));
    }
    let text = out.stdout_text();
    Ok(text
        .lines()
        .find(|l| l.contains("version"))
        .unwrap_or("")
        .trim()
        .to_string())
}

fn write(a: &Args) -> Result<(), String> {
    let tool = version(&a.dsymutil)?;
    let manifest = dx_symbols::dsym_manifest(&a.input, &a.bundle, &tool)?;
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&a.manifest, text + "\n").map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = parse(&args).and_then(|a| write(&a));
    if let Err(e) = result {
        eprintln!("dsym: {e}");
        std::process::exit(1);
    }
}
