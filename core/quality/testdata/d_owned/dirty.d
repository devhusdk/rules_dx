module dirty;

int broken() pure @safe
{
    return undeclared_name;
}
