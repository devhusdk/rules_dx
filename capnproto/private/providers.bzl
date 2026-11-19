"""Providers of the Cap'n Proto rules."""

CapnprotoInfo = provider(
    doc = "Cap'n Proto schemas of one library and its transitive dependencies.",
    fields = {
        "label": "string: label of the capnproto_library that owns direct_srcs.",
        "direct_deps": "List[CapnprotoInfo]: the capnproto_library targets named in deps, before transitive closure.",
        "direct_srcs": "List[File]: schemas owned by this library.",
        "direct_staged": "List[File]: the staged copy of each of direct_srcs, in the same order.",
        "stage_root": "string: exec path of the import root direct_staged lives under.",
        "stage_roots": "depset[string]: import roots of this library, the bundled schemas and every dependency.",
        "staged": "depset[File]: staged schemas under stage_roots.",
        "transitive_srcs": "depset[File]: schemas of this library and every dependency.",
        "transitive_imports": "depset[struct(path, file, staged)]: every schema an import can reach, with its path from the import root and its staged copy.",
    },
)

CapnpStdInfo = provider(
    doc = "The bundled /capnp/*.capnp schemas, staged once.",
    fields = {
        "imports": "List[struct(path, file, staged)]: each bundled schema with its import path and staged copy.",
        "stage_root": "string: exec path of the import root the schemas are staged under.",
    },
)
