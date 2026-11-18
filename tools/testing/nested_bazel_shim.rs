//! The native `bazel` in each nested Bazel slot: runs the pinned Bazel with the slot's startup options.

fn main() -> std::process::ExitCode {
    u8::try_from(dx_nested::shim_main()).map_or(
        std::process::ExitCode::FAILURE,
        std::process::ExitCode::from,
    )
}
