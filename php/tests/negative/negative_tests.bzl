"""Analysis tests for the sources a PHP autoloader is built from."""

load("//tools/testing:action_input.bzl", "action_input_test")

def php_autoload_input_test(name, target, file, present):
    """Checks whether the autoload action of target reads file."""
    action_input_test(name, target, "PhpAutoload", file, present)
