module sum;

int sum(int a, int b) { return a + b; }

unittest { assert(sum(1, 2) == 3); }
