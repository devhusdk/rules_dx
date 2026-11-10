"""Providers of the Cap'n Proto rules."""

CapnpInfo = provider(
    doc = "Cap'n Proto schemas of one library and its transitive dependencies.",
    fields = {
        "label": "string: label of the capnp_library that owns direct_srcs.",
        "direct_deps": "List[CapnpInfo]: the capnp_library targets named in deps, before transitive closure.",
        "direct_srcs": "List[File]: schemas owned by this library.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
        "transitive_imports": "depset[struct(path, file)]: every schema an import can reach, with its path from the import root.",
    },
)
