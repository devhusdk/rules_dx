#include <stdio.h>
#include <string.h>

#include "examples/c-app/store/store.h"

#ifdef __cplusplus
#error "store_test.c must compile as C"
#endif

#define STOCK "examples/c-app/store/stock.json"

#define CHECK(cond)                                         \
  do {                                                      \
    if (!(cond)) {                                          \
      fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #cond); \
      return 1;                                             \
    }                                                       \
  } while (0)

int main(void) {
  CHECK(__STDC_VERSION__ >= 201112L);
  CHECK(store_schema() == 2);

  store_t store;
  char err[64] = "";
  CHECK(store_load(STOCK, &store, err, sizeof(err)) == 0);
  CHECK(store.count == 3);
  CHECK(store_total_cents(&store) == 120 * 35 + 300 * 8);
  const store_item *bolt = store_find(&store, "bolt-m6");
  CHECK(bolt != NULL && bolt->qty == 120 && bolt->price_cents == 35);
  CHECK(store_find(&store, "rivet") == NULL);
  store_free(&store);
  CHECK(store.count == 0 && store.items == NULL);

  CHECK(store_load("examples/c-app/store/missing.json", &store, err,
                   sizeof(err)) != 0);
  CHECK(strcmp(err, "cannot read stock file") == 0);
  CHECK(store.count == 0);

  CHECK(store_load("examples/c-app/store/not_json.txt", &store, err,
                   sizeof(err)) != 0);
  CHECK(strcmp(err, "stock file is not valid JSON") == 0);

  CHECK(store_load("examples/c-app/store/VERSION", &store, err, sizeof(err)) !=
        0);
  CHECK(strcmp(err, "stock file has no items array") == 0);
  return 0;
}
