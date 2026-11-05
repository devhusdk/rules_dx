int twice(int x)
{
    return x * 2;
}

unittest
{
    static assert(twice(2) == 5);
}

void main()
{
}
