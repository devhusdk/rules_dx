#define PY_SSIZE_T_CLEAN
#include <Python.h>

namespace {

PyObject *Greet(PyObject *, PyObject *) {
  return PyUnicode_FromString("dx-native-probe native");
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
