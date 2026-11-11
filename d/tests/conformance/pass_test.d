module pass_test;

import greet : greet;

unittest
{
    assert(greet("world") == "hello world", "passing_case");
}
