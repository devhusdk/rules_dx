/* Portable stand-in for MakeAppx.exe used by pack_test on every host. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int copy_file(const char *from, const char *to) {
  FILE *in = fopen(from, "rb");
  FILE *out;
  char buf[4096];
  size_t n;
  if (!in)
    return 1;
  out = fopen(to, "wb");
  if (!out) {
    fclose(in);
    return 1;
  }
  while ((n = fread(buf, 1, sizeof buf, in)) > 0)
    fwrite(buf, 1, n, out);
  fclose(in);
  return fclose(out) != 0;
}

static void join(char *dst, size_t size, const char *dir, const char *name) {
  snprintf(dst, size, "%s/%s", dir, name);
}

int main(int argc, char **argv) {
  const char *capture = getenv("DX_FAKE_CAPTURE");
  const char *map = NULL;
  const char *out = NULL;
  char path[4096];
  char manifest[4096];
  char src[4096];
  char *slash;
  FILE *args;
  int i;
  if (!capture)
    return 2;
  join(path, sizeof path, capture, "args.txt");
  args = fopen(path, "wb");
  if (!args)
    return 2;
  for (i = 1; i < argc; i++)
    fprintf(args, "%s\n", argv[i]);
  fclose(args);
  if (getenv("DX_FAKE_MAKEAPPX_FAIL") && *getenv("DX_FAKE_MAKEAPPX_FAIL")) {
    fputs("MakeAppx : error: fake failure\n", stderr);
    return 3;
  }
  for (i = 1; i + 1 < argc; i++) {
    if (!strcmp(argv[i], "/f"))
      map = argv[i + 1];
    else if (!strcmp(argv[i], "/p"))
      out = argv[i + 1];
  }
  if (!map || !out)
    return 2;
  join(path, sizeof path, capture, "mapping.txt");
  if (copy_file(map, path))
    return 2;
  snprintf(src, sizeof src, "%s", map);
  slash = strrchr(src, '/');
  if (!slash)
    slash = strrchr(src, '\\');
  if (slash)
    *slash = '\0';
  else
    strcpy(src, ".");
  join(manifest, sizeof manifest, src, "AppxManifest.xml");
  join(path, sizeof path, capture, "AppxManifest.xml");
  if (copy_file(manifest, path))
    return 2;
  {
    FILE *pkg = fopen(out, "wb");
    if (!pkg)
      return 2;
    fputs("fake package\n", pkg);
    fclose(pkg);
  }
  return 0;
}
