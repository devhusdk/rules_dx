"""Public PHP module extensions."""

load("//php/private/composer:extensions.bzl", _composer = "composer")
load("//php/private/toolchain:extensions.bzl", _php = "php")

php = _php
composer = _composer
