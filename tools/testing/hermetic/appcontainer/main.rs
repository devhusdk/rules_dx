//! Runs a command isolated on Windows: no network unless `--net`, files only where granted.
//! The AppContainer backend is the default; the user backend runs it as a throwaway local account.
mod args;

#[cfg(windows)]
mod sys;
#[cfg(windows)]
mod user;

#[cfg(windows)]
fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let outcome = args::parse(&argv).and_then(|config| match (config.mode, config.backend) {
        (args::Mode::SetupGroup, _) => user::setup_group(&config),
        (args::Mode::TeardownGroup, _) => user::teardown_group(&config),
        (args::Mode::Run, args::Backend::AppContainer) => sys::run(&config),
        (args::Mode::Run, args::Backend::User) => user::run(&config),
    });
    match outcome {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("appcontainer_run: {err}");
            std::process::exit(125);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("appcontainer_run only runs on Windows");
    std::process::exit(125);
}
