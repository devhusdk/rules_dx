"""Audit curator declared-Bazel-inputs pins."""

CURATOR_LABEL = "//:audit_curator"

CURATOR_REL = "licenses.toml"

CURATOR_ADVISORY_SETS = ["cargo", "npm", "maven", "nuget", "go", "rubygems"]

CURATOR_SNAPSHOT_RELS = [
    ".dx/advisory/cargo.json",
    ".dx/advisory/cargo.meta.json",
    ".dx/advisory/npm.json",
    ".dx/advisory/npm.meta.json",
    ".dx/advisory/maven.json",
    ".dx/advisory/maven.meta.json",
    ".dx/advisory/nuget.json",
    ".dx/advisory/nuget.meta.json",
    ".dx/advisory/go.json",
    ".dx/advisory/go.meta.json",
    ".dx/advisory/rubygems.json",
    ".dx/advisory/rubygems.meta.json",
]

INVENTORY_SHAPE = ["package", "set", "license", "versions", "text_present"]

NOTICE_HERMETIC = "deploy/release/notice.bzl notice_bundle hermetic"

SEMANTICS_PRESERVED = "same policy plus freshness plus reporting"

REJECTED_STALE_FALLBACK = "stale snapshot fallback rejected"
REJECTED_UNTRACKED_DB = "untracked live database rejected"
REJECTED_SHAPE_DRIFT = "per-package shape drift rejected"

SEED_ONLY = "qualified seed-only"
NO_SUPPORTED = "no Supported claim"
