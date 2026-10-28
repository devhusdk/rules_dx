"""Deterministic synthetic adapter registry."""

ADAPTER_REGISTRY_SCHEMA_VERSION = 1

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

REAL_ADAPTERS = {
    "ansible_lint": {"lint": ["ansible"]},
    "biome": {
        "format": ["javascript", "json", "jsx", "typescript", "tsx"],
        "lint": ["javascript", "json", "jsx", "typescript", "tsx"],
    },
    "buf": {
        "format": ["protobuf"],
        "lint": ["protobuf"],
    },
    "buildifier": {"format": ["starlark"], "lint": ["starlark"]},
    "checkstyle": {"lint": ["java"]},
    "clang_format": {"format": ["c", "cpp", "cuda", "objc", "objcpp"]},
    "clang_tidy": {"lint": ["c", "cpp"]},
    "clippy": {"lint": ["rust"]},
    "cppcheck": {"lint": ["c", "cpp"]},
    "csharpier": {"format": ["csharp"]},
    "cue": {"format": ["cue"]},
    "deadnix": {"lint": ["nix"]},
    "djlint": {"format": ["html_template"], "lint": ["html_template"]},
    "erlfmt": {"format": ["erlang"]},
    "errcheck": {"lint": ["go"]},
    "eslint": {"lint": ["astro", "javascript", "jsx", "mdx", "svelte", "vue"]},
    "fantomas": {"format": ["fsharp"]},
    "flake8": {"lint": ["python", "python_stub"]},
    "fsharplint": {"lint": ["fsharp"]},
    "gofumpt": {"format": ["go"]},
    "graphql_eslint": {"lint": ["graphql"]},
    "hadolint": {"lint": ["dockerfile"]},
    "google_java_format": {"format": ["java"]},
    "govet": {"lint": ["go"]},
    "jsonnetfmt": {"format": ["jsonnet"]},
    "keep_sorted": {"lint": ["text"]},
    "ktfmt": {"format": ["kotlin"]},
    "ktlint": {"lint": ["kotlin"]},
    "kubeconform": {"lint": ["kubernetes"]},
    "markdown_check": {"lint": ["markdown"]},
    "mix_format": {"format": ["elixir"]},
    "modfmt": {"format": ["go_module"]},
    "ormolu": {"format": ["haskell"]},
    "pkl": {"format": ["pkl"]},
    "pmd": {"lint": ["java"]},
    "prettier": {"format": ["astro", "css", "gherkin", "graphql", "html", "javascript", "json", "json5", "jsonc", "jsx", "less", "scss", "sql", "svelte", "tsx", "typescript", "vue", "xml"]},
    "psscriptanalyzer": {"format": ["powershell"], "lint": ["powershell"]},
    "redocly": {"lint": ["api_spec"]},
    "rubocop": {"lint": ["ruby"]},
    "pydoclint": {"lint": ["python", "python_stub"]},
    "pylint": {"lint": ["python", "python_stub"]},
    "qmlformat": {"format": ["qml"]},
    "qmllint": {"lint": ["qml"]},
    "remark": {"format": ["mdx"]},
    "roslyn": {"lint": ["csharp"]},
    "ruff": {"audit": ["python", "python_stub"], "format": ["python", "python_stub"], "lint": ["python", "python_stub"]},
    "rustc": {"typecheck": ["rust"]},
    "rustfmt": {"format": ["rust"]},
    "scalafix": {"lint": ["scala"]},
    "scalafmt": {"format": ["scala"]},
    "shellcheck": {"lint": ["shell"]},
    "shfmt": {"format": ["shell"]},
    "spotbugs": {"lint": ["java"]},
    "statix": {"lint": ["nix"]},
    "standardrb": {"format": ["ruby"]},
    "staticcheck": {"lint": ["go"]},
    "stylelint": {"lint": ["css", "less", "scss"]},
    "stylua": {"format": ["lua"]},
    "taplo": {"format": ["toml"], "lint": ["toml"]},
    "terraform": {"format": ["terraform"]},
    "ty": {"typecheck": ["python", "python_stub"]},
    "vale": {"lint": ["markdown"]},
    "yamlfmt": {"format": ["yaml"]},
    "yamllint": {"lint": ["yaml"]},
    "zig_fmt": {"format": ["zig"]},
}

REAL_CLASS_TO_FAMILY = {
    "ansible": "ansible",
    "api_spec": "api_spec",
    "astro": "astro",
    "c": "cc",
    "cpp": "cc",
    "csharp": "csharp",
    "css": "css",
    "cuda": "cuda",
    "cue": "cue",
    "dockerfile": "dockerfile",
    "d": "d",
    "elixir": "elixir",
    "erlang": "erlang",
    "fortran": "fortran",
    "fsharp": "fsharp",
    "gherkin": "gherkin",
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
    "kotlin": "kotlin",
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
    "toml": "toml",
    "typescript": "typescript",
    "tsx": "typescript",
    "vue": "vue",
    "jsonnet": "jsonnet",
    "xml": "xml",
    "yaml": "yaml",
    "zig": "zig",
}

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
    """Validates the versioned adapter-registry schema."""
    if ADAPTER_REGISTRY_SCHEMA_VERSION != 1:
        return "adapter registry: unsupported schema v" + str(ADAPTER_REGISTRY_SCHEMA_VERSION) + " (want v1)"
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
            if capability not in ["audit", "format", "lint", "typecheck"]:
                return "adapter registry: unknown capability '" + capability + "' for tool '" + tool_id + "'"
            for class_id in REAL_ADAPTERS[tool_id][capability]:
                if not _is_canonical_token(class_id):
                    return "adapter registry: non-canonical class '" + str(class_id) + "' for tool '" + tool_id + "'"
                if class_id not in REAL_CLASS_TO_FAMILY:
                    return "adapter registry: tool '" + tool_id + "' names unclassified class '" + class_id + "'"
    return ""
