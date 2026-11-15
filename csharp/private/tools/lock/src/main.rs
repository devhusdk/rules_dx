//! The lock step of the nuget ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nuget_lock::{Nuget, Paket, Registry};

fn runfile(runfiles: &Path, variable: &str) -> Option<PathBuf> {
    std::path::absolute(runfiles.join(std::env::var_os(variable)?))
        .ok()
        .filter(|path| path.is_file())
}

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("nuget") else {
        eprintln!("nuget has no registry entry");
        return ExitCode::from(2);
    };
    let runfiles = std::env::var_os("RUNFILES_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let mut name = std::env::current_exe().ok()?.into_os_string();
            name.push(".runfiles");
            Some(name.into())
        });
    let paket = runfiles.as_deref().and_then(|runfiles| {
        Some(Paket {
            dotnet: runfile(runfiles, "DOTNET")?,
            nupkg: runfile(runfiles, "PAKET")?,
        })
    });
    let Some(paket) = paket else {
        eprintln!("the dotnet host and the paket package are not in the runfiles");
        return ExitCode::from(2);
    };
    let spec = Nuget::spec(hub);
    dx_lock::main(&spec, &Nuget::new(&spec, &Registry::new()).with_paket(paket))
}
