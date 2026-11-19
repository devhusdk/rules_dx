module greet;

/// Returns a greeting for `name`.
string greet(string name) pure @safe
{
    return "hello " ~ name;
}
