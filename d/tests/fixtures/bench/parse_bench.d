import core.time : Duration;
import std.datetime.stopwatch : benchmark;
import std.file : mkdirRecurse, write;
import std.format : format;
import std.path : buildPath;
import std.process : environment;

uint parse(string input)
{
    uint sum;
    foreach (c; input)
        if (c >= '0' && c <= '9')
            sum += c - '0';
    return sum;
}

void main()
{
    enum iterations = 1000;
    uint sink;
    Duration[1] times = benchmark!({ sink += parse("a1b2c3d4"); })(iterations);
    auto mean = cast(double) times[0].total!"nsecs" / iterations;
    auto dir = environment.get("TEST_UNDECLARED_OUTPUTS_DIR", ".");
    mkdirRecurse(dir);
    write(buildPath(dir, "dx-bench.json"),
        format(`{"schema":"dx-bench/v1","results":[{"name":"parse_small","unit":"ns/iter","mean":%.1f,"stddev":0,"iterations":%s}]}` ~ "\n", mean, iterations));
}
