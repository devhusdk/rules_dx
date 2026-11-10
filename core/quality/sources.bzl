"""Quality source-ownership boundary."""

load(":taxonomy.bzl", "TAXONOMY_CLASSES")

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
    fields = {
        "tools": "Dict[str, FilesToRunProvider]: tool ID to executable.",
        "unavailable": "List[str]: tool IDs whose executable is an unavailable stub on this platform.",
    },
)


KNOWN_SEMANTIC_FILE_CLASSES = TAXONOMY_CLASSES

RUST = "rust"

UPSTREAM_RUST_EXTENSIONS = ["rs"]

PROTOBUF = "protobuf"

UPSTREAM_PROTOBUF_EXTENSIONS = ["proto"]

UPSTREAM_CC_CLASS_EXTENSIONS = {
    "c": ["c", "h"],
    "cpp": ["cc", "cpp", "cxx", "hh", "hpp", "hxx", "ipp", "inl", "tcc", "inc", "h++", "c++", "C", "H"],
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

# Classes whose files are recognised by exact basename or suffix. Matching is
# case-sensitive. Add a row here to onboard a new name-matched class.
NAMED_SOURCE_CLASSES = {
    "make": {
        "names": ["GNUmakefile", "Makefile", "makefile"],
        "suffixes": [".mk"],
    },
    "cmake": {
        "names": ["CMakeLists.txt"],
        "suffixes": [".cmake"],
    },
}

def is_named_source(class_id, basename):
    """Reports whether a basename belongs to a name-matched class (case-sensitive)."""
    row = NAMED_SOURCE_CLASSES.get(class_id)
    if row == None:
        return False
    if basename in row["names"]:
        return True
    for suffix in row["suffixes"]:
        if basename.endswith(suffix) and len(basename) > len(suffix):
            return True
    return False

def named_source_class_error(class_id, basename):
    """Returns the error for a basename the class matcher rejects, or ""."""
    if is_named_source(class_id, basename):
        return ""
    row = NAMED_SOURCE_CLASSES[class_id]
    accepted = row["names"] + ["*" + s for s in row["suffixes"]]
    return "'" + basename + "' is not a " + class_id + " file name; accepted names: " + ", ".join(accepted)

def named_source_library(class_id, doc, srcs_doc):
    """Returns a `<class_id>_library` rule that owns name-matched sources of one class."""
    name = class_id + "_library"

    def _impl(ctx):
        for f in ctx.files.srcs:
            error = named_source_class_error(class_id, f.basename)
            if error != "":
                fail(name + " " + str(ctx.label) + ": " + error)
        direct_sources = {}
        if len(ctx.files.srcs) > 0:
            direct_sources[class_id] = depset(ctx.files.srcs)
        check_direct_sources(direct_sources, str(ctx.label))
        return [
            DefaultInfo(files = depset(
                ctx.files.srcs,
                transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
            )),
            QualitySourcesInfo(direct_sources = direct_sources),
        ]

    return rule(
        implementation = _impl,
        doc = doc,
        attrs = {
            "srcs": attr.label_list(allow_files = True, doc = srcs_doc),
            "deps": attr.label_list(providers = [DefaultInfo], doc = "Other " + class_id + " libraries this package depends on."),
        },
    )

# Classes recognised by directory and extension. These are checked before the
# extension table, so a workflow file leaves the `yaml` class for lint.
PATH_SOURCE_CLASSES = {
    "github_workflow": {
        "dirs": [".github/workflows"],
        "extensions": [".yml", ".yaml"],
    },
}

def path_source_class(path):
    """Returns the path-matched class for a workspace-relative path, or ""."""
    for class_id in sorted(PATH_SOURCE_CLASSES.keys()):
        row = PATH_SOURCE_CLASSES[class_id]
        for d in row["dirs"]:
            if not path.startswith(d + "/"):
                continue
            for ext in row["extensions"]:
                if path.endswith(ext) and len(path) > len(d) + 1 + len(ext):
                    return class_id
    return ""

def is_known_semantic_class(class_id):
    """Reports whether a class ID is in the registry."""
    return class_id in KNOWN_SEMANTIC_FILE_CLASSES

def _is_canonical_id(text):
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def sources_schema_error(classes = None):
    """Validates the class-registry schema."""
    ids = KNOWN_SEMANTIC_FILE_CLASSES if classes == None else classes
    if type(ids) != "list" or len(ids) == 0:
        return "sources registry: want a non-empty class list"
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
