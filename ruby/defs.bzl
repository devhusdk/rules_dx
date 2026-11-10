"""Public Ruby API."""

load(
    "//ruby/private:defs.bzl",
    _ruby_bench = "ruby_bench",
    _ruby_binary = "ruby_binary",
    _ruby_coverage_lib_dirs = "ruby_coverage_lib_dirs",
    _ruby_library = "ruby_library",
    _ruby_test = "ruby_test",
)

ruby_coverage_lib_dirs = _ruby_coverage_lib_dirs
ruby_library = _ruby_library
ruby_binary = _ruby_binary
ruby_test = _ruby_test
ruby_bench = _ruby_bench
