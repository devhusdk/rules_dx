package bench;

import org.openjdk.jmh.annotations.Benchmark;

public class Sum {
  @Benchmark
  public long sum() {
    long total = 0;
    for (int i = 0; i < 1000; i++) {
      total += i;
    }
    return total;
  }
}
