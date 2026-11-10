import 'dart:convert';
import 'dart:io';

import 'package:benchmark_harness/benchmark_harness.dart';

/// Measures each benchmark and writes dx-bench.json, in seconds per run.
void runBenchmarks(List<BenchmarkBase> benchmarks) {
  final results = <Map<String, Object>>[];
  for (final benchmark in benchmarks) {
    final micros = benchmark.measure();
    results.add({
      'name': benchmark.name,
      'unit': 's',
      'mean': micros / 1e6,
      'stddev': 0.0,
      'iterations': 1,
    });
  }
  final dir = Platform.environment['TEST_UNDECLARED_OUTPUTS_DIR'];
  if (dir == null) {
    throw StateError('TEST_UNDECLARED_OUTPUTS_DIR is not set');
  }
  File('$dir/dx-bench.json').writeAsStringSync(jsonEncode({'schema': 'dx-bench/v1', 'results': results}));
}
