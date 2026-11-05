module main;

import std.stdio : writeln;

extern (C) int cc_add(int a, int b);

void main()
{
    writeln(cc_add(2, 3));
}
