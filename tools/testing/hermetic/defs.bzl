"""No-install, network-denied consumer qualification targets."""

load("//cc/toolchains:pins.bzl", "GCC_VERSION", "LLVM_CLANG_VERSION", "MSVC_CL_BUILD")
load("//rust:defs.bzl", "rust_test")
load("//tools/testing:nested_bazel.bzl", "NESTED_BAZEL_DATA", "NESTED_BAZEL_ENV", "NESTED_BAZEL_ENV_INHERIT", "NESTED_BAZEL_NETWORK_ENV", "NESTED_BAZEL_NETWORK_TAGS", "NESTED_BAZEL_TAGS")

# The string each compiler leaves in the binaries it builds: the ELF .comment entry, or the
# PE Rich header entry of cl.exe compiling C++.
COMPILER_PRODUCERS = {
    "gcc": "GCC: (GNU) " + GCC_VERSION,
    "llvm": "clang version " + LLVM_CLANG_VERSION,
    "msvc": "msvc 261." + MSVC_CL_BUILD,
}

def _hermetic_manifest_impl(ctx):
    """Writes the consumer manifest and carries every file it names."""
    compiler = ctx.attr.compiler
    if compiler and compiler not in COMPILER_PRODUCERS:
        fail("%s: unknown compiler %r, want one of %s" % (ctx.label, compiler, sorted(COMPILER_PRODUCERS)))
    files = []
    for target, dest in ctx.attr.files.items():
        srcs = target.files.to_list()
        if len(srcs) != 1:
            fail("%s must name exactly one file for %s" % (target.label, dest))
        files.append({"dest": dest, "src": srcs[0].short_path})
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode_indent({
        "bazelversion": ctx.file._bazelversion.short_path,
        "compiler": {"name": compiler, "producer": COMPILER_PRODUCERS.get(compiler, "")},
        "coverage": ctx.attr.coverage,
        "covered": ctx.attr.covered,
        "dx": ctx.executable._dx.short_path,
        "failures": ctx.attr.failures,
        "generate": ctx.attr.generate,
        "families": ctx.attr.families,
        "files": files,
        "label": ctx.attr.qualification,
        "lint_findings": ctx.attr.lint_findings,
        "languages": ctx.attr.languages,
        "module": ctx.attr.module,
        "module_file": ctx.file._module.short_path,
        "module_fragment": ctx.file.module_fragment.short_path if ctx.file.module_fragment else "",
        "patterns": ctx.attr.patterns,
        "platform": ctx.attr.platform,
        "repo_name": ctx.attr.repo_name,
        "workspace_dir": ctx.attr.workspace_dir,
    }, indent = "  ") + "\n")
    runfiles = ctx.runfiles(
        files = [out, ctx.file._bazelversion, ctx.file._module] + ([ctx.file.module_fragment] if ctx.file.module_fragment else []) + ctx.files.files + ctx.files.srcs,
    ).merge(ctx.attr._dx[DefaultInfo].default_runfiles)
    return [DefaultInfo(files = depset([out]), runfiles = runfiles)]

hermetic_manifest = rule(
    implementation = _hermetic_manifest_impl,
    doc = "Describes one consumer workspace for hermetic qualification.",
    attrs = {
        "compiler": attr.string(doc = "Compiler the consumer artifacts must be built by (gcc, llvm or msvc); empty skips the check."),
        "coverage": attr.string_list(doc = "Consumer target patterns to run under coverage with the network denied."),
        "covered": attr.string_list(doc = "Consumer source paths the coverage report must show as hit."),
        "failures": attr.string_dict(doc = "Manual consumer targets that must fail to build, mapped to the log marker of the failure."),
        "families": attr.string_list(doc = "Family modules under families/ that the consumer depends on, each overridden to its source path."),
        "generate": attr.bool(doc = "Run dx generate on the patterns with the network allowed before the fetch probe."),
        "files": attr.label_keyed_string_dict(
            allow_files = True,
            doc = "Consumer files mapped to their workspace-relative paths.",
        ),
        "languages": attr.string_list(mandatory = True, doc = "Languages this consumer qualifies."),
        "lint_findings": attr.string_dict(doc = "Findings dx lint must report, as tool:rule mapped to a decimal count; empty skips the lint probe."),
        "module": attr.string(mandatory = True, doc = "Consumer module name."),
        "module_fragment": attr.label(
            allow_single_file = True,
            doc = "MODULE.bazel lines added to the dx init snippet; appended to the dx init snippet.",
        ),
        "patterns": attr.string_list(mandatory = True, doc = "Consumer target patterns to fetch, build and test."),
        "platform": attr.string(doc = "Platform label passed to every consumer bazel command as --platforms; empty leaves the consumer default."),
        "qualification": attr.string(mandatory = True, doc = "Label of the qualification test."),
        "repo_name": attr.string(mandatory = True, doc = "Repository name the consumer gives rules_dx."),
        "srcs": attr.label_list(allow_files = True, doc = "rules_dx sources the consumer build reads."),
        "workspace_dir": attr.string(mandatory = True, doc = "Consumer workspace directory name."),
        "_bazelversion": attr.label(allow_single_file = True, default = "//:.bazelversion"),
        "_dx": attr.label(cfg = "target", default = "//:dx", executable = True),
        "_module": attr.label(allow_single_file = True, default = "//:MODULE.bazel"),
    },
)

