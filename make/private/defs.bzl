"""Make source ownership for dx quality."""

load("//core/quality:sources.bzl", "named_source_library")

make_library = named_source_library(
    "make",
    doc = "Owns the Makefiles of one package for dx lint (checkmake) and dx format (mbake).",
    srcs_doc = "Makefiles of this package: Makefile, makefile, GNUmakefile or *.mk.",
)
