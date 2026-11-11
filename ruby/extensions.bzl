"""Public Ruby module extensions."""

load("@rules_ruby//ruby:extensions.bzl", _ruby = "ruby")
load("//ruby/private/coverage:extensions.bzl", _ruby_coverage = "ruby_coverage")

ruby = _ruby
ruby_coverage = _ruby_coverage
