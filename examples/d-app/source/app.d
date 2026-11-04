/// The d-app command line program.
module app;

import std.stdio : write, writeln;

import cli.options : nameFrom;
import base.banner : banner;
import base.greeting : greeting;
import semver : SemVer;

void main(string[] args)
{
    write(banner);
    writeln(greeting(nameFrom(args)));
    writeln("semver ", SemVer("1.2.3") < SemVer("1.10.0") ? "orders" : "is broken");
}
