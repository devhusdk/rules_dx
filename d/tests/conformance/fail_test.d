module fail_test;

import greet : greet;

unittest
{
    assert(greet("world") == "goodbye world", "failing_case");
}
