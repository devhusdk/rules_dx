"""The targets of one CPAN distribution in the cpan hub."""

load("@rules_perl//perl:perl_binary.bzl", "perl_binary")
load("@rules_perl//perl:perl_library.bzl", "perl_library")

_EXCLUDE = ["BUILD", "BUILD.bazel", "WORKSPACE", "*.bazel", "t/**", "xt/**"]

def _rooted_impl(ctx):
    outputs = []
    for src in ctx.files.srcs:
        out = ctx.actions.declare_file("%s/%s" % (ctx.attr.dir, src.short_path.split("/" + ctx.label.package + "/", 1)[-1]))
        ctx.actions.symlink(output = out, target_file = src)
        outputs.append(out)
    return [DefaultInfo(files = depset(outputs))]

_rooted = rule(
    implementation = _rooted_impl,
    doc = "Links the files of a distribution with no lib directory below dir.",
    attrs = {
        "dir": attr.string(mandatory = True, doc = "The directory the files are linked below."),
        "srcs": attr.label_list(allow_files = True, doc = "The files of the distribution."),
    },
)

def cpan_package(name, deps):
    """A `perl_library` named `name`, and a `perl_binary` `bin_<file>` for each file under `bin/` and `script/`."""
    if native.glob(["lib/**"], allow_empty = True, exclude_directories = 1):
        srcs = native.glob(["**"], exclude = _EXCLUDE)
        includes = ["lib"]
    elif "-" in name:
        top = name.split("-")[0]
        _rooted(
            name = "_rooted",
            dir = "_rooted/" + top,
            srcs = native.glob(["**"], exclude = _EXCLUDE),
        )
        srcs = [":_rooted"]
        includes = ["_rooted"]
    else:
        srcs = native.glob(["**"], exclude = _EXCLUDE)
        includes = ["."]
    perl_library(
        name = name,
        srcs = srcs,
        includes = includes,
        visibility = ["//visibility:public"],
        deps = deps,
    )
    for script in native.glob(["bin/*", "script/*"], allow_empty = True):
        perl_binary(
            name = "bin_" + script.split("/")[-1].replace(".", "_"),
            main = script,
            visibility = ["//visibility:public"],
            deps = [":" + name],
        )
