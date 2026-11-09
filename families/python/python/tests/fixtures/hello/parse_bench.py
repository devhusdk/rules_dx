"""Benchmark fixture for python_bench."""


def test_parse(benchmark):
    """Times one parse."""
    benchmark(int, "42")
