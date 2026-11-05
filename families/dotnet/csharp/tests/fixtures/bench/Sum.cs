using BenchmarkDotNet.Attributes;

namespace BenchFixture
{
    public class SumBench
    {
        [Benchmark]
        public int Sum()
        {
            var total = 0;
            for (var i = 0; i < 1000; i++)
                total += i;
            return total;
        }
    }
}
