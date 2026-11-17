#include <stdio.h>
#include <string.h>
#include <windows.h>

#include <appmodel.h>

#include "greet.h"

static int exe_dir(char *out, DWORD size) {
  DWORD length = GetModuleFileNameA(NULL, out, size);
  if (length == 0 || length >= size) {
    return 0;
  }
  char *slash = strrchr(out, '\\');
  if (slash == NULL) {
    return 0;
  }
  *slash = '\0';
  return 1;
}

int main(void) {
  char dir[MAX_PATH];
  if (!exe_dir(dir, MAX_PATH)) {
    fprintf(stderr, "dxmsix: cannot locate the executable\n");
    return 2;
  }
  char path[MAX_PATH + 64];
  snprintf(path, sizeof path, "%s\\share\\greeting.txt", dir);
  FILE *file = fopen(path, "r");
  if (file == NULL) {
    fprintf(stderr, "dxmsix: missing resource %s\n", path);
    return 3;
  }
  char line[256];
  if (fgets(line, sizeof line, file) == NULL) {
    fprintf(stderr, "dxmsix: empty resource %s\n", path);
    fclose(file);
    return 4;
  }
  fclose(file);
  printf("resource=%s", line);
  printf("library=%s\n", greet_line());
  wchar_t name[128];
  UINT32 count = 128;
  if (GetCurrentPackageFullName(&count, name) == ERROR_SUCCESS) {
    printf("package=%ls\n", name);
  } else {
    printf("package=none\n");
  }
#ifdef DX_CONSUMER_PROBE
  printf("probe=on\n");
#else
  printf("probe=off\n");
#endif
  return 0;
}
