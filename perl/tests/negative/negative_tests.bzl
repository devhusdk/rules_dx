"""Analysis tests for the files a Perl test can see at run time."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_LINUX = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _runfile_impl(ctx):
    env = analysistest.begin(ctx)
    files = analysistest.target_under_test(env)[DefaultInfo].default_runfiles.files.to_list()
    paths = [f.short_path for f in files]
    found = [p for p in paths if p == ctx.attr.file or p.endswith("/" + ctx.attr.file)]
    asserts.equals(env, ctx.attr.present, len(found) > 0, "runfiles: " + str(paths))
    return analysistest.end(env)

_runfile_test = analysistest.make(_runfile_impl, attrs = {
    "file": attr.string(),
    "present": attr.bool(),
})

def perl_runfile_test(name, target, file, present):
    """Checks whether the test target carries file in its runfiles."""
    _runfile_test(
        name = name,
        size = "small",
        target_under_test = target,
        file = file,
        present = present,
        target_compatible_with = _LINUX,
    )
