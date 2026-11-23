fn main() -> std::process::ExitCode {
    u8::try_from(dx_nested::shim_main()).map_or(
        std::process::ExitCode::FAILURE,
        std::process::ExitCode::from,
    )
}
