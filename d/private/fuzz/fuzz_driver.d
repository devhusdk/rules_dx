/// Stands in for libFuzzer's `main` when a d_fuzz_test binary is built without the pinned
/// libFuzzer: on Windows, where lld-link cannot take -fsanitize=fuzzer, and when the binary
/// is built outside the fuzz transition. It feeds each input file, then `-runs=N` generated
/// inputs, to the target's LLVMFuzzerTestOneInput.
module dx_fuzz_driver;

import std.algorithm.searching : startsWith;
import std.conv : ConvException, to;
import std.file : FileException, read;
import std.stdio : stderr;

extern (C) int LLVMFuzzerTestOneInput(const(ubyte)* data, size_t size);

int main(string[] args)
{
    long runs = 0;
    bool ranFile = false;
    foreach (arg; args[1 .. $])
    {
        if (arg.startsWith("-runs="))
        {
            try
                runs = arg["-runs=".length .. $].to!long;
            catch (ConvException)
            {
                stderr.writeln("fuzz_driver: bad ", arg);
                return 1;
            }
        }
        else if (arg.length > 0 && arg[0] != '-')
        {
            const(ubyte)[] bytes;
            try
                bytes = cast(const(ubyte)[]) read(arg);
            catch (FileException e)
            {
                stderr.writeln("fuzz_driver: ", e.msg);
                return 1;
            }
            LLVMFuzzerTestOneInput(bytes.ptr, bytes.length);
            ranFile = true;
        }
    }
    // A fixed linear congruential generator keeps generated runs reproducible.
    uint state = 12345;
    foreach (run; 0 .. runs)
    {
        auto input = new ubyte[](cast(size_t)(run % 64));
        foreach (ref b; input)
        {
            state = state * 1664525u + 1013904223u;
            b = cast(ubyte)(state >> 24);
        }
        LLVMFuzzerTestOneInput(input.ptr, input.length);
    }
    if (!ranFile && runs == 0)
    {
        stderr.writeln("fuzz_driver: pass -runs=N or input files");
        return 1;
    }
    return 0;
}
