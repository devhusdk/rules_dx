"""Public Perl API."""

load(
    "//perl/private:defs.bzl",
    _perl_bench = "perl_bench",
    _perl_binary = "perl_binary",
    _perl_library = "perl_library",
    _perl_test = "perl_test",
)

perl_library = _perl_library
perl_binary = _perl_binary
perl_test = _perl_test
perl_bench = _perl_bench
