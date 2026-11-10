"""Providers the Swift wrappers export for documentation tools. No external loads."""

SwiftDocsInfo = provider(
    doc = "The symbol graphs and sources of one Swift library, for documentation tools.",
    fields = {
        "graphs": "List of structs (module_name, symbol_graph_dir): the directory of `<Module>.symbols.json` files swift-symbolgraph-extract wrote for each module the library defines.",
        "srcs": "List of the library's own .swift files.",
    },
)
