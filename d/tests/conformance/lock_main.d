/// Prints a greeting once the locked semver package compares two versions.
module lock_main;

import std.stdio : writeln;

import semver : SemVer;

void main()
{
    if (!(SemVer("1.2.3") < SemVer("1.10.0")))
    {
        throw new Exception("semver ordering is broken");
    }
    writeln("hello lock");
}
