#include <stdio.h>
#include <string.h>

#include "examples/c-app/cli/cli.h"

#define CHECK(cond)                                                            \
  do {                                                                         \
    if (!(cond)) {                                                             \
      fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #cond);               \
      return 1;                                                                \
    }                                                                          \
  } while (0)

static int run(char **argv, int argc, char *out, size_t out_len, char *err,
               size_t err_len) {
  FILE *o = tmpfile();
  FILE *e = tmpfile();
  if (o == NULL || e == NULL) {
    return -1;
  }
  int code = cli_run(argc, argv, o, e);
  rewind(o);
  rewind(e);
  size_t n = fread(out, 1, out_len - 1, o);
  out[n] = '\0';
  n = fread(err, 1, err_len - 1, e);
  err[n] = '\0';
  fclose(o);
  fclose(e);
  return code;
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

  char *no_file[] = {"stock", "--file", "examples/c-app/store/absent.json",
                     "list"};
  CHECK(run(no_file, 4, out, sizeof(out), err, sizeof(err)) == 3);
  CHECK(strcmp(err, "stock: cannot read stock file\n") == 0);

  char *bad_file[] = {"stock", "--file",
                      "examples/c-app/store/malformed/negative.json", "list"};
  CHECK(run(bad_file, 4, out, sizeof(out), err, sizeof(err)) == 3);
  CHECK(strcmp(err, "stock: stock item is malformed\n") == 0);
  return 0;
}
