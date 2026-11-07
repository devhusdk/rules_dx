#include <cstdio>

int dx_defines_value();

int main() {
    if (dx_defines_value() != 1024) {
        return 1;
    }
    std::puts("defines ok");
    return 0;
}
