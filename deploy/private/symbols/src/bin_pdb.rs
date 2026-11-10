//! Checks a Windows PDB against its PE file and writes the pdb manifest.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;

struct Args {
    input: PathBuf,
    pdb: PathBuf,
    manifest: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let names = ["--in", "--pdb", "--manifest"];
    let mut found: [Option<PathBuf>; 3] = Default::default();
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
        input: take(0)?,
        pdb: take(1)?,
        manifest: take(2)?,
    })
}

fn write(a: &Args) -> Result<(), String> {
    let manifest = dx_symbols::pdb_manifest(&a.input, &a.pdb)?;
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&a.manifest, text + "\n").map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = parse(&args).and_then(|a| write(&a));
    if let Err(e) = result {
        eprintln!("pdb: {e}");
        std::process::exit(1);
    }
}
