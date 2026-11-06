"""`ts_mutate` for `dx mutate`."""

load("//libs/mutate:mutate.bzl", "dx_mutate")

def ts_mutate(name, visibility = None, tags = None):
    """Declares a dx-mutate target that runs stryker in this package.

    Args:
      name: target name; the runner is `<name>_run`.
      visibility: visibility of both targets.
      tags: extra tags.
    """
    dx_mutate(name, "stryker", visibility, tags)
