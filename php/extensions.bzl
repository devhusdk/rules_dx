"""Public PHP module extensions."""

load("//php/private/lock:hub.bzl", _composer = "composer")
load("//php/private/toolchain:extensions.bzl", _php = "php")

php = _php
composer = _composer
