#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "tasks.h"

static int expect(const char *what, const char *got, const char *want) {
  if (strcmp(got, want) != 0) {
    fprintf(stderr, "%s: got '%s', want '%s'\n", what, got, want);
    return 1;
  }
  return 0;
}

static long slurp(const char *path, uint8_t *buf, size_t cap) {
  FILE *file = fopen(path, "rb");
  if (file == NULL) {
    return -1;
  }
  size_t len = fread(buf, 1, cap, file);
  fclose(file);
  return (long)len;
}

int main(void) {
  const char *board_path = getenv("DX_TASKS_BOARD");
  const char *notes_path = getenv("DX_TASKS_NOTES");
  if (board_path == NULL || notes_path == NULL) {
    fprintf(stderr, "DX_TASKS_BOARD and DX_TASKS_NOTES name the assets\n");
    return 2;
  }
  uint8_t board[4096];
  long board_len = slurp(board_path, board, sizeof(board));
  if (board_len < 0 || tasks_load(board, (size_t)board_len) != 4) {
    fprintf(stderr, "the packaged board did not load four tasks\n");
    return 1;
  }
  uint8_t out[128];
  int failures = 0;
  tasks_summary(out, sizeof(out));
  failures +=
      expect("summary", (const char *)out, "tasks=4 done=1 open=3 assets=1");
  const char *add = "add Link the C interface";
  failures +=
      tasks_apply((const uint8_t *)add, strlen(add), out, sizeof(out)) != 0;
  failures += expect("add", (const char *)out, "ok:added 5");
  const char *bad = "done 42";
  failures +=
      tasks_apply((const uint8_t *)bad, strlen(bad), out, sizeof(out)) != 1;
  failures += expect("reject", (const char *)out, "err:no_such_task");
  tasks_summary(out, sizeof(out));
  failures += expect("summary after", (const char *)out,
                     "tasks=5 done=1 open=4 assets=1");
  uint8_t notes[4096];
  long notes_len = slurp(notes_path, notes, sizeof(notes));
  if (notes_len < 0) {
    fprintf(stderr, "notes asset is unreadable\n");
    return 1;
  }
  const char *name = "notes.txt";
  tasks_asset_report((const uint8_t *)name, strlen(name), notes,
                     (size_t)notes_len, out, sizeof(out));
  failures += expect("asset", (const char *)out,
                     "asset notes.txt bytes=41 adler32:3d6a0e9d");
  return failures == 0 ? 0 : 1;
}
