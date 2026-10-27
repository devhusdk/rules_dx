#include <QCoreApplication>
#include <QObject>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QString>
#include <QStringList>
#include <QUrl>
#include <QtGlobal>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <memory>
#include <set>
#include <sstream>
#include <string>
#include <vector>

#if defined(__APPLE__)
#include <mach-o/dyld.h>
#endif

namespace {

constexpr const char *kPinnedVersion = "6.11.3";

const std::vector<std::string> kHostGlibc = {
    "ld-linux-aarch64.so.1",
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libdl.so.2",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
};

#if defined(__APPLE__)
const std::vector<std::string> kHostPrefixes = {"/usr/local/", "/opt/",
                                                "/Library/Frameworks/"};

const std::vector<std::string> kRequired = {
    "QtCore", "QtNetwork", "QtQml", "QtQmlModels", "libqmlplugin.dylib",
};
#else
const std::vector<std::string> kHostPrefixes = {"/usr/", "/lib/", "/lib64/",
                                                "/opt/"};

#if defined(__aarch64__)
constexpr const char *kPcre = "libpcre2-8.so.0";
#else
constexpr const char *kPcre = "libpcre.so.1";
#endif

const std::vector<std::string> kRequired = {
    "libQt6Core.so.6",      "libQt6Network.so.6", "libQt6Qml.so.6",
    "libQt6QmlModels.so.6", "libicuuc.so.73",     "libglib-2.0.so.0",
    kPcre,                  "libqmlplugin.so",
};
#endif

constexpr const char *kComponent =
    "import QtQml\n"
    "import QtQml.Models\n"
    "QtObject {\n"
    "    property ListModel model: ListModel { ListElement { value: 7 } }\n"
    "    property int answer: model.get(0).value * 6\n"
    "}\n";

std::string Basename(const std::string &path) {
  const auto slash = path.rfind('/');
  return slash == std::string::npos ? path : path.substr(slash + 1);
}

bool StartsWith(const std::string &text, const std::string &prefix) {
  return text.compare(0, prefix.size(), prefix) == 0;
}

bool OnHost(const std::string &path) {
  for (const auto &prefix : kHostPrefixes) {
    if (StartsWith(path, prefix)) {
      return true;
    }
  }
  return false;
}

bool IsHostGlibc(const std::string &name) {
  for (const auto &allowed : kHostGlibc) {
    if (name == allowed) {
      return true;
    }
  }
  return false;
}

#if defined(__APPLE__)
std::set<std::string> MappedObjects() {
  std::set<std::string> objects;
  for (uint32_t index = 0; index < _dyld_image_count(); ++index) {
    objects.insert(_dyld_get_image_name(index));
  }
  return objects;
}
#else
std::set<std::string> MappedObjects() {
  std::set<std::string> objects;
  std::ifstream maps("/proc/self/maps");
  std::string line;
  while (std::getline(maps, line)) {
    const auto slash = line.find('/');
    if (slash == std::string::npos) {
      continue;
    }
    const std::string path = line.substr(slash);
    if (path.find(".so") != std::string::npos) {
      objects.insert(path);
    }
  }
  return objects;
}
#endif

std::string ImportRoot() {
  const char *qmldir = std::getenv("DX_QT_QMLDIR");
  const char *root = std::getenv("TEST_SRCDIR");
  if (qmldir == nullptr || root == nullptr) {
    return "";
  }
  std::string path = std::string(root) + "/" + qmldir;
  for (int level = 0; level < 2; ++level) {
    path = path.substr(0, path.rfind('/'));
  }
  return path;
}

int Fail(const std::string &message) {
  std::fprintf(stderr, "qt probe: %s\n", message.c_str());
  return 1;
}

} // namespace

int main(int argc, char **argv) {
  if (std::strcmp(qVersion(), kPinnedVersion) != 0) {
    return Fail(std::string("loaded Qt ") + qVersion() + ", want " +
                kPinnedVersion);
  }
  const std::string imports = ImportRoot();
  if (imports.empty()) {
    return Fail(
        "DX_QT_QMLDIR and TEST_SRCDIR must name the pinned QML imports");
  }
  QCoreApplication app(argc, argv);
  QQmlEngine engine;
  engine.setImportPathList(QStringList{QString::fromStdString(imports)});
  QQmlComponent component(&engine);
  component.setData(kComponent, QUrl(QStringLiteral("file:///probe.qml")));
  std::unique_ptr<QObject> object(component.create());
  if (!object) {
    return Fail("QML component failed: " +
                component.errorString().toStdString());
  }
  const int answer = object->property("answer").toInt();
  if (answer != 42) {
    return Fail("QML evaluated answer " + std::to_string(answer) + ", want 42");
  }
  const std::set<std::string> objects = MappedObjects();
  std::set<std::string> loaded;
  for (const auto &path : objects) {
    const std::string name = Basename(path);
    if (OnHost(path) && !IsHostGlibc(name)) {
      return Fail("loaded " + path + " from the host");
    }
    loaded.insert(name);
  }
  for (const auto &name : kRequired) {
    bool found = false;
    for (const auto &have : loaded) {
      found = found || StartsWith(have, name);
    }
    if (!found) {
      return Fail("expected " + name + " to be loaded");
    }
  }
  std::ostringstream report;
  for (const auto &path : objects) {
    report << path << "\n";
  }
  std::printf("Qt %s evaluated QML with %zu shared objects:\n%s", qVersion(),
              objects.size(), report.str().c_str());
  return 0;
}
