import ldc.libfuzzer;

mixin DefineTestOneInput!fuzz_target;

int fuzz_target(in ubyte[] data)
{
    if (data.length >= 3 && data[0] == 'f' && data[1] == 'u' && data[2] == 'z')
    {
        auto p = cast(ubyte*) data.ptr;
        p[data.length] = 0;
    }
    return 0;
}
