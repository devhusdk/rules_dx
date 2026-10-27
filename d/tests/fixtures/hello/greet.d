module greet;

string greeting(string name) pure @safe
{
    return "Hello, " ~ name ~ "!";
}

unittest
{
    assert(greeting("d") == "Hello, d!");
}

unittest
{
    assert(greeting("") == "Hello, !");
}
