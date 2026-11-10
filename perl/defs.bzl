"""Public Perl API."""

load(
    "//perl/private:defs.bzl",
    _PERL_COMPATIBLE_WITH = "PERL_COMPATIBLE_WITH",
    _perl_bench = "perl_bench",
    _perl_binary = "perl_binary",
    _perl_library = "perl_library",
    _perl_test = "perl_test",
)

PERL_COMPATIBLE_WITH = _PERL_COMPATIBLE_WITH
perl_library = _perl_library
perl_binary = _perl_binary
perl_test = _perl_test
perl_bench = _perl_bench
