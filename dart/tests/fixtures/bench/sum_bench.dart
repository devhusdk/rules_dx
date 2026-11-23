import 'package:benchmark_harness/benchmark_harness.dart';
import 'package:dx_bench/dx_bench.dart';

class SumBench extends BenchmarkBase {
  SumBench() : super('sum');

  var total = 0;

  @override
  void run() {
    for (var i = 0; i < 1000; i++) {
      total += i;
    }
  }
}

void main() {
  runBenchmarks([SumBench()]);
}
