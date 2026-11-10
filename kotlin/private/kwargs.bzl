load("//libs/starlark:wrapper.bzl", "dx_src_path", "dx_without_warnings_attr")

_KOTLIN_TEST_ROOTS = [
    "src/test/java/",
    "src/test/kotlin/",
    "javatests/",
    "kotlin/",
    "java/",
    "test/",
]

def kotlin_has_own_opts(kwargs):
    return kwargs.get("kotlinc_opts") != None or kwargs.get("warnings_as_errors") != None

def kotlin_kotlinc_opts_with_warnings_as_errors(name, kwargs):
    """Returns kwargs with kotlinc_opts routed through the shared default or a per-target options rule."""
    out = dx_without_warnings_attr(kwargs)
    if kotlin_has_own_opts(kwargs):
        out["kotlinc_opts"] = ":" + name + "_kotlinc_opts"
    else:
        out["kotlinc_opts"] = str(Label("//kotlin/private:warnings_as_errors"))
    return out

def kotlin_test_class(name, srcs, package):
    """Returns the test class kt_jvm_test infers for the wrapper name, or None."""
    if type(srcs) != "list":
        return None
    for src in srcs:
        path = dx_src_path(package, src)
        if path == None:
            continue
        relative = path.replace(package + "/", "") if package else path
        if relative.split(".")[0] != name and path.split("/")[-1].split(".")[0] != name:
            continue
        for root in _KOTLIN_TEST_ROOTS:
            parts = path.split(root, 1)
            if len(parts) == 2:
                return parts[1].split(".")[0].replace("/", ".")
    return None

def kotlin_test_entry_kwargs(name, srcs, package, kwargs):
    """Returns kwargs with test_class set to the inferred class when unset."""
    if kwargs.get("test_class") not in (None, ""):
        return kwargs
    test_class = kotlin_test_class(name, srcs, package)
    if test_class == None:
        return kwargs
    return dict(kwargs, test_class = test_class)
