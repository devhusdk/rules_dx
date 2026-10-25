#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
  if (argc != 3 || strcmp(argv[1], "--marker") != 0) {
    fprintf(stderr, "want --marker <path>, got %d args\n", argc - 1);
    return 1;
  }
  FILE *marker = fopen(argv[2], "r");
  if (marker == NULL) {
    fprintf(stderr, "cannot open %s\n", argv[2]);
    return 1;
  }
  char line[64] = {0};
  const char *got = fgets(line, sizeof line, marker);
  fclose(marker);
  if (got == NULL || strcmp(line, "dx args marker\n") != 0) {
    fprintf(stderr, "unexpected marker contents\n");
    return 1;
  }
  return 0;
}
