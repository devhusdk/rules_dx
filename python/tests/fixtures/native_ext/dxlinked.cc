#define PY_SSIZE_T_CLEAN
#include <Python.h>

extern "C" int dx_helper_add(int a, int b);

namespace {

PyObject *Add(PyObject *, PyObject *args) {
  int a, b;
  if (!PyArg_ParseTuple(args, "ii", &a, &b)) {
    return nullptr;
  }
  return PyLong_FromLong(dx_helper_add(a, b));
}

PyMethodDef kMethods[] = {
    {"add", Add, METH_VARARGS, "Add two ints in the helper library."},
    {nullptr, nullptr, 0, nullptr},
};

PyModuleDef kModule = {
    PyModuleDef_HEAD_INIT, "dxlinked", nullptr, -1, kMethods,
};

} // namespace

PyMODINIT_FUNC PyInit_dxlinked() { return PyModule_Create(&kModule); }
