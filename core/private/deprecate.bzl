"""The one way a public rules_dx symbol is retired."""

def deprecated(what, remove_in):
    """Warns that `what` goes away in rules_dx `remove_in`; pass both as string literals."""
    print("WARNING: {} is deprecated and will be removed in rules_dx {}".format(what, remove_in))  # buildifier: disable=print
