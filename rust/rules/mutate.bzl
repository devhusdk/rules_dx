"""`rust_mutate` for `dx mutate`: runs cargo-mutants on the crate in this package."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

def rust_mutate(name, visibility = None, tags = None):
    """Declares a dx-mutate target and its `<name>_run` runner.

    The runner needs `cargo-mutants` on PATH (or CARGO_MUTANTS) and a Cargo
    manifest in this package.

    Args:
      name: target name; the runner is `<name>_run`.
      visibility: visibility of both targets.
      tags: extra tags.
    """
    native.filegroup(
        name = name,
        tags = (tags or []) + ["dx-mutate"],
        visibility = visibility,
    )
    sh_binary(
        name = name + "_run",
        srcs = [Label("//rust/rules:mutate_run.sh")],
        args = [native.package_name()],
        testonly = True,
        visibility = visibility,
    )
