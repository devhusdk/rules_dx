"""Exposes the real lint aspect results of one target under the workspace configuration."""

load("//core/quality/testdata:config_subject.bzl", "config_results")
load("//core/quality/testdata:real_aspect_subject.bzl", "real_aspect_subject")

SPOTBUGS_CONFIG = "workspace"

def spotbugs_results(name, target):
    """Declares the lint results of target."""
    real_aspect_subject(name = name + "_subject", target = target, tags = ["manual"], testonly = True)
    config_results(name = name, allow_empty = True, config = SPOTBUGS_CONFIG, subject = name + "_subject", tags = ["manual"], testonly = True)
