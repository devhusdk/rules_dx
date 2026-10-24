#include <stdio.h>
#include <stdlib.h>

#include "greet.h"

int main(void) {
  const char *appdir = getenv("APPDIR");
  if (appdir == NULL) {
    fprintf(stderr, "dxhello: APPDIR is not set\n");
    return 2;
  }
  char path[4096];
  snprintf(path, sizeof path, "%s/usr/share/dxhello/fixtures/greeting.txt",
           appdir);
  FILE *file = fopen(path, "r");
  if (file == NULL) {
    fprintf(stderr, "dxhello: missing resource %s\n", path);
    return 3;
  }
  char line[256];
  if (fgets(line, sizeof line, file) == NULL) {
    fprintf(stderr, "dxhello: empty resource %s\n", path);
    fclose(file);
    return 4;
  }
  fclose(file);
  printf("%s", line);
  printf("%s\n", dx_greeting());
  printf("appdir=%s\n", appdir);
  return 0;
}
