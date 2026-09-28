pub fn is_covered_language(path: &str) -> bool {
    if path.ends_with(".d.ts") || path.ends_with(".d.mts") || path.ends_with(".d.cts") {
        return false;
    }
    path.ends_with(".rs")
        || path.ends_with(".go")
        || path.ends_with(".py")
        || path.ends_with(".js")
        || path.ends_with(".jsx")
        || path.ends_with(".mjs")
        || path.ends_with(".cjs")
        || path.ends_with(".ts")
        || path.ends_with(".tsx")
        || path.ends_with(".mts")
        || path.ends_with(".cts")
        || path.ends_with(".java")
        || path.ends_with(".kt")
        || path.ends_with(".scala")
        || path.ends_with(".cs")
        || path.ends_with(".fs")
        || path.ends_with(".fsi")
        || path.ends_with(".c")
        || path.ends_with(".cc")
        || path.ends_with(".cpp")
        || path.ends_with(".cxx")
        || path.ends_with(".h")
        || path.ends_with(".hh")
        || path.ends_with(".hpp")
        || path.ends_with(".hxx")
}

pub(crate) fn is_starlark(path: &str) -> bool {
    path.ends_with(".bzl")
}
