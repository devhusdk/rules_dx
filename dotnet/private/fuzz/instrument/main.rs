//! Copies an assembly and runs SharpFuzz on the copy: `instrument <sharpfuzz> <input> <output>`.

use std::process::{Command, ExitCode};
use std::{env, fs};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let [sharpfuzz, input, output] = args.as_slice() else {
        eprintln!("usage: instrument <sharpfuzz> <input> <output>");
        return ExitCode::from(2);
    };
    if let Err(err) = fs::copy(input, output).and_then(|_| {
        let mut perms = fs::metadata(output)?.permissions();
        perms.set_readonly(false);
        fs::set_permissions(output, perms)
    }) {
        eprintln!("instrument: copying {input} to {output}: {err}");
        return ExitCode::from(1);
    }
    match Command::new(sharpfuzz).arg(output).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("instrument: sharpfuzz failed: {status}");
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("instrument: running {sharpfuzz}: {err}");
            ExitCode::from(1)
        }
    }
}
