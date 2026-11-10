"""Quality taxonomy: every family with its wrapper owner and file classes."""

TAXONOMY = {
    "ansible": struct(
        owner = "//ansible:defs.bzl",
        classes = ["ansible"],
    ),
    "api_spec": struct(
        owner = "//api_spec:defs.bzl",
        classes = ["api_spec", "api_spec_ref"],
    ),
    "asciidoc": struct(
        owner = "//asciidoc/rules:defs.bzl",
        classes = ["asciidoc"],
    ),
    "astro": struct(
        owner = "//astro:defs.bzl",
        classes = ["astro"],
    ),
    "avro": struct(
        owner = "//avro:defs.bzl",
        classes = ["avro"],
    ),
    "bicep": struct(
        owner = "//bicep:defs.bzl",
        classes = ["bicep"],
    ),
    "capnp": struct(
        owner = "//capnproto:defs.bzl",
        classes = ["capnp"],
    ),
    "cc": struct(
        owner = "//cc/rules:defs.bzl",
        classes = ["c", "cpp"],
    ),
    "cloudformation": struct(
        owner = "//cloudformation:defs.bzl",
        classes = ["cloudformation"],
    ),
    "cmake": struct(
        owner = "//cmake/rules:defs.bzl",
        classes = ["cmake"],
    ),
    "core": struct(
        owner = "other",
        classes = ["github_workflow"],
    ),
    "csharp": struct(
        owner = "@rules_dx_dotnet//csharp/rules:defs.bzl",
        classes = ["csharp"],
    ),
    "css": struct(
        owner = "//javascript:defs.bzl",
        classes = ["css", "less", "scss"],
    ),
    "cuda": struct(
        owner = "@rules_dx_cuda//rules:defs.bzl",
        classes = ["cuda"],
    ),
    "cue": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["cue"],
    ),
    "d": struct(
        owner = "//d:defs.bzl",
        classes = ["d"],
    ),
    "dart": struct(
        owner = "//dart/rules:defs.bzl",
        classes = ["dart"],
    ),
    "dockerfile": struct(
        owner = "//dockerfile:defs.bzl",
        classes = ["dockerfile"],
    ),
    "elixir": struct(
        owner = "//elixir:defs.bzl",
        classes = ["elixir"],
    ),
    "erlang": struct(
        owner = "//erlang:defs.bzl",
        classes = ["erlang"],
    ),
    "flatbuffers": struct(
        owner = "//flatbuffers:defs.bzl",
        classes = ["flatbuffers"],
    ),
    "fortran": struct(
        owner = "//fortran:defs.bzl",
        classes = ["fortran"],
    ),
    "fsharp": struct(
        owner = "@rules_dx_dotnet//fsharp/rules:defs.bzl",
        classes = ["fsharp"],
    ),
    "gherkin": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["gherkin"],
    ),
    "go": struct(
        owner = "//go/rules:defs.bzl",
        classes = ["go"],
    ),
    "go_module": struct(
        owner = "other",
        classes = ["go_module"],
    ),
    "graphql": struct(
        owner = "//javascript:defs.bzl",
        classes = ["graphql"],
    ),
    "haskell": struct(
        owner = "//haskell/rules:defs.bzl",
        classes = ["haskell"],
    ),
    "helm": struct(
        owner = "//helm:defs.bzl",
        classes = ["helm"],
    ),
    "html": struct(
        owner = "//javascript:defs.bzl",
        classes = ["html"],
    ),
    "html_template": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["html_template"],
    ),
    "java": struct(
        owner = "//java:defs.bzl",
        classes = ["java"],
    ),
    "javascript": struct(
        owner = "//javascript:defs.bzl",
        classes = ["javascript", "jsx"],
    ),
    "json": struct(
        owner = "other",
        classes = ["json", "json5", "jsonc"],
    ),
    "jsonnet": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["jsonnet"],
    ),
    "julia": struct(
        owner = "//julia:defs.bzl",
        classes = ["julia"],
    ),
    "jupyter": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["jupyter"],
    ),
    "kotlin": struct(
        owner = "//kotlin:defs.bzl",
        classes = ["kotlin"],
    ),
    "kubernetes": struct(
        owner = "//kubernetes:defs.bzl",
        classes = ["kubernetes"],
    ),
    "latex": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["latex"],
    ),
    "lua": struct(
        owner = "//lua:defs.bzl",
        classes = ["lua"],
    ),
    "make": struct(
        owner = "//make/rules:defs.bzl",
        classes = ["make"],
    ),
    "markdown": struct(
        owner = "other",
        classes = ["markdown"],
    ),
    "mdx": struct(
        owner = "//mdx:defs.bzl",
        classes = ["mdx"],
    ),
    "nix": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["nix"],
    ),
    "objc": struct(
        owner = "//cc/rules:defs.bzl",
        classes = ["objc", "objcpp"],
    ),
    "ocaml": struct(
        owner = "//ocaml/rules:defs.bzl",
        classes = ["ocaml"],
    ),
    "perl": struct(
        owner = "//perl:defs.bzl",
        classes = ["perl"],
    ),
    "php": struct(
        owner = "//php:defs.bzl",
        classes = ["php"],
    ),
    "pkl": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["pkl"],
    ),
    "powershell": struct(
        owner = "//powershell/rules:defs.bzl",
        classes = ["powershell"],
    ),
    "protobuf": struct(
        owner = "other",
        classes = ["protobuf"],
    ),
    "python": struct(
        owner = "//python:defs.bzl",
        classes = ["python", "python_stub"],
    ),
    "qml": struct(
        owner = "//qt/rules:defs.bzl",
        classes = ["qml"],
    ),
    "rego": struct(
        owner = "//rego:defs.bzl",
        classes = ["rego"],
    ),
    "rst": struct(
        owner = "//rst/rules:defs.bzl",
        classes = ["rst"],
    ),
    "ruby": struct(
        owner = "//ruby:defs.bzl",
        classes = ["ruby"],
    ),
    "rust": struct(
        owner = "//rust/rules:defs.bzl",
        classes = ["rust"],
    ),
    "scala": struct(
        owner = "//scala:defs.bzl",
        classes = ["scala"],
    ),
    "shell": struct(
        owner = "other",
        classes = ["shell"],
    ),
    "sql": struct(
        owner = "//sql/rules:defs.bzl",
        classes = ["sql"],
    ),
    "starlark": struct(
        owner = "other",
        classes = ["starlark"],
    ),
    "svelte": struct(
        owner = "//svelte:defs.bzl",
        classes = ["svelte"],
    ),
    "swift": struct(
        owner = "//swift/rules:defs.bzl",
        classes = ["swift"],
    ),
    "terraform": struct(
        owner = "//terraform:defs.bzl",
        classes = ["terraform"],
    ),
    "text": struct(
        owner = "other",
        classes = ["text"],
    ),
    "thrift": struct(
        owner = "//thrift:defs.bzl",
        classes = ["thrift"],
    ),
    "toml": struct(
        owner = "other",
        classes = ["toml"],
    ),
    "typescript": struct(
        owner = "//typescript:defs.bzl",
        classes = ["tsx", "typescript"],
    ),
    "typst": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["typst"],
    ),
    "vue": struct(
        owner = "//vue:defs.bzl",
        classes = ["vue"],
    ),
    "xml": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["xml"],
    ),
    "yaml": struct(
        owner = "//quality/rules:defs.bzl",
        classes = ["yaml"],
    ),
    "zig": struct(
        owner = "//zig:defs.bzl",
        classes = ["zig"],
    ),
}

def taxonomy_schema_error(taxonomy = TAXONOMY):
    """Returns an error string for a malformed taxonomy, else the empty string."""
    seen = {}
    for family in sorted(taxonomy.keys()):
        entry = taxonomy[family]
        if not entry.owner:
            return "taxonomy: family '" + family + "' has an empty owner"
        if not entry.classes:
            return "taxonomy: family '" + family + "' has no classes"
        for class_id in entry.classes:
            if class_id in seen:
                return "taxonomy: class '" + class_id + "' listed under both '" + seen[class_id] + "' and '" + family + "'"
            seen[class_id] = family
    return ""

TAXONOMY_CLASSES = sorted([class_id for entry in TAXONOMY.values() for class_id in entry.classes])

CLASS_TO_FAMILY = {
    class_id: family
    for family, entry in TAXONOMY.items()
    for class_id in entry.classes
}

FAMILY_OWNERS = {family: entry.owner for family, entry in TAXONOMY.items()}
