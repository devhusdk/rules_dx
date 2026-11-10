package bench

import org.openjdk.jmh.annotations.Benchmark

class Sum {
  @Benchmark
  def sum(): Long = {
    var total = 0L
    var i = 0
    while (i < 1000) {
      total += i
      i += 1
    }
    total
  }
}
