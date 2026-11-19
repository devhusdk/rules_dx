"""Public Ruby API."""

load(
    "//ruby/private:defs.bzl",
    _ruby_bench = "ruby_bench",
    _ruby_binary = "ruby_binary",
    _ruby_library = "ruby_library",
    _ruby_test = "ruby_test",
)
load("//tools/docs/ruby:defs.bzl", _ruby_doc_shard = "ruby_doc_shard")

ruby_library = _ruby_library
ruby_binary = _ruby_binary
ruby_test = _ruby_test
ruby_bench = _ruby_bench
ruby_doc_shard = _ruby_doc_shard
