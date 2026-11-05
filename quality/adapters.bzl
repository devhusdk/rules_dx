"""Deterministic synthetic adapter registry."""

load(":tools.bzl", "TOOLS")

SYNTHETIC_ADAPTERS = {
    "fmt-a": {"format": ["rust"]},
    "lint-a": {"lint": ["python", "rust"]},
    "lint-b": {"lint": ["rust"]},
}

SYNTHETIC_CLASS_TO_FAMILY = {
    "python": "python",
    "rust": "rust",
}

def adapter_supported_classes(tool_id, capability):
    """Returns the sorted supported classes for one synthetic tool/capability."""
    if tool_id not in SYNTHETIC_ADAPTERS:
        fail("adapters: unknown tool '" + tool_id +
             "': not in the synthetic adapter registry")
    return sorted(SYNTHETIC_ADAPTERS[tool_id].get(capability, []))

REAL_ADAPTERS = {tool.id: tool.adapters for tool in TOOLS}

REAL_CLASS_TO_FAMILY = {
    "ansible": "ansible",
    "api_spec": "api_spec",
    "api_spec_ref": "api_spec",
    "astro": "astro",
    "bicep": "bicep",
    "c": "cc",
    "cpp": "cc",
    "csharp": "csharp",
    "cloudformation": "cloudformation",
    "css": "css",
    "cuda": "cuda",
    "cue": "cue",
    "dockerfile": "dockerfile",
    "make": "make",
    "capnp": "capnp",
    "avro": "avro",
    "cmake": "cmake",
    "d": "d",
    "dart": "dart",
    "elixir": "elixir",
    "erlang": "erlang",
    "flatbuffers": "flatbuffers",
    "fortran": "fortran",
    "fsharp": "fsharp",
    "gherkin": "gherkin",
    "github_workflow": "core",
    "go": "go",
    "go_module": "go_module",
    "graphql": "graphql",
    "haskell": "haskell",
    "html": "html",
    "html_template": "html_template",
    "java": "java",
    "javascript": "javascript",
    "json": "json",
    "json5": "json",
    "jsonc": "json",
    "jsx": "javascript",
    "jupyter": "jupyter",
    "kotlin": "kotlin",
    "helm": "helm",
    "kubernetes": "kubernetes",
    "less": "css",
    "lua": "lua",
    "nix": "nix",
    "markdown": "markdown",
    "mdx": "mdx",
    "objc": "objc",
    "objcpp": "objc",
    "ocaml": "ocaml",
    "perl": "perl",
    "php": "php",
    "pkl": "pkl",
    "protobuf": "protobuf",
    "powershell": "powershell",
    "python": "python",
    "python_stub": "python",
    "qml": "qml",
    "rego": "rego",
    "ruby": "ruby",
    "rust": "rust",
    "scala": "scala",
    "scss": "css",
    "shell": "shell",
    "sql": "sql",
    "starlark": "starlark",
    "svelte": "svelte",
    "terraform": "terraform",
    "text": "text",
    "thrift": "thrift",
    "toml": "toml",
    "typescript": "typescript",
    "tsx": "typescript",
    "typst": "typst",
    "latex": "latex",
    "rst": "rst",
    "asciidoc": "asciidoc",
    "vue": "vue",
    "jsonnet": "jsonnet",
    "julia": "julia",
    "xml": "xml",
    "yaml": "yaml",
    "zig": "zig",
    "swift": "swift",
}

