load("//libs/starlark:wrapper.bzl", "dx_append_flags", "dx_src_path", "dx_warnings_as_errors")
load("@rules_java//java/common/rules:java_helper.bzl", "helper")

def java_javacopts_with_warnings_as_errors(kwargs):
    """Returns kwargs with -Xlint:all and, unless turned off, -Werror on javacopts."""
    out, warning_flags = dx_warnings_as_errors(kwargs, ["-Werror"])
    out = dx_append_flags(out, "javacopts", ["-Xlint:all"])
    return dx_append_flags(out, "javacopts", warning_flags)

def java_entry_class(name, srcs, package):
    """Returns the class java_* infers for the wrapper name, or None without a Java root."""
    path = None
    if type(srcs) == "list":
        for src in srcs:
            src_path = dx_src_path(package, src)
            if src_path != None and src_path.split("/")[-1] == name + ".java":
                path = src_path[:-len(".java")]
                break
    if path == None:
        path = helper.get_relative(package, name)
    segments = helper.java_segments(path)
    return ".".join(segments) if segments != None else None

def _java_unset(value):
    """Returns whether an entry attribute counts as unset."""
    return value == None or value == ""

def _java_entry_wanted(srcs, kwargs, attr):
    """Returns whether upstream would infer attr for these kwargs."""
    if not _java_unset(kwargs.get(attr)):
        return False
    use_testrunner = kwargs.get("use_testrunner")
    create_executable = kwargs.get("create_executable")
    if attr == "test_class":
        return use_testrunner == None or use_testrunner == True
    if create_executable != None and create_executable != True:
        return False
    if use_testrunner != None and use_testrunner != False:
        return False
    return srcs != None and srcs != []

def java_entry_kwargs(name, srcs, package, kwargs, attr):
    """Returns kwargs with attr set to the inferred class when upstream would infer one."""
    if not _java_entry_wanted(srcs, kwargs, attr):
        return kwargs
    entry = java_entry_class(name, srcs, package)
    if entry == None:
        return kwargs
    return dict(kwargs, **{attr: entry})

def java_entry_error(name, srcs, package, kwargs, attr):
    """Returns the error for a wrapper that needs attr but cannot infer it, or None."""
    if not _java_entry_wanted(srcs, kwargs, attr):
        return None
    if java_entry_class(name, srcs, package) != None:
        return None
    kind = "java_test" if attr == "test_class" else "java_binary"
    return "%s //%s:%s: cannot infer %s from srcs or package; set %s." % (kind, package, name, attr, attr)
