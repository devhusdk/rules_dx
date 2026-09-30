"""Quality source-ownership boundary."""

QualitySourcesInfo = provider(
    doc = "Directly owned repository sources keyed by semantic file class.",
    fields = {
        "direct_sources": (
            "Dict[str, depset[File]]: semantic file-class ID to directly " +
            "owned source artifacts. Empty depsets are omitted, never retained."
        ),
    },
)

SOURCES_REGISTRY_SCHEMA_VERSION = 1

KNOWN_SEMANTIC_FILE_CLASSES = [
    "text",
    "c",
    "cpp",
    "cuda",
    "csharp",
    "fsharp",
    "powershell",
    "css",
    "less",
    "scss",
    "javascript",
    "jsx",
    "typescript",
    "tsx",
    "vue",
    "svelte",
    "astro",
    "mdx",
    "graphql",
    "html",
    "html_template",
    "json",
    "json5",
    "jsonc",
    "markdown",
    "toml",
    "xml",
    "yaml",
    "java",
    "kotlin",
    "scala",
    "python",
    "python_stub",
    "cue",
    "gherkin",
    "go",
    "jsonnet",
    "pkl",
    "protobuf",
    "qml",
    "ruby",
    "rust",
    "shell",
    "sql",
    "starlark",
    "terraform",
    "go_module",
]

RUST = "rust"

def is_known_semantic_class(class_id):
    """Reports whether a class ID is in the versioned registry."""
    return class_id in KNOWN_SEMANTIC_FILE_CLASSES

def _is_canonical_id(text):
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def sources_schema_error(classes = None):
    """Validates the versioned class-registry schema."""
    ids = KNOWN_SEMANTIC_FILE_CLASSES if classes == None else classes
    if type(ids) != "list" or len(ids) == 0:
        return "sources registry: want a non-empty class list (schema v1)"
    seen = {}
    for class_id in ids:
        if type(class_id) != "string" or not _is_canonical_id(class_id):
            return "sources registry: non-canonical class ID '" + str(class_id) + "' (want [a-z0-9_])"
        if class_id in seen:
            return "sources registry: duplicate class ID '" + class_id + "'"
        seen[class_id] = True
    return ""

def check_direct_sources(direct_sources, what):
    """Validates provider-construction shape and known IDs."""
    if type(direct_sources) != "dict":
        fail("QualitySourcesInfo (" + what + "): direct_sources must be a " +
             "dict of class ID to depset, got " + type(direct_sources))
    for class_id in direct_sources.keys():
        if class_id not in KNOWN_SEMANTIC_FILE_CLASSES:
            fail("QualitySourcesInfo (" + what + "): unknown semantic " +
                 "file class '" + class_id + "'")
        sources = direct_sources[class_id]
        if type(sources) != "depset":
            fail("QualitySourcesInfo (" + what + "): class '" + class_id +
                 "' must map to a depset of Files, got " + type(sources))
        for f in sources.to_list():
            if type(f) != "File":
                fail("QualitySourcesInfo (" + what + "): class '" + class_id +
                     "' holds a non-File member: " + type(f))
