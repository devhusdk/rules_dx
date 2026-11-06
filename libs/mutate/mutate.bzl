"""Shared `dx mutate` target for the language wrappers."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

def dx_mutate(name, kind, visibility = None, tags = None):
    """Declares a dx-mutate target and its `<name>_run` runner.

    Args:
      name: target name; the runner is `<name>_run`.
      kind: mutation tool: mutmut, gremlins, stryker or pitest.
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
        srcs = [Label("//libs/mutate:run.sh")],
        args = [kind, native.package_name()],
        testonly = True,
        visibility = visibility,
    )
