"""Analysis tests for the inputs that make Dart builds fail on undeclared dependencies."""

load("//tools/testing:action_input.bzl", "action_input_test")

def dart_module_input_test(name, target, file, present):
    action_input_test(name, target, "DartModule", file, present)
