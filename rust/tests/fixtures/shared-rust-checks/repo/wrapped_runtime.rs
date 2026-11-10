fn main() {
    let mut args = std::env::args_os().skip(1);
    let subcommand = args.next().expect("usage: wrapped_runtime SUBCOMMAND [ARG]...");
    let managed = std::env::var_os("DX_WASI_MANAGED_RUNTIME").expect("DX_WASI_MANAGED_RUNTIME");
    let status = std::process::Command::new(&managed)
        .arg(subcommand)
        .args(["--env", "CONSUMER_RUNTIME=wrapped"])
        .args(args)
        .status()
        .unwrap_or_else(|error| panic!("{}: {error}", managed.display()));
    std::process::exit(status.code().unwrap_or(1));
}
