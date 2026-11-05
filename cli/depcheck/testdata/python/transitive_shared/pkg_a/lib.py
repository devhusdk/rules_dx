"""Package A uses the shared helper, not pytest directly."""

from pkg_b import label


def run():
    return label()
