"""The one way a public rules_dx symbol is retired.

A rename adds the new name and keeps the old one for one minor release,
calling `deprecated` from its body. Each call is recorded in
`api/deprecations.txt`, and `//tools/ci:deprecations_test` fails once the
module version reaches the removal version, so the old name cannot outlive it.
"""

def deprecated(what, remove_in):
    """Warns that `what` goes away in rules_dx `remove_in`.

    Pass both as string literals: the deprecations test reads them from source.

    Args:
      what: The public name being retired and what to use instead, for example
        `"dx_foo (use dx_bar)"`.
      remove_in: The `MAJOR.MINOR.PATCH` release that removes it.
    """
    print("WARNING: {} is deprecated and will be removed in rules_dx {}".format(what, remove_in))  # buildifier: disable=print
