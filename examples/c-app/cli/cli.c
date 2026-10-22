#include "examples/c-app/cli/cli.h"

#include <stdlib.h>
#include <string.h>

#include "examples/c-app/store/store.h"

#define DEFAULT_STOCK "examples/c-app/store/stock.json"

static int usage(FILE *err) {
  fprintf(err, "usage: stock [--file FILE] (list | total | get NAME)\n");
  return 2;
}

int cli_run(int argc, char **argv, FILE *out, FILE *err) {
  const char *path = getenv("STOCK_FILE");
  if (path == NULL) {
    path = DEFAULT_STOCK;
  }
  int i = 1;
  if (i + 1 < argc && strcmp(argv[i], "--file") == 0) {
    path = argv[i + 1];
    i += 2;
  }
  const char *command = i < argc ? argv[i] : "list";

  store_t store;
  char message[64] = "";
  if (store_load(path, &store, message, sizeof(message)) != 0) {
    fprintf(err, "stock: %s\n", message);
    return 3;
  }
  int code = 0;
  if (strcmp(command, "list") == 0) {
    for (size_t k = 0; k < store.count; k++) {
      fprintf(out, "%s %ld @ %ld\n", store.items[k].name, store.items[k].qty,
              store.items[k].price_cents);
    }
  } else if (strcmp(command, "total") == 0) {
    fprintf(out, "total %ld\n", store_total_cents(&store));
  } else if (strcmp(command, "get") == 0 && i + 1 < argc) {
    const store_item *item = store_find(&store, argv[i + 1]);
    if (item == NULL) {
      fprintf(err, "stock: no item %s\n", argv[i + 1]);
      code = 4;
    } else {
      fprintf(out, "%s %ld\n", item->name, item->qty);
    }
  } else {
    code = usage(err);
  }
  store_free(&store);
  return code;
}
