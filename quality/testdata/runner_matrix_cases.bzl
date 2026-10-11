"""Layer-2 matrix cases (snapshot workflow): every supported language x capability cell."""

load(":runner_matrix_data.bzl", "DATA_CASES")
load(":runner_matrix_file_family.bzl", "FILE_FAMILY_CASES")
load(":runner_matrix_js.bzl", "JS_CASES")
load(":runner_matrix_json.bzl", "JSON_CASES")
load(":runner_matrix_jvm.bzl", "JVM_CASES")
load(":runner_matrix_markdown.bzl", "MARKDOWN_CASES")
load(":runner_matrix_markup.bzl", "MARKUP_CASES")
load(":runner_matrix_native.bzl", "NATIVE_CASES")
load(":runner_matrix_python.bzl", "PYTHON_CASES")
load(":runner_matrix_rust.bzl", "RUST_CASES")
load(":runner_matrix_scala_dotnet.bzl", "SCALA_DOTNET_CASES")
load(":runner_matrix_structured.bzl", "STRUCTURED_CASES")
load(":runner_matrix_tests.bzl", "runner_matrix_suite")
load(":runner_matrix_ts.bzl", "TS_CASES")

def runner_matrix_cases(name):
    runner_matrix_suite(name, RUST_CASES + PYTHON_CASES + JS_CASES + TS_CASES + JSON_CASES + MARKUP_CASES + DATA_CASES + MARKDOWN_CASES + JVM_CASES + SCALA_DOTNET_CASES + NATIVE_CASES + STRUCTURED_CASES + FILE_FAMILY_CASES)
