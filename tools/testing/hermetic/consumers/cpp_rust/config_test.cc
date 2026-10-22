#ifndef DX_CONSUMER_CONFIG
#error "the consumer-owned .bazelrc was not applied"
#endif

int main() { return DX_CONSUMER_CONFIG == 1 ? 0 : 1; }
