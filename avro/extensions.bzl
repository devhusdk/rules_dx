"""Module extension for the avro-cpp stub repo."""

_BUILD = """load("{missing}", "dx_missing_avro_cpp")

package(default_visibility = ["//visibility:public"])

dx_missing_avro_cpp(name = "avro_cpp")

dx_missing_avro_cpp(name = "avrogencpp")
"""

def _repo_impl(rctx):
    rctx.file("BUILD.bazel", _BUILD.format(missing = rctx.attr.missing))

_avro_cpp_stub = repository_rule(
    implementation = _repo_impl,
    attrs = {"missing": attr.string(mandatory = True)},
)

def _avro_cc_impl(mctx):
    _avro_cpp_stub(name = "dx_avro_cpp", missing = str(Label("//avro/private:missing.bzl")))
    return mctx.extension_metadata(reproducible = True)

avro_cc = module_extension(
    implementation = _avro_cc_impl,
    doc = "Creates @dx_avro_cpp; override_repo it with avro-cpp to enable avro_cc_library.",
)
