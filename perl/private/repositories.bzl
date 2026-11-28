"""Re-exports the rules_perl interpreter repositories extension."""

load("@rules_perl//perl:extensions.bzl", _perl_repositories = "perl_repositories")

perl_repositories = _perl_repositories
