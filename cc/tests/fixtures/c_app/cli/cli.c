#include "cc/tests/fixtures/c_app/cli/cli.h"

#include <stdlib.h>
#include <string.h>

#include "cc/tests/fixtures/c_app/store/store.h"

#define PATH_MAX_LEN 4096

static int is_file(const char *path) {
  FILE *file = fopen(path, "rb");
  if (file == NULL) {
    return 0;
  }
  return fclose(file) == 0;
}

static int append(char *buf, size_t *used, const char *text) {
  size_t pos = *used;
  while (*text != '\0' && pos + 1 < PATH_MAX_LEN) {
    buf[pos++] = *text++;
  }
  buf[pos] = '\0';
  *used = pos;
  return *text == '\0';
}

static int join(char *buf, const char *dir, const char *rest) {
  size_t used = 0;
  return append(buf, &used, dir) && append(buf, &used, "/") &&
         append(buf, &used, rest);
}

static int from_manifest(char *buf, const char *manifest, const char *key) {
  FILE *file = fopen(manifest, "r");
  if (file == NULL) {
    return 0;
  }
  size_t len = strlen(key);
  char line[2 * PATH_MAX_LEN];
  int found = 0;
  while (!found && fgets(line, sizeof(line), file) != NULL) {
    if (strncmp(line, key, len) == 0 && line[len] == ' ') {
      line[strcspn(line, "\r\n")] = '\0';
      size_t used = 0;
      found = append(buf, &used, line + len + 1);
    }
  }
  return fclose(file) == 0 && found;
}

static const char *packaged_stock(const char *argv0, char *buf) {
  const char *dirs[] = {getenv("RUNFILES_DIR"), getenv("TEST_SRCDIR")};
  for (size_t k = 0; k < 2; k++) {
    if (dirs[k] != NULL && join(buf, dirs[k], STOCK_RLOCATION) &&
        is_file(buf)) {
      return buf;
    }
  }
  const char *manifest = getenv("RUNFILES_MANIFEST_FILE");
  if (manifest != NULL && from_manifest(buf, manifest, STOCK_RLOCATION)) {
    return buf;
  }
  char tree[PATH_MAX_LEN];
  size_t used = 0;
  if (append(tree, &used, argv0) && append(tree, &used, ".runfiles") &&
      join(buf, tree, STOCK_RLOCATION) && is_file(buf)) {
    return buf;
  }
  return STOCK_RLOCATION;
}

static const char *user_path(const char *path, char *buf) {
  const char *base = getenv("BUILD_WORKING_DIRECTORY");
  int absolute =
      path[0] == '/' || path[0] == '\\' || (path[0] != '\0' && path[1] == ':');
  if (base == NULL || absolute || !join(buf, base, path)) {
    return path;
  }
  return buf;
}

static int usage(FILE *err) {
  fprintf(err, "usage: stock [--file FILE] (list | total | get NAME)\n");
  return 2;
}

int cli_run(int argc, char **argv, FILE *out, FILE *err) {
  char packaged[PATH_MAX_LEN];
  char resolved[PATH_MAX_LEN];
  const char *path = getenv("STOCK_FILE");
  int arg = 1;
  if (arg + 1 < argc && strcmp(argv[arg], "--file") == 0) {
    path = argv[arg + 1];
    arg += 2;
  }
  path = path != NULL ? user_path(path, resolved)
                      : packaged_stock(argv[0], packaged);
  const char *command = arg < argc ? argv[arg] : "list";

  store_t store;
  char message[64] = "";
  if (store_load(path, &store, message, sizeof(message)) != 0) {
    fprintf(err, "stock: %s\n", message);
    return 3;
  }
  int code = 0;
  if (strcmp(command, "list") == 0) {
    for (size_t k = 0; k < store.count; k++) {
      fprintf(out, "%s %lld @ %lld\n", store.items[k].name,
              (long long)store.items[k].qty,
              (long long)store.items[k].price_cents);
    }
  } else if (strcmp(command, "total") == 0) {
    fprintf(out, "total %lld\n", (long long)store_total_cents(&store));
  } else if (strcmp(command, "get") == 0 && arg + 1 < argc) {
    const store_item *item = store_find(&store, argv[arg + 1]);
    if (item == NULL) {
      fprintf(err, "stock: no item %s\n", argv[arg + 1]);
      code = 4;
    } else {
      fprintf(out, "%s %lld\n", item->name, (long long)item->qty);
    }
  } else {
    code = usage(err);
  }
  store_free(&store);
  return code;
}