# Capabilities a family can never have: no static type system (or no
# maintained checker) exists, so the support matrix shows `n/a`, not a gap.
# Keyed by capability, then family. "n/a" means never; "not yet" belongs in
# the gap ledger. A family with a real adapter must not be listed here.
NOT_APPLICABLE = {
    "lint": {
        "cue": "cue vet is the only CUE analysis and it is already the typecheck capability; the pinned cue binary has no separate linter",
    },
    "format": {
        "capnp": "schema IDL; no maintained .capnp formatter exists, capnp only validates",
        "flatbuffers": "schema IDL; no maintained .fbs formatter exists, flatc only validates",
        "avro": "schema files; avro-tools has no formatter, it only validates",
        "thrift": "schema IDL; the thrift compiler has no formatter, it only validates",
    },
    "typecheck": {
        "ansible": "dynamically typed YAML playbooks; no type system beyond ansible-lint",
        "api_spec": "declarative API description; no type system beyond schema validation",
        "bicep": "declarative templates; bicep lint already reports compile errors and analyzer rules, so there is no separate type check",
        "cloudformation": "declarative templates; cfn-lint schema validation is lint, not type checking",
        "dockerfile": "no static type system; hadolint covers lint",
        "gherkin": "natural-language scenarios; no type system",
        "helm": "chart lint plus schema validation of rendered manifests",
        "html_template": "markup templates; no static type system",
        "jsonnet": "dynamically typed; no maintained type checker (the official linter only flags unused variables)",
        "kubernetes": "declarative manifests; schema validation is not type checking",
        "latex": "typesetting macro language; no static type system",
        "make": "no static type system; build language with untyped strings",
        "nix": "dynamically typed expression language; no maintained type checker",
        "cmake": "no static type system; build language with untyped strings",
        "perl": "dynamically typed; no maintained type checker",
        "powershell": "no type checker; PSScriptAnalyzer is a linter only",
        "rst": "markup; no static type system",
        "asciidoc": "markup; no static type system",
        "sql": "type checking needs a database schema and dialect; standalone .sql files carry neither",
        "typst": "dynamically typed markup; no standalone type checker",
        "xml": "data format; schema validation is not type checking",
        "yaml": "data format; schema validation is not type checking",
    },
}

def not_applicable_error(not_applicable, adapters, class_to_family):
    """Validates a not-applicable declaration against the adapter registry."""
    families = {}
    for family in class_to_family.values():
        families[family] = True
    for capability in not_applicable:
        if capability not in ["lint", "format", "typecheck"]:
            return "not applicable: unknown capability '" + str(capability) + "'"
        for family, reason in not_applicable[capability].items():
            if family not in families:
                return "not applicable: unknown family '" + str(family) + "' for " + capability
            if reason == "":
                return "not applicable: empty reason for " + capability + ":" + family
            for tool_id in adapters:
                for class_id in adapters[tool_id].get(capability, []):
                    if class_to_family.get(class_id) == family:
                        return "not applicable: " + capability + ":" + family + " contradicts tool '" + tool_id + "'"
    return ""

def real_supported_classes(tool_id, capability):
    """Returns the sorted supported classes for one real tool/capability."""
    if tool_id not in REAL_ADAPTERS:
        fail("adapters: unknown tool '" + tool_id +
             "': not in the real adapter registry")
    return sorted(REAL_ADAPTERS[tool_id].get(capability, []))

def _is_canonical_token(text):
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def adapter_registry_schema_error():
    """Validates the adapter-registry schema."""
    for class_id in REAL_CLASS_TO_FAMILY:
        if not _is_canonical_token(class_id):
            return "adapter registry: non-canonical class '" + str(class_id) + "'"
        family = REAL_CLASS_TO_FAMILY[class_id]
        if not _is_canonical_token(family):
            return "adapter registry: non-canonical family '" + str(family) + "' for class '" + class_id + "'"
    for tool_id in REAL_ADAPTERS:
        if not _is_canonical_token(tool_id):
            return "adapter registry: non-canonical tool '" + str(tool_id) + "'"
        for capability in REAL_ADAPTERS[tool_id]:
            if capability not in ["audit", "format", "lint", "typecheck", "unused"]:
                return "adapter registry: unknown capability '" + capability + "' for tool '" + tool_id + "'"
            for class_id in REAL_ADAPTERS[tool_id][capability]:
                if not _is_canonical_token(class_id):
                    return "adapter registry: non-canonical class '" + str(class_id) + "' for tool '" + tool_id + "'"
                if class_id not in REAL_CLASS_TO_FAMILY:
                    return "adapter registry: tool '" + tool_id + "' names unclassified class '" + class_id + "'"
    return not_applicable_error(NOT_APPLICABLE, REAL_ADAPTERS, REAL_CLASS_TO_FAMILY)
