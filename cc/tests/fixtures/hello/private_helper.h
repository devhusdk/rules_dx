// Private header that sits in a cc_test's srcs, not in a library's hdrs.
#pragma once

// Twice returns 2 * x.
static inline int Twice(int x) { return 2 * x; }
