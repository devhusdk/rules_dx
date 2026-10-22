#include <cstring>

#include "examples/c-app/store/store.h"

int main() {
  store_t store;
  char err[64] = "";
  if (store_load("examples/c-app/store/stock.json", &store, err, sizeof(err)) !=
      0) {
    return 1;
  }
  const bool ok = store.count == 3 && store_find(&store, "nut-m6") != nullptr &&
                  store_find(&store, "nut-m6")->qty == 0;
  store_free(&store);
  return ok ? 0 : 1;
}
