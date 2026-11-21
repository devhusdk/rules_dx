"""The Pod modules every Perl target gets, reached through a private attribute."""

load("@rules_perl//perl:defs.bzl", "PerlInfo")
load("//core:wrapper.bzl", "dx_private_dep_rule")

perl_pod_dep = dx_private_dep_rule(
    Label("//perl/private:pod"),
    [PerlInfo, DefaultInfo],
    "Provides the Pod modules the pinned Perl lacks on macOS.",
)
