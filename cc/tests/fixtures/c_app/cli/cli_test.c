#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cc/tests/fixtures/c_app/cli/cli.h"

#define CHECK(cond)                                                            \
  do {                                                                         \
    if (!(cond)) {                                                             \
      fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #cond);               \
      return 1;                                                                \
    }                                                                          \
  } while (0)

static int run(char **argv, int argc, char *out, size_t out_len, char *err,
               size_t err_len) {
  FILE *out_file = tmpfile();
  FILE *err_file = tmpfile();
  if (out_file == NULL || err_file == NULL) {
    return -1;
  }
  int code = cli_run(argc, argv, out_file, err_file);
  rewind(out_file);
  rewind(err_file);
  size_t got = fread(out, 1, out_len - 1, out_file);
  out[got] = '\0';
  got = fread(err, 1, err_len - 1, err_file);
  err[got] = '\0';
  int closed = fclose(out_file) == 0;
  closed = fclose(err_file) == 0 && closed;
  return closed ? code : -1;
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

static void set_env(const char *name, const char *value) {
#ifdef _WIN32
  _putenv_s(name, value != NULL ? value : "");
#else
  if (value != NULL) {
    setenv(name, value, 1);
  } else {
    unsetenv(name);
  }
#endif
}

static int write_file(const char *path, const char *text) {
  FILE *file = fopen(path, "wb");
  if (file == NULL) {
    return 0;
  }
  fputs(text, file);
  return fclose(file) == 0;
}

static int lists_stock(char **argv, int argc) {
  char out[512];
  char err[512];
  return run(argv, argc, out, sizeof(out), err, sizeof(err)) == 0 &&
         strstr(out, "bolt-m6 120 @ 35") != NULL;
}

static int lacks_stock(char **argv, int argc) {
  char out[512];
  char err[512];
  return run(argv, argc, out, sizeof(out), err, sizeof(err)) == 3;
}

/* Resolves the packaged stock file through each runfiles fallback in turn. */
static int check_runfiles_fallbacks(void) {
  const char *found = getenv("RUNFILES_DIR");
  if (found == NULL) {
    found = getenv("TEST_SRCDIR");
  }
  const char *tmp = getenv("TEST_TMPDIR");
  if (found == NULL || tmp == NULL) {
    return 0;
  }
  char root[4096];
  char stock[4096];
  char manifest[4096];
  char text[8192];
  if (!concat(root, sizeof(root), found, "") ||
      !concat(stock, sizeof(stock), root, "/" STOCK_RLOCATION) ||
      !concat(manifest, sizeof(manifest), tmp, "/stock.manifest")) {
    return 0;
  }
  set_env("STOCK_FILE", NULL);
  set_env("RUNFILES_DIR", "/nonexistent");
  set_env("TEST_SRCDIR", "/nonexistent");

  char *plain[] = {"stock", "list"};
  size_t used = 0;
  if (!append(text, sizeof(text), &used,
              "other /nowhere\r\n" STOCK_RLOCATION
              ".bak /nowhere\n" STOCK_RLOCATION " ") ||
      !append(text, sizeof(text), &used, stock) ||
      !append(text, sizeof(text), &used, "\r\n") ||
      !write_file(manifest, text)) {
    return 0;
  }
  set_env("RUNFILES_MANIFEST_FILE", manifest);
  if (!lists_stock(plain, 2)) {
    return 0;
  }

  if (!write_file(manifest, "other /nowhere\n")) {
    return 0;
  }
  if (!lacks_stock(plain, 2)) {
    return 0;
  }
  if (!concat(manifest, sizeof(manifest), tmp, "/absent.manifest")) {
    return 0;
  }
  set_env("RUNFILES_MANIFEST_FILE", manifest);
  if (!lacks_stock(plain, 2)) {
    return 0;
  }

  set_env("RUNFILES_MANIFEST_FILE", NULL);
  size_t root_len = strlen(root);
  if (root_len > 9 && strcmp(root + root_len - 9, ".runfiles") == 0) {
    char binary[4096];
    if (!concat(binary, sizeof(binary), root, "")) {
      return 0;
    }
    binary[root_len - 9] = '\0';
    char *tree[] = {binary, "list"};
    if (!lists_stock(tree, 2)) {
      return 0;
    }
  }

  set_env("BUILD_WORKING_DIRECTORY", root);
  set_env("STOCK_FILE", STOCK_RLOCATION);
  return lists_stock(plain, 2);
}

int main(void) {
  char out[512];
  char err[512];

  char *list[] = {"stock", "list"};
  CHECK(run(list, 2, out, sizeof(out), err, sizeof(err)) == 0);
  CHECK(strcmp(out, "bolt-m6 120 @ 35\nwasher-m6 300 @ 8\nnut-m6 0 @ 22\n") ==
        0);

  char *total[] = {"stock", "total"};
  CHECK(run(total, 2, out, sizeof(out), err, sizeof(err)) == 0);
  CHECK(strcmp(out, "total 6600\n") == 0);

  char *get[] = {"stock", "get", "washer-m6"};
  CHECK(run(get, 3, out, sizeof(out), err, sizeof(err)) == 0);
  CHECK(strcmp(out, "washer-m6 300\n") == 0);

  char *missing[] = {"stock", "get", "rivet"};
  CHECK(run(missing, 3, out, sizeof(out), err, sizeof(err)) == 4);
  CHECK(strcmp(err, "stock: no item rivet\n") == 0 && out[0] == '\0');

  char *bad_cmd[] = {"stock", "sell"};
  CHECK(run(bad_cmd, 2, out, sizeof(out), err, sizeof(err)) == 2);
  CHECK(strstr(err, "usage:") == err);

  char *no_file[] = {"stock", "--file",
                     "cc/tests/fixtures/c_app/store/absent.json", "list"};
  CHECK(run(no_file, 4, out, sizeof(out), err, sizeof(err)) == 3);
  CHECK(strcmp(err, "stock: cannot read stock file\n") == 0);

  char *bad_file[] = {"stock", "--file",
                      "cc/tests/fixtures/c_app/store/malformed/negative.json",
                      "list"};
  CHECK(run(bad_file, 4, out, sizeof(out), err, sizeof(err)) == 3);
  CHECK(strcmp(err, "stock: stock item is malformed\n") == 0);

  CHECK(check_runfiles_fallbacks());
  return 0;
}
