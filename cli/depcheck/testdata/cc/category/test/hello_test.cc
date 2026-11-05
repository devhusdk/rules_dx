#include <cassert>
#include <greet/greet.h>
#include <testhelper/helper.h>

int main() { assert(greet::hello() == 0); assert(testhelper::check() == 0); return 0; }
