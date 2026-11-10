"""Exposes the scalafix lint results of one target under the scalafix_aux configuration."""

load("//core/quality/testdata:config_subject.bzl", "config_results")
load("//core/quality/testdata:real_aspect_subject.bzl", "real_aspect_subject")

SCALAFIX_CONFIG = "scalafix_aux"

def scalafix_results(name, target):
    """Declares the scalafix lint results of target."""
    real_aspect_subject(name = name + "_subject", target = target, tags = ["manual"], testonly = True)
    config_results(name = name, allow_empty = True, config = SCALAFIX_CONFIG, subject = name + "_subject", tags = ["manual"], testonly = True)
