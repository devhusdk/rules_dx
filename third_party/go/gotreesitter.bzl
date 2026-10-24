"""Repository rule for the gotreesitter Go module with the Java, Kotlin, and Scala grammars."""

_IMPORTPATH = "github.com/odvcencio/gotreesitter"

_VERSION = "v0.56.0"

_SHA256 = "0925d93fadca3ad943e20594d602c3639d79f9ad8f82d466262672890d7592a6"

_INTERNAL = [
    "internal/declarationfacts",
    "internal/incr",
    "internal/luapattern",
    "internal/parsercorephase0",
    "internal/recover",
    "internal/recoverymerge",
    "internal/recoveryturn",
    "internal/reducechoice",
    "internal/scannerstate",
    "internal/sched",
]

_GRAMMAR_DEPS = ["", "grammars/grammar_blobs", "grammars/runtime"]

_PACKAGES = dict(
    [(path, []) for path in _INTERNAL] + [
        ("internal/syntaxfacts", ["internal/declarationfacts"]),
        ("", _INTERNAL + ["internal/syntaxfacts"]),
        ("grammars/grammar_blobs", []),
        ("grammars/internal/pythonruntime", [""]),
        ("grammars/runtime", ["", "grammars/internal/pythonruntime"]),
        ("grammars/java", _GRAMMAR_DEPS),
        ("grammars/kotlin", _GRAMMAR_DEPS),
        ("grammars/scala", _GRAMMAR_DEPS),
    ],
)

_EMBEDS = {
    "": '["testdata/result_compat_ownership_v1.json", "//grammars/grammar_blobs:go.bin"]',
    "grammars/grammar_blobs": 'glob(["*.bin"])',
}

_BUILD = """load("@rules_go//go:def.bzl", "go_library")
{exports}
go_library(
    name = "{name}",
    srcs = glob(["*.go"], exclude = ["*_test.go"]),
    embedsrcs = {embeds},
    importpath = "{importpath}",
    visibility = ["//visibility:public"],
    deps = {deps},
)
"""

def _name(path):
    return path.rsplit("/", 1)[-1] if path else "gotreesitter"

def _label(path):
    return "//{}:{}".format(path, _name(path))

def _gotreesitter_repo_impl(rctx):
    rctx.download_and_extract(
        url = "https://proxy.golang.org/{}/@v/{}.zip".format(_IMPORTPATH, _VERSION),
        sha256 = _SHA256,
        type = "zip",
        stripPrefix = "{}@{}".format(_IMPORTPATH, _VERSION),
    )
    for path, deps in _PACKAGES.items():
        rctx.file(
            "{}/BUILD.bazel".format(path) if path else "BUILD.bazel",
            _BUILD.format(
                exports = '\nexports_files(["go.bin"])\n' if path == "grammars/grammar_blobs" else "",
                name = _name(path),
                embeds = _EMBEDS.get(path, "[]"),
                importpath = "/".join([_IMPORTPATH, path]) if path else _IMPORTPATH,
                deps = repr([_label(dep) for dep in deps]),
            ),
        )

gotreesitter_repo = repository_rule(
    implementation = _gotreesitter_repo_impl,
    doc = "Fetches gotreesitter and writes BUILD files for the packages the JVM Gazelle languages use.",
)
