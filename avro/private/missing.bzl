"""Stub for avro-cpp when the consumer does not provide it."""

_MESSAGE = "avro_cc_library needs avro-cpp: add bazel_dep(name = \"avro-cpp\", version = \"1.12.0.bcr.1\"), the single_version_override entries for avro-cpp and fmt from rules_dx, and override_repo(use_extension(\"@rules_dx//avro:extensions.bzl\", \"avro_cc\"), dx_avro_cpp = \"avro-cpp\")"

def _impl(ctx):
    fail(_MESSAGE + " (" + str(ctx.label) + ")")

dx_missing_avro_cpp = rule(
    implementation = _impl,
    doc = "Fails analysis with instructions for providing avro-cpp.",
    executable = True,
)
