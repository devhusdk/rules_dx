"""No-install, network-denied consumer qualification targets."""

load("//rust/rules:defs.bzl", "rust_test")
load("//tools/testing:nested_bazel.bzl", "NESTED_BAZEL_COMPATIBLE_WITH", "NESTED_BAZEL_DATA", "NESTED_BAZEL_ENV", "NESTED_BAZEL_ENV_INHERIT", "NESTED_BAZEL_NETWORK_ENV", "NESTED_BAZEL_NETWORK_TAGS", "NESTED_BAZEL_TAGS")

def _hermetic_manifest_impl(ctx):
    """Writes the consumer manifest and carries every file it names."""
    files = []
    for target, dest in ctx.attr.files.items():
        srcs = target.files.to_list()
        if len(srcs) != 1:
            fail("%s must name exactly one file for %s" % (target.label, dest))
        files.append({"dest": dest, "src": srcs[0].short_path})
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode_indent({
        "bazelversion": ctx.file._bazelversion.short_path,
        "coverage": ctx.attr.coverage,
        "covered": ctx.attr.covered,
        "dx": ctx.executable._dx.short_path,
        "failures": ctx.attr.failures,
        "files": files,
        "fragment_before_toolchains": ctx.attr.fragment_before_toolchains,
        "label": ctx.attr.qualification,
        "languages": ctx.attr.languages,
        "module": ctx.attr.module,
        "module_file": ctx.file._module.short_path,
        "module_fragment": ctx.file.module_fragment.short_path,
        "patterns": ctx.attr.patterns,
        "repo_name": ctx.attr.repo_name,
        "rust_extra_rustc_flags": ctx.attr.rust_extra_rustc_flags,
        "rust_target_triples": ctx.attr.rust_target_triples,
        "workspace_dir": ctx.attr.workspace_dir,
    }, indent = "  ") + "\n")
    runfiles = ctx.runfiles(
        files = [out, ctx.file._bazelversion, ctx.file._module, ctx.file.module_fragment] + ctx.files.files + ctx.files.srcs,
    ).merge(ctx.attr._dx[DefaultInfo].default_runfiles)
    return [DefaultInfo(files = depset([out]), runfiles = runfiles)]

hermetic_manifest = rule(
    implementation = _hermetic_manifest_impl,
    doc = "Describes one consumer workspace for hermetic qualification.",
    attrs = {
        "coverage": attr.string_list(doc = "Consumer target patterns to run under coverage with the network denied."),
        "covered": attr.string_list(doc = "Consumer source paths the coverage report must show as hit."),
        "failures": attr.string_dict(doc = "Manual consumer targets that must fail to build, mapped to the log marker of the failure."),
        "files": attr.label_keyed_string_dict(
            allow_files = True,
            doc = "Consumer files mapped to their workspace-relative paths.",
        ),
        "fragment_before_toolchains": attr.bool(
            doc = "Insert module_fragment before the first register_toolchains of the dx init snippet instead of after it.",
        ),
        "languages": attr.string_list(mandatory = True, doc = "Languages this consumer qualifies."),
        "module": attr.string(mandatory = True, doc = "Consumer module name."),
        "module_fragment": attr.label(
            allow_single_file = True,
            mandatory = True,
            doc = "MODULE.bazel lines added to the dx init snippet; appended unless fragment_before_toolchains.",
        ),
        "patterns": attr.string_list(mandatory = True, doc = "Consumer target patterns to fetch, build and test."),
        "qualification": attr.string(mandatory = True, doc = "Label of the qualification test."),
        "repo_name": attr.string(mandatory = True, doc = "Repository name the consumer gives rules_dx."),
        "rust_extra_rustc_flags": attr.string_list(doc = "rustc flags added to the dx init rust.toolchain as extra_rustc_flags."),
        "rust_target_triples": attr.string_list(doc = "Rust target triples added to the dx init rust.toolchain."),
        "srcs": attr.label_list(allow_files = True, doc = "rules_dx sources the consumer build reads."),
        "workspace_dir": attr.string(mandatory = True, doc = "Consumer workspace directory name."),
        "_bazelversion": attr.label(allow_single_file = True, default = "//:.bazelversion"),
        "_dx": attr.label(cfg = "target", default = "//:dx", executable = True),
        "_module": attr.label(allow_single_file = True, default = "//:MODULE.bazel"),
    },
)

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
        module_fragment,
        patterns,
        coverage = [],
        covered = [],
        failures = {},
        fragment_before_toolchains = False,
        module = "hermetic_consumer",
        repo_name = "my_dx",
        rust_extra_rustc_flags = [],
        rust_target_triples = [],
        srcs = [],
        workspace_dir = "consumer ws",
        **kwargs):
    """Declares one consumer qualification test and its manifest."""
    hermetic_manifest(
        name = name + "_manifest",
        testonly = True,
        coverage = coverage,
        covered = covered,
        failures = failures,
        files = files,
        fragment_before_toolchains = fragment_before_toolchains,
        languages = languages,
        module = module,
        module_fragment = module_fragment,
        patterns = patterns,
        qualification = "//%s:%s" % (native.package_name(), name),
        repo_name = repo_name,
        rust_extra_rustc_flags = rust_extra_rustc_flags,
        rust_target_triples = rust_target_triples,
        srcs = srcs,
        target_compatible_with = ["@platforms//os:linux"],
        workspace_dir = workspace_dir,
    )
    rust_test(
        name = name,
        size = "enormous",
        srcs = ["//tools/testing/hermetic:qualification_test.rs"],
        data = [":" + name + "_manifest"],
        env = {"DX_HERMETIC_MANIFEST": "$(rootpath :%s_manifest)" % name},
        tags = QUALIFICATION_TAGS + ["hermetic-consumer"],
        target_compatible_with = ["@platforms//os:linux"],
        deps = [
            "//tools/testing:dx_testing",
            "//tools/testing/hermetic",
        ],
        **kwargs
    )

def nested_bazel_test(name, network = False, data = None, env = None, tags = None, target_compatible_with = None, **kwargs):
    """Declares a rust_test that runs a nested Bazel with the shared tags, env and offline downloader."""
    env_inherit = kwargs.pop("env_inherit", NESTED_BAZEL_ENV_INHERIT)
    rust_test(
        name = name,
        data = (data or []) + NESTED_BAZEL_DATA,
        env = {"RUST_TEST_THREADS": "1"} | (NESTED_BAZEL_NETWORK_ENV if network else NESTED_BAZEL_ENV) | (env or {}),
        tags = (NESTED_BAZEL_NETWORK_TAGS if network else NESTED_BAZEL_TAGS) + (tags or []),
        target_compatible_with = target_compatible_with or NESTED_BAZEL_COMPATIBLE_WITH,
        env_inherit = env_inherit,
        **kwargs
    )
