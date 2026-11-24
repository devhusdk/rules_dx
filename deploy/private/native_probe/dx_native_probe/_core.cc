#define PY_SSIZE_T_CLEAN
#include <Python.h>

#include <cstring>

namespace {

PyObject *Greet(PyObject *, PyObject *) {
  // A runtime-sized copy and a throw make MSVC import vcruntime140.dll.
  static const char kGreeting[] = "dx-native-probe native";
  volatile size_t size = sizeof(kGreeting);
  char buffer[sizeof(kGreeting)];
  std::memcpy(buffer, kGreeting, size);
#ifdef _MSC_VER
  try {
    if (size != 0) throw static_cast<int>(size);
  } catch (int) {
  }
#endif
  return PyUnicode_FromString(buffer);
}

PyMethodDef kMethods[] = {
    {"greet", Greet, METH_NOARGS, "Return the probe greeting."},
    {nullptr, nullptr, 0, nullptr},
};

PyModuleDef kModule = {
    PyModuleDef_HEAD_INIT, "_core", nullptr, -1, kMethods,
};

} // namespace

PyMODINIT_FUNC PyInit__core() { return PyModule_Create(&kModule); }
