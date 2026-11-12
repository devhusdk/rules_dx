fn main() {
    std::process::exit(dx_shim::run::main(std::env::args_os().skip(1).collect()));
}
