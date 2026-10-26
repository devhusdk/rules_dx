#include "examples/c-app/store/store.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cJSON.h"
#include "examples/c-app/store/store_build.h"

static char *read_all(const char *path) {
  FILE *file = fopen(path, "rb");
  if (file == NULL) {
    return NULL;
  }
  size_t cap = 1024;
  size_t len = 0;
  char *buf = malloc(cap);
  while (buf != NULL) {
    size_t got = fread(buf + len, 1, cap - len - 1, file);
    len += got;
    if (got == 0) {
      break;
    }
    if (cap - len < 2) {
      cap *= 2;
      char *grown = realloc(buf, cap);
      // LCOV_EXCL_START - reason: realloc failure cannot be forced, issue: 2142
      if (grown == NULL) {
        free(buf);
        buf = NULL;
        break;
      }
      // LCOV_EXCL_STOP - reason: end of range, issue: 2142
      buf = grown;
    }
  }
  fclose(file);
  if (buf != NULL) {
    buf[len] = '\0';
  }
  return buf;
}

static int fail(char *err, size_t err_len, const char *message) {
  if (err != NULL && err_len > 0) {
    snprintf(err, err_len, "%s", message);
  }
  return -1;
}

#define STORE_MAX_VALUE 1000000.0

static int whole(const cJSON *number, long long *out) {
  double value = number->valuedouble;
  if (!(value >= 0.0 && value <= STORE_MAX_VALUE)) {
    return 0;
  }
  *out = (long long)value;
  return (double)*out == value;
}

int store_schema(void) { return STORE_SCHEMA; }

int store_load(const char *path, store_t *out, char *err, size_t err_len) {
  out->items = NULL;
  out->count = 0;
  char *text = read_all(path);
  if (text == NULL) {
    return fail(err, err_len, "cannot read stock file");
  }
  cJSON *root = cJSON_Parse(text);
  free(text);
  if (root == NULL) {
    return fail(err, err_len, "stock file is not valid JSON");
  }
  const cJSON *items = cJSON_GetObjectItemCaseSensitive(root, "items");
  if (!cJSON_IsArray(items)) {
    cJSON_Delete(root);
    return fail(err, err_len, "stock file has no items array");
  }
  int n = cJSON_GetArraySize(items);
  store_item *list = calloc((size_t)n + 1, sizeof(store_item));
  // LCOV_EXCL_START - reason: calloc failure cannot be forced, issue: 2142
  if (list == NULL) {
    cJSON_Delete(root);
    return fail(err, err_len, "out of memory");
  }
  // LCOV_EXCL_STOP - reason: end of range, issue: 2142
  size_t used = 0;
  const cJSON *entry = NULL;
  cJSON_ArrayForEach(entry, items) {
    const cJSON *name = cJSON_GetObjectItemCaseSensitive(entry, "name");
    const cJSON *qty = cJSON_GetObjectItemCaseSensitive(entry, "qty");
    const cJSON *price = cJSON_GetObjectItemCaseSensitive(entry, "price_cents");
    long long qty_value = 0;
    long long price_value = 0;
    if (!cJSON_IsString(name) || !cJSON_IsNumber(qty) ||
        !cJSON_IsNumber(price) || !whole(qty, &qty_value) ||
        !whole(price, &price_value) ||
        strlen(name->valuestring) >= sizeof(list[0].name)) {
      free(list);
      cJSON_Delete(root);
      return fail(err, err_len, "stock item is malformed");
    }
    snprintf(list[used].name, sizeof(list[used].name), "%s", name->valuestring);
    list[used].qty = qty_value;
    list[used].price_cents = price_value;
    used++;
  }
  cJSON_Delete(root);
  out->items = list;
  out->count = used;
  return 0;
}

long long store_total_cents(const store_t *store) {
  long long total = 0;
  for (size_t i = 0; i < store->count; i++) {
    total += store->items[i].qty * store->items[i].price_cents;
  }
  return total;
}

const store_item *store_find(const store_t *store, const char *name) {
  for (size_t i = 0; i < store->count; i++) {
    if (strcmp(store->items[i].name, name) == 0) {
      return &store->items[i];
    }
  }
  return NULL;
}

void store_free(store_t *store) {
  free(store->items);
  store->items = NULL;
  store->count = 0;
}
