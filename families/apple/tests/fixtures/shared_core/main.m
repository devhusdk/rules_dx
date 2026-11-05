int shared_core_open(const char *board);

int main(void) {
    return shared_core_open("") == 1 ? 0 : 1;
}
