//! Checks a Windows PDB against its PE file and writes the pdb manifest.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use dx_symbols::cli::{CliError, parse_flags, write_manifest};

fn run(args: &[String]) -> Result<(), CliError> {
    let [input, pdb, manifest] = parse_flags(args, ["--in", "--pdb", "--manifest"])?;
    write_manifest(&manifest, &dx_symbols::pdb_manifest(&input, &pdb)?)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("pdb: {e}");
        std::process::exit(1);
    }
}
