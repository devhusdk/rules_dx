"""Vue component libraries and app builds."""

load("@aspect_rules_js//js:defs.bzl", _js_library = "js_library")
load("//javascript/private:frameworks.bzl", "dx_framework_forward_rule", "dx_framework_library", "dx_framework_build", "dx_framework_test")

visibility(["//..."])

_vue_library_forward = dx_framework_forward_rule("vue", ".vue")

def vue_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over js_library for Vue SFCs."""
    dx_framework_library(name, srcs, _js_library, _vue_library_forward, skip = [], visibility = visibility, **kwargs)

def vue_build(name, srcs, config, node_modules, package_json = None, public = None, ssr_entry = None, deps = None, visibility = None, **kwargs):
    """Builds one Vite Vue app into a bundled web app tree and an optional SSR tree."""
    dx_framework_build(name, "vue_build", "Vue", ["vite", "@vitejs/plugin-vue", "vue"], srcs, config, node_modules, package_json = package_json, public = public, ssr_entry = ssr_entry, deps = deps, visibility = visibility, **kwargs)

def vue_test(name, *, srcs, node_modules, data = None, **kwargs):
    """Runs jest over Vue components, compiled by the linked @vue/compiler-sfc. Coverage maps back to the .vue sources."""
    dx_framework_test(name, srcs, Label("//vue/private:jest_config"), Label("//vue/private:jest_transform"), ["@vue/compiler-sfc", "vue"], node_modules, data = data, **kwargs)
