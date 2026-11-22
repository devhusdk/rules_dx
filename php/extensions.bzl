"""Public PHP module extensions."""

load("//php/private/coverage:extensions.bzl", _php_coverage = "php_coverage")
load("//php/private/lock:hub.bzl", _composer = "composer")
load("//php/private/toolchain:extensions.bzl", _php = "php")

php = _php
composer = _composer
php_coverage = _php_coverage
