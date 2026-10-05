#include "cc/tests/fixtures/visibility/consumer/consumer.h"

int consume(void) { return default_visible() + explicit_visible(); }
