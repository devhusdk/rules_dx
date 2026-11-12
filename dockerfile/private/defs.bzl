"""Dockerfile source ownership for dx quality."""

load("//core/quality:sources.bzl", "dockerfile_name_error", "named_source_library")

dockerfile_library = named_source_library(
    "dockerfile",
    doc = "Owns the Dockerfiles of one package for dx lint (hadolint).",
    srcs_doc = "Dockerfiles of this package: Dockerfile, Containerfile, Dockerfile.*, Containerfile.*, *.Dockerfile, *.dockerfile, *.Containerfile or *.containerfile.",
    name_error = dockerfile_name_error,
)
