"""Starter fixture composition."""

def starter_fixture(name, layers = []):
    """Declares a fixture: its own directory plus the shared `common/<layer>` directories."""
    native.filegroup(
        name = name,
        srcs = native.glob([name + "/**"]) + native.glob(["common/%s/**" % layer for layer in layers]),
        visibility = ["//visibility:public"],
    )
