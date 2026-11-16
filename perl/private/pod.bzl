"""The `Pod` modules of the pinned Perl release, for perls whose archive lacks them."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@rules_perl//perl/private:perl_version.bzl", "PERL_BUILD", "PERL_INTEGRITY", "PERL_RELEASES_URL", "PERL_VERSION")

_PLATFORM = "x86_64-linux-gnu"

_ARCHIVE = "perl-{}-{}".format(PERL_VERSION, _PLATFORM)

_BUILD = """\
load("{perl_library}", "perl_library")

perl_library(
    name = "pod",
    srcs = glob(["Pod/**"]),
    visibility = ["//visibility:public"],
)
""".format(perl_library = str(Label("@rules_perl//perl:perl_library.bzl")))

def _pod_impl(mctx):
    http_archive(
        name = "dx_perl_pod",
        urls = ["{}/{}/{}.tar.xz".format(PERL_RELEASES_URL, PERL_BUILD, _ARCHIVE)],
        integrity = PERL_INTEGRITY[_PLATFORM],
        strip_prefix = "{}/lib/{}".format(_ARCHIVE, PERL_VERSION),
        build_file_content = _BUILD,
    )
    return mctx.extension_metadata(reproducible = True)

pod = module_extension(implementation = _pod_impl)
