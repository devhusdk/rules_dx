#ifndef EXAMPLES_C_APP_STORE_STORE_H_
#define EXAMPLES_C_APP_STORE_STORE_H_

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  char name[32];
  long qty;
  long price_cents;
} store_item;

typedef struct {
  store_item *items;
  size_t count;
} store_t;

int store_load(const char *path, store_t *out, char *err, size_t err_len);
long store_total_cents(const store_t *store);
const store_item *store_find(const store_t *store, const char *name);
int store_schema(void);
void store_free(store_t *store);

#ifdef __cplusplus
}
#endif

#endif
