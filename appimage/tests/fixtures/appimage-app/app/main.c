#include <stdio.h>
#include <stdlib.h>

#include "greet.h"

int main(void) {
  const char *appdir = getenv("APPDIR");
  if (appdir == NULL) {
    fprintf(stderr, "dxappimage: APPDIR is not set\n");
    return 2;
  }
  char path[4096];
  snprintf(path, sizeof path, "%s/usr/share/dxappimage/greeting.txt", appdir);
  FILE *file = fopen(path, "r");
  if (file == NULL) {
    fprintf(stderr, "dxappimage: missing resource %s\n", path);
    return 3;
  }
  char line[256];
  if (fgets(line, sizeof line, file) == NULL) {
    fprintf(stderr, "dxappimage: empty resource %s\n", path);
    fclose(file);
    return 4;
  }
  fclose(file);
  printf("resource=%s", line);
  printf("library=%s\n", greet_line());
#ifdef DX_CONSUMER_PROBE
  printf("probe=on\n");
#else
  printf("probe=off\n");
#endif
  return 0;
}
