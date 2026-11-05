"""Benchmark fixture for python_benchmark."""


def test_parse(benchmark):
    """Times one parse."""
    benchmark(int, "42")
