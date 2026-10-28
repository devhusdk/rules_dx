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

QualityToolsInfo = provider(
    doc = "Quality tools a rule carries, keyed by tool ID.",
    fields = {"tools": "Dict[str, FilesToRunProvider]: tool ID to executable."},
)

SOURCES_REGISTRY_SCHEMA_VERSION = 1

KNOWN_SEMANTIC_FILE_CLASSES = [
    "text",
    "c",
    "cpp",
    "cuda",
    "objc",
    "objcpp",
    "csharp",
    "fsharp",
    "powershell",
    "css",
    "less",
    "scss",
    "dockerfile",
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
    "ansible",
    "api_spec",
    "java",
    "kotlin",
    "scala",
    "python",
    "python_stub",
    "jupyter",
    "cue",
    "d",
    "elixir",
    "erlang",
    "fortran",
    "gherkin",
    "go",
    "haskell",
    "jsonnet",
    "lua",
    "ocaml",
    "perl",
    "php",
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
    "zig",
    "nix",
    "kubernetes",
]

RUST = "rust"

UPSTREAM_RUST_EXTENSIONS = ["rs"]

PROTOBUF = "protobuf"

UPSTREAM_PROTOBUF_EXTENSIONS = ["proto"]

UPSTREAM_CC_CLASS_EXTENSIONS = {
    "c": ["c", "h"],
    "cpp": ["cc", "cpp", "cxx", "hh", "hpp", "hxx"],
    "cuda": ["cu", "cuh"],
    "objc": ["m"],
    "objcpp": ["mm"],
}

def upstream_source_class(extension):
    """Returns the semantic file class for one upstream native extension, or ""."""
    if extension in UPSTREAM_RUST_EXTENSIONS:
        return RUST
    if extension in UPSTREAM_PROTOBUF_EXTENSIONS:
        return PROTOBUF
    for class_id in sorted(UPSTREAM_CC_CLASS_EXTENSIONS.keys()):
        if extension in UPSTREAM_CC_CLASS_EXTENSIONS[class_id]:
            return class_id
    return ""

DOCKERFILE_NAME_PATTERNS = [
    "Dockerfile",
    "Containerfile",
    "Dockerfile.*",
    "Containerfile.*",
    "*.Dockerfile",
    "*.dockerfile",
    "*.Containerfile",
    "*.containerfile",
]

_DOCKERFILE_STEMS = ["Dockerfile", "Containerfile"]

_DOCKERFILE_SUFFIXES = [".Dockerfile", ".dockerfile", ".Containerfile", ".containerfile"]

def is_dockerfile_name(basename):
    """Reports whether one file basename names a Dockerfile; `*.dockerignore` never does."""
    if basename.endswith(".dockerignore"):
        return False
    for stem in _DOCKERFILE_STEMS:
        if basename == stem:
            return True
        if basename.startswith(stem + ".") and len(basename) > len(stem) + 1:
            return True
    for suffix in _DOCKERFILE_SUFFIXES:
        if basename.endswith(suffix) and len(basename) > len(suffix):
            return True
    return False

def dockerfile_name_error(basename):
    """Returns the error for a basename the Dockerfile matcher rejects, or ""."""
    if is_dockerfile_name(basename):
        return ""
    return "'" + basename + "' is not a Dockerfile name; accepted names: " + ", ".join(DOCKERFILE_NAME_PATTERNS)

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
