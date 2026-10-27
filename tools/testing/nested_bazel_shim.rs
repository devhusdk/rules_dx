//! The native `bazel` in each nested Bazel slot: runs the pinned Bazel with the slot's startup options.

fn main() {
    std::process::exit(dx_testing::nested::shim_main());
}
