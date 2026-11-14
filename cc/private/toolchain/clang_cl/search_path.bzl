"""Joins library directories into one search path value for cc_args format."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@bazel_skylib//rules/directory:providers.bzl", "DirectoryInfo")

def _directory_path(target):
    if DirectoryInfo in target:
        return target[DirectoryInfo].path
    files = target[DefaultInfo].files.to_list()
    if len(files) != 1:
        fail("%s should be either a directory or a single file." % target.label)
    return files[0].path

def _search_path_impl(ctx):
    return [
        DefaultInfo(files = depset(transitive = [d[DefaultInfo].files for d in ctx.attr.directories])),
        BuildSettingInfo(value = ctx.attr.separator.join([_directory_path(d) for d in ctx.attr.directories])),
    ]

search_path = rule(
    implementation = _search_path_impl,
    attrs = {
        "directories": attr.label_list(mandatory = True),
        "separator": attr.string(default = ";"),
    },
)
