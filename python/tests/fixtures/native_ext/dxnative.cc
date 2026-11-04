#define PY_SSIZE_T_CLEAN
#include <Python.h>

#include <string>
#include <vector>

namespace {

PyObject* Join(PyObject*, PyObject* args) {
  PyObject* list;
  const char* sep;
  Py_ssize_t sep_len;
  if (!PyArg_ParseTuple(args, "O!s#", &PyList_Type, &list, &sep, &sep_len)) {
    return nullptr;
  }
  std::vector<std::string> parts;
  Py_ssize_t size = PyList_GET_SIZE(list);
  for (Py_ssize_t i = 0; i < size; ++i) {
    Py_ssize_t len;
    const char* text = PyUnicode_AsUTF8AndSize(PyList_GET_ITEM(list, i), &len);
    if (text == nullptr) {
      return nullptr;
    }
    parts.emplace_back(text, len);
  }
  std::string out;
  for (size_t i = 0; i < parts.size(); ++i) {
    if (i > 0) {
      out.append(sep, sep_len);
    }
    out += parts[i];
  }
  return PyUnicode_FromStringAndSize(out.data(), out.size());
}

PyObject* BuildInfo(PyObject*, PyObject*) {
#if defined(_MSC_VER) && defined(__clang__)
  const char* compiler = "clang-cl";
#elif defined(_MSC_VER)
  const char* compiler = "msvc";
#elif defined(__clang__)
  const char* compiler = "clang";
#elif defined(__GNUC__)
  const char* compiler = "gcc";
#else
  const char* compiler = "unknown";
#endif
  return Py_BuildValue("(iis)", PY_MAJOR_VERSION, PY_MINOR_VERSION, compiler);
}

PyMethodDef kMethods[] = {
    {"join", Join, METH_VARARGS, "Join a list of str with a separator."},
    {"build_info", BuildInfo, METH_NOARGS,
     "Return (major, minor, compiler) of the build."},
    {nullptr, nullptr, 0, nullptr},
};

PyModuleDef kModule = {
    PyModuleDef_HEAD_INIT, "dxnative", nullptr, -1, kMethods,
};

}  // namespace

PyMODINIT_FUNC PyInit_dxnative() { return PyModule_Create(&kModule); }
