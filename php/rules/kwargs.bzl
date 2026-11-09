load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//php/toolchain:pins.bzl", "PHP_CELLS")

PHP_COMPATIBLE_WITH = cells_compatible_with(PHP_CELLS, "//php/toolchain", sorted(PHP_CELLS.keys()), unsupported = True)

PHP_DEFERRED_SKIP_TAGS = ["no-format"]

def php_compatible_with(target_compatible_with):
    """Appends the pinned PHP cells to the caller's constraints."""
    return (target_compatible_with or []) + PHP_COMPATIBLE_WITH

def php_deferred_tags(tags):
    """Adds the skip tags that keep the deferred php class out of quality aspects."""
    out = list(tags or [])
    for tag in PHP_DEFERRED_SKIP_TAGS:
        if tag not in out:
            out.append(tag)
    return out

def php_kwargs(kwargs):
    """Returns kwargs with the deferred quality tags and the PHP cell constraints."""
    return dict(
        kwargs,
        tags = php_deferred_tags(kwargs.get("tags")),
        target_compatible_with = php_compatible_with(kwargs.get("target_compatible_with")),
    )
