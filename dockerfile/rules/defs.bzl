"""Dockerfile source ownership for dx quality."""

load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources", "dockerfile_name_error")

def _dockerfile_library_impl(ctx):
    for f in ctx.files.srcs:
        error = dockerfile_name_error(f.basename)
        if error != "":
            fail("dockerfile_library " + str(ctx.label) + ": " + error)
    direct_sources = {}
    if len(ctx.files.srcs) > 0:
        direct_sources["dockerfile"] = depset(ctx.files.srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

dockerfile_library = rule(
    implementation = _dockerfile_library_impl,
    doc = "Owns the Dockerfiles of one package for dx lint (hadolint).",
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "Dockerfiles of this package: Dockerfile, Containerfile, Dockerfile.*, Containerfile.*, *.Dockerfile, *.dockerfile, *.Containerfile or *.containerfile.",
        ),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Other Dockerfile libraries this package depends on."),
    },
)
