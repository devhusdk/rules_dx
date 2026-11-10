package bench

import org.openjdk.jmh.annotations.Benchmark

open class Sum {
    @Benchmark
    open fun sum(): Long {
        var total = 0L
        for (i in 0 until 1000) {
            total += i
        }
        return total
    }
}
