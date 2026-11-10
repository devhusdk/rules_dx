"""Private Jazzer helper for java_fuzz_test, kotlin_fuzz_test and scala_fuzz_test."""

load("@rules_java//java:defs.bzl", _java_binary = "java_binary")
load("//libs/starlark:fuzz.bzl", "dx_fuzz_test")
load("//libs/starlark:wrapper.bzl", "dx_src_path")

_ROOTS = [
    ["src", "main", "java"],
    ["src", "main", "kotlin"],
    ["src", "main", "scala"],
    ["src", "test", "java"],
    ["src", "test", "kotlin"],
    ["src", "test", "scala"],
    ["javatests"],
    ["java"],
    ["kotlin"],
    ["scala"],
    ["test"],
]

def _class_of(path):
    """Returns the dotted class for a source path without extension, or None without a source root."""
    parts = path.split("/")
    for i in range(len(parts)):
        for root in _ROOTS:
            if parts[i:i + len(root)] == root and i + len(root) < len(parts):
                return ".".join(parts[i + len(root):])
    return None

def jazzer_target_class(kind, name, srcs, target_class, package, ext):
    """Returns target_class, else the class inferred from the src named <name><ext> or the single src, else from the package and name; fails when none."""
    if target_class:
        return target_class
    path = None
    if type(srcs) == "list":
        paths = [dx_src_path(package, src) for src in srcs]
        for src_path in paths:
            if src_path != None and src_path.split("/")[-1] == name + ext:
                path = src_path[:-len(ext)]
                break
        if path == None and len(paths) == 1 and paths[0] != None and paths[0].endswith(ext):
            path = paths[0][:-len(ext)]
    if path == None:
        path = package + "/" + name if package else name
    inferred = _class_of(path)
    if inferred == None:
        fail("%s %s needs target_class" % (kind, name))
    return inferred

def jazzer_fuzz(name, library, srcs, deps, target_class, visibility, tags, kwargs, library_kwargs = {}):
    """Declares the fuzz library, the Jazzer binary and the `<name>_run` runner. library is the language's library macro."""
    dx_fuzz_test(
        name,
        library,
        dict({"srcs": srcs, "deps": deps}, **library_kwargs),
        tags = tags,
        visibility = visibility,
        fuzz_rule = _java_binary,
        fuzz_kwargs = {
            "main_class": "com.code_intelligence.jazzer.Jazzer",
            "runtime_deps": [":" + name + "_bin", Label("@dx_jazzer//jar"), Label("@dx_jazzer_api//jar")],
        },
        engine = "jazzer",
        target = target_class,
        **kwargs
    )
