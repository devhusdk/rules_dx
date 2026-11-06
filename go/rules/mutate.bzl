"""`go_mutate` for `dx mutate`."""

load("//libs/mutate:mutate.bzl", "dx_mutate")

def go_mutate(name, visibility = None, tags = None):
    """Declares a dx-mutate target that runs gremlins in this package.

    Args:
      name: target name; the runner is `<name>_run`.
      visibility: visibility of both targets.
      tags: extra tags.
    """
    dx_mutate(name, "gremlins", visibility, tags, Label("//libs/mutate:gremlins"))
