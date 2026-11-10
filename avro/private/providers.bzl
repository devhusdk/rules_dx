"""Providers of the Avro rules."""

AvroInfo = provider(
    doc = "Avro schemas of one library and its transitive dependencies.",
    fields = {
        "label": "Label: the avro_library target.",
        "direct_deps": "List[Label]: avro_library dependencies.",
        "direct_srcs": "List[File]: schemas owned by this library.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
        "import_root": "str: directory imports resolve from, the workspace root.",
    },
)
