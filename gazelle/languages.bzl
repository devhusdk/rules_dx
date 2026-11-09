"""Gazelle language packages composed into dispatch."""

GAZELLE_LANGUAGES = [
    "astro",
    "avro",
    "capnproto",
    "cc",
    "csharp",
    "d",
    "dart",
    "elixir",
    "erlang",
    "flatbuffers",
    "fsharp",
    "go",
    "haskell",
    "java",
    "javascript",
    "julia",
    "kotlin",
    "lua",
    "mdx",
    "ocaml",
    "ownership",
    "perl",
    "php",
    "powershell",
    "protobuf",
    "python",
    "ruby",
    "rust",
    "scala",
    "svelte",
    "swift",
    "thrift",
    "typescript",
    "vue",
    "zig",
]

GAZELLE_SUPPORT_PACKAGES = [
    "common",
    "dispatch",
    "dotnetplatform",
]

def gazelle_languages_test_source(name):
    lines = ["package dispatch", "", "import ("]
    lines += ['\t_ "github.com/devhusdk/rules_dx/gazelle/%s"' % lang for lang in GAZELLE_LANGUAGES]
    lines += [")", ""]
    lines.append("var registeredPackages = []string{%s}" % ", ".join(['"%s"' % lang for lang in GAZELLE_LANGUAGES]))
    lines.append("var supportPackages = []string{%s}" % ", ".join(['"%s"' % pkg for pkg in GAZELLE_SUPPORT_PACKAGES]))
    native.genrule(
        name = name,
        outs = ["all_languages_test.go"],
        cmd = "cat > $@ <<'EOF'\n" + "\n".join(lines) + "\nEOF",
    )
