#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static int fits(int written, size_t size) {
  return written > 0 && (size_t)written < size;
}

static int holds_repository(const char *root, const char *repository) {
  char path[PATH_MAX];
  struct stat info;
  if (!fits(snprintf(path, sizeof path, "%s/%s", root, repository),
            sizeof path)) {
    return 0;
  }
  return stat(path, &info) == 0 && S_ISDIR(info.st_mode);
}

static int runfiles_beside(char *out, size_t size, const char *executable,
                           const char *repository) {
  return executable != NULL &&
         fits(snprintf(out, size, "%s.runfiles", executable), size) &&
         holds_repository(out, repository);
}

static int find_runfiles(char *out, size_t size, const char *argv0,
                         const char *repository) {
  static const char *const variables[] = {"RUNFILES_DIR", "TEST_SRCDIR"};
  char self[PATH_MAX];
  ssize_t length;
  size_t index;
  if (runfiles_beside(out, size, argv0, repository)) {
    return 1;
  }
  length = readlink("/proc/self/exe", self, sizeof self - 1);
  if (length > 0) {
    self[length] = '\0';
    if (runfiles_beside(out, size, self, repository)) {
      return 1;
    }
  }
  for (index = 0; index < sizeof variables / sizeof variables[0]; index++) {
    const char *value = getenv(variables[index]);
    if (value != NULL && value[0] != '\0' &&
        holds_repository(value, repository)) {
      return fits(snprintf(out, size, "%s", value), size);
    }
  }
  return 0;
}

int dx_qt_launch(int argc, char **argv, const char *repository,
                 const char *tool);

int dx_qt_launch(int argc, char **argv, const char *repository,
                 const char *tool) {
  static const char *const host_routes[] = {
      "DYLD_FALLBACK_FRAMEWORK_PATH",
      "DYLD_FALLBACK_LIBRARY_PATH",
      "DYLD_FRAMEWORK_PATH",
      "DYLD_INSERT_LIBRARIES",
      "DYLD_LIBRARY_PATH",
      "QML2_IMPORT_PATH",
      "QML_IMPORT_PATH",
      "QT_CONF_PATH",
      "QT_PLUGIN_PATH",
      "QT_QPA_PLATFORM_PLUGIN_PATH",
  };
  char root[PATH_MAX];
  char library[PATH_MAX];
  char program[PATH_MAX];
  size_t index;
  if (argc < 1 || !find_runfiles(root, sizeof root, argv[0], repository)) {
    fprintf(stderr, "rules_dx: cannot find the Qt runfiles of %s for %s\n",
            repository, tool);
    return 127;
  }
  if (!fits(snprintf(library, sizeof library, "%s/%s/lib", root, repository),
            sizeof library) ||
      !fits(
          snprintf(program, sizeof program, "%s/%s/%s", root, repository, tool),
          sizeof program)) {
    fprintf(stderr, "rules_dx: the Qt runfiles path of %s is too long\n", tool);
    return 127;
  }
  for (index = 0; index < sizeof host_routes / sizeof host_routes[0]; index++) {
    unsetenv(host_routes[index]);
  }
  if (setenv("LD_LIBRARY_PATH", library, 1) != 0) {
    perror("rules_dx: LD_LIBRARY_PATH");
    return 127;
  }
  argv[0] = program;
  execv(program, argv);
  fprintf(stderr, "rules_dx: cannot run %s: ", program);
  perror(NULL);
  return 127;
}
