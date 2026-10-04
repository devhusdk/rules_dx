#include "examples/adopt-polyglot/native_cpp/shapes.h"

#include <cassert>

int main() {
    assert(Area("square") == 4.0);
    assert(Area("triangle") == 2.0);
    assert(Area("unknown") == 0.0);
    return 0;
}
