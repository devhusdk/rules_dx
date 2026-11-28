#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cc/tests/fixtures/c_app/store/store.h"

#ifdef __cplusplus
#error "store_test.c must compile as C"
#endif

#define STOCK "cc/tests/fixtures/c_app/store/stock.json"

#define CHECK(cond)                                                            \
  do {                                                                         \
    if (!(cond)) {                                                             \
      fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #cond);               \
      return 1;                                                                \
    }                                                                          \
  } while (0)

/* Writes a stock file larger than the reader's first buffer. */
static int write_large_stock(const char *path, int items) {
  FILE *file = fopen(path, "wb");
  if (file == NULL) {
    return 0;
  }
  fputs("{\"items\":[", file);
  for (int i = 0; i < items; i++) {
    fprintf(file, "%s{\"name\":\"item-%d\",\"qty\":1,\"price_cents\":2}",
            i > 0 ? "," : "", i);
  }
  fputs("]}", file);
  return fclose(file) == 0;
}

static int append(char *buf, size_t cap, size_t *used, const char *text) {
  size_t pos = *used;
  while (*text != '\0' && pos + 1 < cap) {
    buf[pos++] = *text++;
  }
  buf[pos] = '\0';
  *used = pos;
  return *text == '\0';
}

static int concat(char *buf, size_t cap, const char *head, const char *tail) {
  size_t used = 0;
  return append(buf, cap, &used, head) && append(buf, cap, &used, tail);
}

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

  CHECK(store_load("cc/tests/fixtures/c_app/store/missing.json", &store, err,
                   sizeof(err)) != 0);
  CHECK(strcmp(err, "cannot read stock file") == 0);
  CHECK(store.count == 0);

  CHECK(store_load("cc/tests/fixtures/c_app/store/not_json.txt", &store, err,
                   sizeof(err)) != 0);
  CHECK(strcmp(err, "stock file is not valid JSON") == 0);

  CHECK(store_load("cc/tests/fixtures/c_app/store/VERSION", &store, err,
                   sizeof(err)) != 0);
  CHECK(strcmp(err, "stock file has no items array") == 0);

  const char *malformed[] = {
      "cc/tests/fixtures/c_app/store/malformed/wrong_type.json",
      "cc/tests/fixtures/c_app/store/malformed/fractional.json",
      "cc/tests/fixtures/c_app/store/malformed/negative.json",
      "cc/tests/fixtures/c_app/store/malformed/huge.json",
  };
  for (size_t i = 0; i < sizeof(malformed) / sizeof(malformed[0]); i++) {
    CHECK(store_load(malformed[i], &store, err, sizeof(err)) != 0);
    CHECK(strcmp(err, "stock item is malformed") == 0);
    CHECK(store.count == 0 && store.items == NULL);
  }

  const char *tmp = getenv("TEST_TMPDIR");
  CHECK(tmp != NULL);
  char large[512];
  CHECK(concat(large, sizeof(large), tmp, "/large_stock.json"));
  CHECK(write_large_stock(large, 100));
  CHECK(store_load(large, &store, err, sizeof(err)) == 0);
  CHECK(store.count == 100);
  CHECK(store_total_cents(&store) == 200);
  CHECK(store_find(&store, "item-99") != NULL);
  store_free(&store);
  return 0;
}
