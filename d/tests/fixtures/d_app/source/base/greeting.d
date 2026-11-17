/// Builds the greeting the app prints.
module base.greeting;

/// Returns the greeting for `name`.
string greeting(string name)
{
    return "Hello, " ~ name ~ "!";
}

unittest
{
    assert(greeting("d") == "Hello, d!");
}
