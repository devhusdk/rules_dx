"""Shared `dx mutate` target for the language wrappers."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

def dx_mutate(name, kind, visibility = None, tags = None, tool = None):
    """Declares a dx-mutate target and its `<name>_run` runner.

    Args:
      name: target name; the runner is `<name>_run`.
      kind: mutation tool: mutmut, gremlins, stryker or pitest.
      visibility: visibility of both targets.
      tags: extra tags.
      tool: prebuilt tool label, used when the tool is neither overridden nor on PATH.
    """
    native.filegroup(
        name = name,
        tags = (tags or []) + ["dx-mutate"],
        visibility = visibility,
    )
    sh_binary(
        name = name + "_run",
        srcs = [Label("//libs/mutate:run.sh")],
        args = [kind, native.package_name()] + (["$(rootpath %s)" % tool] if tool else []),
        data = [tool] if tool else [],
        testonly = True,
        visibility = visibility,
    )
