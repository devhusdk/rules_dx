"""The built-in opam package annotations."""

BUILTIN_ANNOTATIONS = {
    "afl-persistent": struct(build_file = Label("//ocaml/private:afl-persistent.BUILD.bazel"), library = "", strip_prefix = "ocaml-afl-persistent-1.4"),
    "cmdliner": struct(build_file = Label("//ocaml/private:cmdliner.BUILD.bazel"), library = "", strip_prefix = ""),
    "crowbar": struct(build_file = Label("//ocaml/private:crowbar.BUILD.bazel"), library = "", strip_prefix = ""),
}

def pick_annotation(package, annotations):
    """Returns the consumer annotation for a package, else the built-in one, else None."""
    return annotations.get(package) or BUILTIN_ANNOTATIONS.get(package)
