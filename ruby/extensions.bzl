"""Re-exports the rules_ruby module extension."""

load("@rules_ruby//ruby:extensions.bzl", _ruby = "ruby")

ruby = _ruby