# Read by the support matrix (tools/docs/support_matrix) for the hermetic_qualification kind.
# Targets that cannot run on Windows (AppImage, GCC) pass their own compatible_with.
HERMETIC_COMPATIBLE_WITH = select({
    "@platforms//os:linux": [],
    "@platforms//os:macos": [],
    "@platforms//os:windows": [],
    "//conditions:default": ["@platforms//:incompatible"],
})

# The manifest only lists the consumer; the unit tests read it on macOS and Windows too.
MANIFEST_COMPATIBLE_WITH = select({
    "@platforms//os:linux": [],
    "@platforms//os:macos": [],
    "@platforms//os:windows": [],
    "//conditions:default": ["@platforms//:incompatible"],
})

QUALIFICATION_TAGS = [
    "exclusive",
    "hermetic-qualification",
    "manual",
    "no-cache",
    "no-coverage",
    "no-remote",
    "no-sandbox",
    "requires-network",
]

def hermetic_qualification(
        name,
        languages,
        files,
        patterns,
        module_fragment = None,
        coverage = [],
        covered = [],
        compiler = "",
        failures = {},
        families = [],
        generate = False,
        lint_findings = {},
        module = "hermetic_consumer",
        platform = "",
        repo_name = "my_dx",
        srcs = None,
        smoke = False,
        compatible_with = HERMETIC_COMPATIBLE_WITH,
        workspace_dir = "consumer ws",
        **kwargs):
    """Declares a consumer manifest and a golden check; smoke also runs the nested qualification."""
    srcs = srcs or []
    hermetic_manifest(
        name = name + "_manifest",
        testonly = True,
        compiler = compiler,
        coverage = coverage,
        covered = covered,
        failures = failures,
        families = families,
        generate = generate,
        files = files,
        languages = languages,
        lint_findings = lint_findings,
        module = module,
        module_fragment = module_fragment,
        patterns = patterns,
        platform = platform,
        qualification = "//%s:%s" % (native.package_name(), name),
        repo_name = repo_name,
        srcs = srcs,
        target_compatible_with = MANIFEST_COMPATIBLE_WITH,
        workspace_dir = workspace_dir,
    )
    if not smoke:
        golden = "//tools/testing/hermetic:golden/%s.txt" % name
        rust_test(
            name = name,
            srcs = ["//tools/testing/hermetic:manifest_golden_test.rs"],
            crate_root = "//tools/testing/hermetic:manifest_golden_test.rs",
            data = [":" + name + "_manifest", golden],
            env = {
                "DX_GOLDEN": "$(rootpath %s)" % golden,
                "DX_HERMETIC_MANIFEST": "$(rootpath :%s_manifest)" % name,
            },
            size = "small",
            tags = ["hermetic-golden"],
            target_compatible_with = compatible_with,
            deps = [
                "//tools/testing:dx_testing",
                "@crates//:serde_json",
            ],
        )
        return
    rust_test(
        name = name,
        timeout = kwargs.pop("timeout", "moderate"),
        srcs = ["//tools/testing/hermetic:qualification_test.rs"],
        data = [":" + name + "_manifest"] + select({
            "@platforms//os:windows": ["//tools/testing/hermetic:appcontainer_run"],
            "//conditions:default": [],
        }),
        env = select({
            "@platforms//os:windows": {
                "DX_HERMETIC_APPCONTAINER": "$(rlocationpath //tools/testing/hermetic:appcontainer_run)",
                "DX_HERMETIC_MANIFEST": "$(rootpath :%s_manifest)" % name,
            },
            "//conditions:default": {"DX_HERMETIC_MANIFEST": "$(rootpath :%s_manifest)" % name},
        }),
        tags = QUALIFICATION_TAGS + ["hermetic-consumer", "resources:nested_bazel:1"],
        target_compatible_with = compatible_with,
        deps = [
            "//tools/testing:dx_testing",
            "//tools/testing/hermetic",
        ],
        **kwargs
    )

def nested_bazel_test(name, *, network = False, data = None, env = None, tags = None, target_compatible_with = None, **kwargs):
    """Declares a rust_test that runs a nested Bazel with the shared tags, env and offline downloader."""
    target_compatible_with = target_compatible_with or []
    env_inherit = kwargs.pop("env_inherit", NESTED_BAZEL_ENV_INHERIT)
    rust_test(
        name = name,
        data = (data or []) + NESTED_BAZEL_DATA,
        env = {"RUST_TEST_THREADS": "1"} | (NESTED_BAZEL_NETWORK_ENV if network else NESTED_BAZEL_ENV) | (env or {}),
        tags = (NESTED_BAZEL_NETWORK_TAGS if network else NESTED_BAZEL_TAGS) + (tags or []),
        target_compatible_with = target_compatible_with,
        env_inherit = env_inherit,
        **kwargs
    )
