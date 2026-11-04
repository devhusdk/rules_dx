/// Reads the command line.
module cli.options;

/// Returns the name to greet: the first argument, or "world".
string nameFrom(string[] args)
{
    return args.length > 1 ? args[1] : "world";
}

unittest
{
    assert(nameFrom(["app"]) == "world");
    assert(nameFrom(["app", "d"]) == "d");
}
