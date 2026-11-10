"""Public Java API."""

load(
    "//java/private:defs.bzl",
    _java_library = "java_library",
    _java_binary = "java_binary",
    _java_test = "java_test",
    _java_jmh_test = "java_jmh_test",
    _java_fuzz_test = "java_fuzz_test",
    _java_bench = "java_bench",
)

java_library = _java_library
java_binary = _java_binary
java_test = _java_test
java_jmh_test = _java_jmh_test
java_fuzz_test = _java_fuzz_test
java_bench = _java_bench
