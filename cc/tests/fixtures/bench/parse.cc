#include <benchmark/benchmark.h>

#include <string>

static void BM_Parse(benchmark::State& state) {
  std::string input = "12345";
  for (auto _ : state) {
    int value = std::stoi(input);
    benchmark::DoNotOptimize(value);
  }
}
BENCHMARK(BM_Parse);
